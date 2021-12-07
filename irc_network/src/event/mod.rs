//! Defines the `Event` type and associated objects.

mod clock;
mod event;

pub mod details;

pub use clock::EventClock;

pub use event::Event;
pub use event::DetailType;

pub use details::*;

/// An update to be applied by the event log.
#[derive(Debug)]
pub enum EventLogUpdate
{
    /// Create and apply a new `Event`. Note that only the target and details
    /// are specified here; other fields are generated by the event log itself
    /// based on its current state.
    NewEvent(crate::ObjectId, EventDetails),

    /// Update this server's current epoch. All future events will be emitted
    /// with the new epoch ID.
    EpochUpdate(crate::EpochId)
}

#[cfg(test)]
mod tests;