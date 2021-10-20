use super::*;
use quote::quote;
use syn::{
    parse_macro_input,
    braced,
    parenthesized,
    Token,
    Type,
    Result,
    Ident,
    Expr,
    LitStr,
    LitInt,
    token,
    punctuated::Punctuated,
};
use syn::parse::{Parse, ParseStream};
use proc_macro2::Span;

mod kw
{
    syn::custom_keyword!(target);
}

enum MessageArg
{
    Target(kw::target),
    Arg(MessageArgDefn),
}

struct MessageArgDefn
{
    name: Ident,
    _colon: Token![:],
    typename: Type,
    _dot: Option<Token![.]>,
    expr: Option<Expr>
}

struct MessageDefn
{
    is_numeric: bool,
    name: String,
    typename: Ident,
    aliases: Option<Punctuated<Ident, Token![,]>>,
    _arrow1: Token![=>],
    _brace: token::Brace,
      _paren2: token::Paren,
        args: Punctuated<MessageArg, Token![,]>,
      _arrow2: Token![=>],
      value: LitStr,
}

struct MessageDefnList
{
    messages: Punctuated<MessageDefn, Token![,]>
}

impl Parse for MessageArg
{
    fn parse(input: ParseStream) -> Result<Self>
    {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::target)
        {
            Ok(Self::Target(input.parse()?))
        } else {
            Ok(Self::Arg(input.parse()?))
        }
    }
}

impl Parse for MessageArgDefn
{
    fn parse(input: ParseStream) -> Result<Self>
    {
        let name = input.parse::<Ident>()?;
        let _colon = input.parse::<Token![:]>()?;
        let typename = input.parse::<Type>()?;
        let _dot = input.parse::<Option<Token![.]>>()?;
        let expr = if _dot.is_some() { Some(input.parse::<Expr>()?) } else { None };

        Ok(Self { name: name, _colon: _colon, typename: typename, _dot: _dot, expr: expr })
    }
}

impl Parse for MessageDefn
{
    fn parse(input: ParseStream) -> Result<Self>
    {
        let content1;
        let content2;
        let content3;

        let (is_numeric, name, typename) = if let Ok(i) = input.parse::<LitInt>() {
            (true, i.to_string(), Ident::new(&format!("Numeric{}", i), Span::call_site()))
        } else {
            let ident: Ident = input.parse()?;
            (false, ident.to_string(), ident)
        };

        let aliases = if input.peek(token::Paren) {
            let _paren = parenthesized!(content1 in input);
            Some(content1.parse_terminated(Ident::parse)?)
        } else {
            None
        };

        Ok(MessageDefn {
            is_numeric: is_numeric,
            name: name,
            typename: typename,
            aliases: aliases,
            _arrow1: input.parse()?,
            _brace: braced!(content2 in input),
            _paren2: parenthesized!(content3 in content2),
            args: content3.parse_terminated(MessageArg::parse)?,
            _arrow2: content2.parse()?,
            value: content2.parse()?
        })
    }
}

impl Parse for MessageDefnList
{
    fn parse(input: ParseStream) -> Result<Self>
    {
        Ok(Self {
            messages: input.parse_terminated(MessageDefn::parse)?
        })
    }
}

pub fn define_messages(input: TokenStream) -> TokenStream
{
    let input = parse_macro_input!(input as MessageDefnList);

    generate_message_list(input)
}

fn generate_message_list(input: MessageDefnList) -> TokenStream
{
    let mut out = proc_macro2::TokenStream::new();

    for message in input.messages
    {
        let name = message.name;
        let typename = message.typename;
        let format_str = message.value;
        let aliases = message.aliases.iter();

        let mut message_args = Vec::new();
        let mut message_argtypes = Vec::new();

        let mut format_args = Vec::new();
        let mut format_values = Vec::new();

        let mut need_target = message.is_numeric;

        for arg_or_targ in message.args
        {
            match arg_or_targ {
                MessageArg::Target(_) => {
                    need_target = true;
                },
                MessageArg::Arg(arg) => {
                    message_args.push(arg.name.clone());
                    message_argtypes.push(arg.typename.clone());
                    format_args.push(arg.name.clone());
                    let fval_name = arg.name;
                    let fval_val = if let Some(e) = arg.expr {
                        quote!(#fval_name.#e)
                    } else {
                        quote!(#fval_name)
                    };
                    format_values.push(fval_val);
                }
            }
        }

        let (target_arg, target_def) = if need_target {
            (Some(quote!(target: &impl crate::ircd::irc::messages::MessageTarget, )), Some(quote!(target = target.format(), )))
        } else {
            (None, None)
        };

        let prefix = if message.is_numeric {
            Some(quote!(":{source} ", #name, " {target} ", ))
        } else {
            None
        };

        out.extend(quote!(
            #[derive(Debug,Clone)]
            pub struct #typename(String);
            #( pub type #aliases = #typename; )*

            impl #typename
            {
                pub fn new(source: &impl crate::ircd::irc::messages::MessageSource, #target_arg #( #message_args: #message_argtypes ),* ) -> Self
                {
                    Self(format!(concat!(#prefix #format_str, "\r\n"),
                                 source = source.format(),
                                 #target_def
                                 #( #format_args = #format_values),*
                            ))
                }
            }

            impl std::fmt::Display for #typename
            {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result
                {
                    self.0.fmt(f)
                }
            }

            impl crate::ircd::irc::messages::Message for #typename
            { }
        ));
    }

    //panic!("{}", out);

    out.into()
}