use super::*;
use crate::ircd::*;
use std::collections::HashMap;
use std::cell::RefCell;
use irc::numeric;

use ircd_macros::command_handler;

mod processor;
pub use processor::*;

type CommandResult = Result<(), CommandError>;

pub trait CommandHandler
{
    fn min_parameters(&self) -> usize;

    fn validate(&self, server: &Server, cmd: &ClientCommand) -> CommandResult
    {
        if cmd.args.len() < self.min_parameters()
        {
            return Err(numeric::NotEnoughParameters::new(server, &cmd.source, &cmd.command).into());
        }
        Ok(())
    }

    fn handle(&self, server: &Server, cmd: &ClientCommand, proc: &mut CommandProcessor) -> CommandResult
    {
        match &cmd.source {
            CommandSource::PreClient(pc) => {
                self.handle_preclient(server, pc, cmd, proc)
            },
            CommandSource::User(u) => {
                self.handle_user(server, &u, cmd, proc)
            }
        }
    }

    fn handle_preclient<'a>(&self, server: &Server, source: &'a RefCell<PreClient>, _cmd: &ClientCommand, _proc: &mut CommandProcessor) -> CommandResult
    {
        Err(numeric::NotRegistered::new(server, &*source.borrow()).into())
    }

    fn handle_user<'a>(&self, server: &Server, source: &'a wrapper::User, _cmd: &ClientCommand, _proc: &mut CommandProcessor) -> CommandResult
    {
        Err(numeric::AlreadyRegistered::new(server, source).into())
    }
}

pub struct CommandRegistration
{
    command: String,
    handler: Box<dyn CommandHandler>
}

pub struct CommandDispatcher
{
    handlers: HashMap<String, &'static Box<dyn CommandHandler>>
}

inventory::collect!(CommandRegistration);

impl CommandDispatcher {
    pub fn new() -> Self
    {
        let mut map = HashMap::new();

        for reg in inventory::iter::<CommandRegistration> {
            map.insert(reg.command.to_ascii_uppercase(), &reg.handler);
        }

        Self {
            handlers: map
        }
    }

    pub fn resolve_command(&self, cmd: &str) -> Option<&Box<dyn CommandHandler>>
    {
        self.handlers.get(&cmd.to_ascii_uppercase()).map(|x| *x)
    }
}

mod nick;
mod user;
mod join;
mod part;
mod message;
mod quit;
