use ircd::*;
use irc_server::{
    Server,
};
use client_listener::*;

mod management
{
    mod command;
    pub use command::*;
    mod service;
    pub use service::*;
}

#[derive(Debug,StructOpt)]
#[structopt(rename_all = "kebab")]
struct Opts
{
    /// Network-wide config file location
    #[structopt(short,long)]
    network_conf: PathBuf,

    /// Server config file location
    #[structopt(short,long)]
    server_conf: PathBuf
}

#[derive(Debug,Deserialize)]
struct TlsConfig
{
    key_file: PathBuf,
    cert_file: PathBuf,
}

#[derive(Debug,Deserialize)]
struct ListenerConfig
{
    address: String,
    #[serde(default)]
    tls: bool,
}

#[derive(Debug,Deserialize)]
struct ServerConfig
{
    server_id: ServerId,
    server_name: ServerName,

    management_address: String,

    listeners: Vec<ListenerConfig>,

    tls_config: Option<TlsConfig>,
    node_config: NodeConfig,
}

impl ServerConfig
{
    pub fn load_file<P: AsRef<Path>>(filename: P) -> Result<Self, ConfigError>
    {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader)?)
    }
}

fn load_tls_server_config(conf: &TlsConfig) -> Result<client_listener::TlsSettings, Box<dyn Error>>
{
    let cert_file = File::open(&conf.cert_file)?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain = rustls_pemfile::certs(&mut cert_reader)?;

    let key_file = File::open(&conf.key_file)?;
    let mut key_reader = BufReader::new(key_file);

    let server_key = rustls_pemfile::read_one(&mut key_reader)?;

    use rustls_pemfile::Item;

    let server_key = match server_key {
        Some(Item::RSAKey(key)) | Some(Item::PKCS8Key(key)) => Ok(key),
        Some(Item::X509Certificate(_)) | None => Err(ConfigError::FormatError("No private key in file".to_string()))
    }?;

    Ok(client_listener::TlsSettings { key: server_key, cert_chain: cert_chain })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let opts = Opts::from_args();

    let network_config = NetworkConfig::load_file(opts.network_conf)?;
    let server_config = ServerConfig::load_file(opts.server_conf)?;

    SimpleLogger::new().with_level(log::LevelFilter::Debug)
//                       .with_module_level("ircd_sync::replicated_log", log::LevelFilter::Trace)
//                       .with_module_level("irc_server::server", log::LevelFilter::Trace)
                       .with_module_level("rustls", log::LevelFilter::Info)
                       .init().unwrap();

    let (client_send, client_recv) = channel(128);
    let (server_send, server_recv) = channel(128);
    let (new_send, new_recv) = channel(128);
    let (shutdown_send, shutdown_recv) = channel(1);

    let id_gen = EventIdGenerator::new(server_config.server_id, EpochId::new(1), 0);
    let mut event_log = ReplicatedEventLog::new(id_gen, server_send, new_recv, network_config, server_config.node_config);

    let client_listeners = ListenerCollection::new(client_send)?;

    let mut server = Server::new(server_config.server_id,
                                 server_config.server_name,
                                 client_recv,
                                 server_recv,
                                 new_send);

    if let Some(conf) = server_config.tls_config {
        let tls_conf = load_tls_server_config(&conf)?;
        client_listeners.load_tls_certificates(tls_conf)?;
    }

    for listener in server_config.listeners
    {
        let conn_type = if listener.tls {ConnectionType::Tls} else {ConnectionType::Clear};
        client_listeners.add_listener(listener.address.parse().unwrap(), conn_type)?;
    }

    ctrlc::set_handler(move || {
        shutdown_send.try_send(()).expect("Failed to send shutdown command");
    }).expect("Failed to set Ctrl+C handler");

    let (management_send, management_recv) = channel(128);
    let management_address = server_config.management_address.clone();

    let _management_task = tokio::spawn(async move {
        let mut server = management::ManagementServer::start(management_address.parse().unwrap());

        while let Some(cmd) = server.recv().await
        {
            match cmd
            {
                management::ManagementCommand::ServerCommand(scmd) =>
                {
                    management_send.send(scmd).await.ok();
                }
            }
        }
    });

    event_log.sync_to_network().await;

    tokio::spawn(event_log.sync_task());

    // Run the actual server
    server.run(management_recv, shutdown_recv).await;

    // ...and once it shuts down, give the network sync some time to push the ServerQuit out
    time::sleep(std::time::Duration::new(1,0)).await;

    Ok(())
}