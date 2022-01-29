use client_listener::*;

use std::collections::HashMap;
use std::net::SocketAddr;

use tokio::{
//    select,
    sync::mpsc::{
        channel
    },
};
use simple_logger::SimpleLogger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
{
    SimpleLogger::new().with_level(log::LevelFilter::Debug)
                       .init().unwrap();

    let mut connections = HashMap::new();
    let (event_send, mut event_recv) = channel(128);

    let addr: SocketAddr = "127.0.0.1:5555".parse()?;

    let listeners = ListenerCollection::new(event_send)?;
    let _id = listeners.add_listener(addr, ConnectionType::Clear)?;

    while let Some(event) = event_recv.recv().await
    {
        match event.detail
        {
            ConnectionEventDetail::NewConnection(conn) =>
            {
                log::info!("New connection {:?}", event.source);
                connections.insert(event.source, conn);
            }
            ConnectionEventDetail::Message(msg) =>
            {
                log::info!("Message from {:?}: {}", event.source, msg);
                if let Some(conn) = connections.get(&event.source)
                {
                    conn.send(msg);
                }
                else
                {
                    log::warn!("Got message from unknown connection id {:?}", event.source);
                }
            }
            ConnectionEventDetail::Error(err) =>
            {
                log::error!("Got error {}", err);
            }
        }
    }

    Ok(())
}