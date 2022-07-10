use super::*;

/// Saved state of a [`Server`] for later resumption
#[derive(serde::Serialize,serde::Deserialize)]
pub struct ServerState<Policy = crate::policy::StandardPolicyService>
    where Policy: PolicyService + Saveable
{
    id: ServerId,
    name: ServerName,
    net: Network,
    epoch: EpochId,
    id_generator: ObjectIdGenerator,
    history_log: NetworkHistoryLog,
    policy_state: Policy::Saved,
}

impl<Policy: PolicyService + Saveable> Server<Policy>
{
    /// Save the server's state for later resumption
    pub fn save_state(self) -> ServerState<Policy>
    {
        ServerState {
            id: self.my_id,
            name: self.name,
            net: self.net.into_inner(),
            epoch: self.epoch,
            id_generator: self.id_generator,
            history_log: self.history_log.into_inner(),
            policy_state: self.policy_service.save(),
        }
    }

    /// Restore from a previously saved state.
    ///
    /// The `listener_collection` is only used during the resumption to restore
    /// connection data; the other arguments are as for [`new`](Self::new).
    pub fn restore_from(
            state: ServerState<Policy>,
            event_log: Arc<ReplicatedEventLog>,
            rpc_receiver: UnboundedReceiver<NetworkMessage>,
            subscriber: UnboundedSender<NetworkHistoryUpdate>,
        ) -> std::io::Result<Self>
    {
        Ok(Self {
            my_id: state.id,
            name: state.name,
            version: Self::build_version(),
            net: RwLock::new(state.net),
            epoch: state.epoch,
            id_generator: state.id_generator,
            event_log,
            rpc_receiver: Mutex::new(rpc_receiver),
            history_log: RwLock::new(state.history_log),
            subscriber,
            policy_service: Policy::restore(state.policy_state),
        })
    }
}