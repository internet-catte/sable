use crate::capability::ClientCapability;
use crate::capability::WithSupportedTags;
use crate::errors::HandleResult;
use crate::messages::MessageSink;
use crate::prelude::numeric;
use sable_network::prelude::*;
use sable_network::utils::*;

use super::message;

/// Extension trait to translate a network history entry into client protocol messages
pub(crate) trait SendHistoryItem {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult;
}

impl SendHistoryItem for HistoryLogEntry {
    fn send_to(&self, conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        match &self.details {
            NetworkStateChange::NewUser(detail) => detail.send_to(conn, self),
            NetworkStateChange::NewUserConnection(detail) => detail.send_to(conn, self),
            NetworkStateChange::UserConnectionDisconnected(detail) => detail.send_to(conn, self),
            NetworkStateChange::UserAwayChange(detail) => detail.send_to(conn, self),
            NetworkStateChange::UserNickChange(detail) => detail.send_to(conn, self),
            NetworkStateChange::UserModeChange(detail) => detail.send_to(conn, self),
            NetworkStateChange::UserQuit(detail) => detail.send_to(conn, self),
            NetworkStateChange::BulkUserQuit(detail) => detail.send_to(conn, self),
            NetworkStateChange::ChannelModeChange(detail) => detail.send_to(conn, self),
            NetworkStateChange::ChannelTopicChange(detail) => detail.send_to(conn, self),
            NetworkStateChange::ListModeAdded(detail) => detail.send_to(conn, self),
            NetworkStateChange::ListModeRemoved(detail) => detail.send_to(conn, self),
            NetworkStateChange::MembershipFlagChange(detail) => detail.send_to(conn, self),
            NetworkStateChange::ChannelJoin(detail) => detail.send_to(conn, self),
            NetworkStateChange::ChannelKick(detail) => detail.send_to(conn, self),
            NetworkStateChange::ChannelPart(detail) => detail.send_to(conn, self),
            NetworkStateChange::ChannelInvite(detail) => detail.send_to(conn, self),
            NetworkStateChange::ChannelRename(detail) => detail.send_to(conn, self),
            NetworkStateChange::NewMessage(detail) => detail.send_to(conn, self),
            NetworkStateChange::NewServer(detail) => detail.send_to(conn, self),
            NetworkStateChange::ServerQuit(detail) => detail.send_to(conn, self),
            NetworkStateChange::NewAuditLogEntry(detail) => detail.send_to(conn, self),
            NetworkStateChange::UserLoginChange(detail) => detail.send_to(conn, self),
            NetworkStateChange::ServicesUpdate(detail) => detail.send_to(conn, self),
            // EventComplete is handled further up and has no meaning here
            NetworkStateChange::EventComplete(_) => Ok(()),
        }
    }
}

impl SendHistoryItem for update::NewUser {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        Ok(())
    }
}

impl SendHistoryItem for update::NewUserConnection {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        Ok(())
    }
}

impl SendHistoryItem for update::UserConnectionDisconnected {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        Ok(())
    }
}

impl SendHistoryItem for update::UserAwayChange {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        if Some(self.user.user.id) == conn.user_id() {
            // Echo back to the user
            let message = match self.new_reason {
                None => numeric::Unaway::new(),
                Some(_) => numeric::NowAway::new(),
            };
            conn.send(message.format_for(&self.user, &self.user));
        } else {
            // Tell other users sharing a channel if they enabled away-notify
            let message = match self.new_reason {
                None => message::Unaway::new(&self.user),
                Some(reason) => message::Away::new(&self.user, reason.value()),
            };
            let message = message.with_tags_from(from_entry);
            conn.send(message.with_required_capabilities(ClientCapability::AwayNotify));
        }

        Ok(())
    }
}

impl SendHistoryItem for update::UserNickChange {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Nick::new(&self.user, &self.new_nick).with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::UserModeChange {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Mode::new(
            &self.user,
            &self.user,
            &format_umode_changes(&self.added, &self.removed),
        )
        .with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::UserQuit {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Quit::new(&self.user, &self.message).with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::BulkUserQuit {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        Ok(())
    }
}

impl SendHistoryItem for update::ChannelModeChange {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let (mut changes, params) = format_cmode_changes(self);
        for p in params {
            changes.push(' ');
            changes.push_str(&p);
        }

        let message = message::Mode::new(&self.changed_by, &self.channel, &changes)
            .with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::ChannelTopicChange {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Topic::new(&self.setter, &self.channel.name, &self.new_text)
            .with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::ListModeAdded {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let text = format!("+{} {}", self.list_type.mode_char(), self.pattern);
        let message =
            message::Mode::new(&self.set_by, &self.channel, &text).with_tags_from(from_entry);
        conn.send(message);
        Ok(())
    }
}

impl SendHistoryItem for update::ListModeRemoved {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let text = format!("-{} {}", self.list_type.mode_char(), self.pattern);
        let message =
            message::Mode::new(&self.removed_by, &self.channel, &text).with_tags_from(from_entry);
        conn.send(message);
        Ok(())
    }
}

impl SendHistoryItem for update::MembershipFlagChange {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let (mut changes, args) =
            format_channel_perm_changes(&self.user.nickname, &self.added, &self.removed);

        changes += " ";
        changes += &args.join(" ");

        let message = message::Mode::new(&self.changed_by, &self.channel, &changes)
            .with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::ChannelJoin {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Join::new(&self.user, &self.channel.name).with_tags_from(from_entry);

        conn.send(message);

        if !self.membership.permissions.is_empty() {
            let (mut changes, args) = format_channel_perm_changes(
                &self.user.nickname,
                &self.membership.permissions,
                &MembershipFlagSet::new(),
            );

            changes += " ";
            changes += &args.join(" ");

            let msg = message::Mode::new(&self.user, &self.channel, &changes);
            conn.send(msg);
        }

        if let Some(away_reason) = self.user.user.away_reason {
            let message =
                message::Away::new(&self.user, away_reason.value()).with_tags_from(from_entry);

            conn.send(message.with_required_capabilities(ClientCapability::AwayNotify));
        }

        Ok(())
    }
}

impl SendHistoryItem for update::ChannelKick {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message =
            message::Kick::new(&self.source, &self.user, &self.channel.name, &self.message)
                .with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::ChannelPart {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Part::new(&self.user, &self.channel.name, &self.message)
            .with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::ChannelInvite {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Invite::new(&self.source, &self.user, &self.channel.name)
            .with_tags_from(from_entry);

        conn.send(message);

        Ok(())
    }
}

impl SendHistoryItem for update::ChannelRename {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        // Not part of history, so it is handled entirely in send_realtime.rs.
        // See https://github.com/ircv3/ircv3-specifications/issues/532
        Ok(())
    }
}

impl SendHistoryItem for update::NewMessage {
    fn send_to(&self, conn: impl MessageSink, from_entry: &HistoryLogEntry) -> HandleResult {
        let message = message::Message::new(
            &self.source,
            &self.target,
            self.message.message_type,
            &self.message.text,
        )
        .with_tags_from(from_entry);

        // Users should only see their own message echoed if they've asked for it,
        // unless it's sent to themself
        match &self.source {
            update::HistoricMessageSource::User(user) => {
                if conn.user_id() == Some(user.user.id)
                    && !matches!(&self.target, update::HistoricMessageTarget::User(target) if target.user.id == user.user.id)
                {
                    conn.send(message.with_required_capabilities(ClientCapability::EchoMessage));
                } else {
                    conn.send(message);
                }
            }
            _ => conn.send(message),
        }

        Ok(())
    }
}

impl SendHistoryItem for update::NewServer {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        Ok(())
    }
}

impl SendHistoryItem for update::ServerQuit {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        Ok(())
    }
}

impl SendHistoryItem for update::NewAuditLogEntry {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        todo!();
    }
}

impl SendHistoryItem for update::UserLoginChange {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        todo!();
    }
}

impl SendHistoryItem for update::ServicesUpdate {
    fn send_to(&self, _conn: impl MessageSink, _from_entry: &HistoryLogEntry) -> HandleResult {
        Ok(())
    }
}
