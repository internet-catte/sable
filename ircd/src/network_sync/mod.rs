use irc_network::event::*;

use gossip::*;
use tokio::{
    sync::mpsc::{
        Sender,
        Receiver,
    },
};

struct UpdateHandler
{
    send_channel: Sender<Event>,
}

impl gossip::UpdateHandler for UpdateHandler
{
    fn on_update(&self, update: gossip::Update)
    {
        if let Ok(event) = serde_json::from_slice::<Event>(update.content())
        {
            log::debug!("Got incoming event: {:?}", event);
            // Panic if we can't send the event for processing
            self.send_channel.blocking_send(event).unwrap();
        }
    }
}

pub struct NetworkSync
{

}

impl NetworkSync {
    async fn push_task(channel: Receiver<Event>, service: gossip::GossipService<UpdateHandler>)
    {
        let mut channel = channel;
        while let Some(event) = channel.recv().await
        {
            log::debug!("Sending outgoing event: {:?}", event);
            service.submit(serde_json::to_vec(&event).unwrap()).unwrap();
        }
    }

    pub fn start(gossip_addr: String,
                 peer_addr: Option<String>,
                 inbound_send: Sender<Event>,
                 outbound_recv: Receiver<Event>,
                )
    {
        let peer_init = || { peer_addr.map(|x| vec!(Peer::new(x))) };

        let gossip_handler = Box::new(UpdateHandler { send_channel: inbound_send });
        let mut gossip_service: GossipService<UpdateHandler> = 
                GossipService::new(gossip_addr.parse().unwrap(), PeerSamplingConfig::default(), GossipConfig::default());

        gossip_service.start(Box::new(peer_init), gossip_handler).unwrap();

        tokio::spawn(Self::push_task(outbound_recv, gossip_service));
    }
}