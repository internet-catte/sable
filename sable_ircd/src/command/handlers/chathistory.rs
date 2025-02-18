use super::*;
use crate::{capability::ClientCapability, utils};
use messages::send_history::SendHistoryItem;
use sable_network::network::update::HistoricMessageTarget;

use std::cmp::{max, min};

fn parse_msgref(subcommand: &str, target: Option<&str>, msgref: &str) -> Result<i64, CommandError> {
    match msgref.split_once('=') {
        Some(("timestamp", ts)) => utils::parse_timestamp(ts).ok_or_else(|| CommandError::Fail {
            command: "CHATHISTORY",
            code: "INVALID_PARAMS",
            context: subcommand.to_string(),
            description: "Invalid timestamp".to_string(),
        }),
        Some(("msgid", _)) => Err(CommandError::Fail {
            command: "CHATHISTORY",
            code: "INVALID_MSGREFTYPE",
            context: match target {
                Some(target) => format!("{} {}", subcommand, target),
                None => subcommand.to_string(),
            },
            description: "msgid-based history requests are not supported yet".to_string(),
        }),
        _ => Err(CommandError::Fail {
            command: "CHATHISTORY",
            code: "INVALID_MSGREFTYPE",
            context: match target {
                Some(target) => format!("{} {}", subcommand, target),
                None => subcommand.to_string(),
            },
            description: format!("{:?} is not a valid message reference", msgref),
        }),
    }
}

#[allow(clippy::too_many_arguments)]
#[command_handler("CHATHISTORY")]
fn handle_chathistory(
    source: UserSource,
    server: &ClientServer,
    response: &dyn CommandResponse,
    subcommand: &str,
    arg_1: &str,
    arg_2: &str,
    arg_3: &str,
    arg_4: Option<&str>,
) -> CommandResult {
    let source = source.deref();

    match subcommand.to_ascii_uppercase().as_str() {
        "TARGETS" => {
            let from_ts = parse_msgref(subcommand, None, arg_1)?;
            let to_ts = parse_msgref(subcommand, None, arg_2)?;
            let limit = arg_3.parse().ok();

            if limit.is_none() {
                response.send(message::Fail::new(
                    "CHATHISTORY",
                    "INVALID_PARAMS",
                    "",
                    "Invalid limit",
                ));
                return Ok(());
            }

            // The spec allows the from and to timestamps in either order; list_targets requires from < to
            list_targets(
                server,
                response,
                source,
                Some(min(from_ts, to_ts)),
                Some(max(from_ts, to_ts)),
                limit,
            );
        }
        "LATEST" => {
            let target = arg_1;
            let to_ts = match arg_2 {
                "*" => None,
                _ => Some(parse_msgref(subcommand, Some(target), arg_2)?),
            };

            let limit = arg_3.parse().ok();
            if limit.is_none() {
                response.send(message::Fail::new(
                    "CHATHISTORY",
                    "INVALID_PARAMS",
                    "",
                    "Invalid limit",
                ));
                return Ok(());
            }

            send_history_for_target(
                server,
                response,
                source,
                subcommand,
                target,
                None,
                to_ts,
                limit,
                Some(0), // forward limit
            )?;
        }
        "BEFORE" => {
            let target = arg_1;
            let from_ts = parse_msgref(subcommand, Some(target), arg_2)?;

            let limit = arg_3.parse().ok();
            if limit.is_none() {
                response.send(message::Fail::new(
                    "CHATHISTORY",
                    "INVALID_PARAMS",
                    "",
                    "Invalid limit",
                ));
                return Ok(());
            }

            send_history_for_target(
                server,
                response,
                source,
                subcommand,
                target,
                Some(from_ts),
                None,
                limit,
                Some(0), // forward limit
            )?;
        }
        "AFTER" => {
            let target = arg_1;
            let start_ts = parse_msgref(subcommand, Some(target), arg_2)?;

            let limit = arg_3.parse().ok();
            if limit.is_none() {
                response.send(message::Fail::new(
                    "CHATHISTORY",
                    "INVALID_PARAMS",
                    "",
                    "Invalid limit",
                ));
                return Ok(());
            }

            send_history_for_target(
                server,
                response,
                source,
                subcommand,
                target,
                Some(start_ts),
                None,
                Some(0), // backward limit
                limit,
            )?;
        }
        "AROUND" => {
            let target = arg_1;
            let around_ts = parse_msgref(subcommand, Some(target), arg_2)?;

            let limit = match arg_3.parse::<usize>().ok() {
                Some(limit) => limit,
                None => {
                    response.send(message::Fail::new(
                        "CHATHISTORY",
                        "INVALID_PARAMS",
                        "",
                        "Invalid limit",
                    ));
                    return Ok(());
                }
            };

            send_history_for_target(
                server,
                response,
                source,
                subcommand,
                target,
                Some(around_ts),
                None,
                Some(limit / 2), // backward limit
                Some(limit / 2), // forward limit
            )?;
        }
        "BETWEEN" => {
            let target = arg_1;
            let start_ts = parse_msgref(subcommand, Some(target), arg_2)?;
            let end_ts = parse_msgref(subcommand, Some(target), arg_3)?;

            let limit = arg_4.and_then(|arg| arg.parse().ok());
            if limit.is_none() {
                response.send(message::Fail::new(
                    "CHATHISTORY",
                    "INVALID_PARAMS",
                    "",
                    "Invalid limit",
                ));
                return Ok(());
            }

            send_history_for_target(
                server,
                response,
                source,
                subcommand,
                target,
                Some(start_ts),
                Some(end_ts),
                Some(0), // backward limit
                limit,
            )?;
        }
        _ => {
            response.send(message::Fail::new(
                "CHATHISTORY",
                "INVALID_PARAMS",
                subcommand,
                "Invalid subcommand",
            ));
        }
    }

    Ok(())
}

// Helper to extract the target name for chathistory purposes from a given event.
// This might be the source or target of the actual event, or might be None if it's
// an event type that we don't include in history playback
fn target_name_for_entry(for_user: UserId, entry: &HistoryLogEntry) -> Option<String> {
    match &entry.details {
        NetworkStateChange::NewMessage(message) => {
            if matches!(&message.target, HistoricMessageTarget::User(user) if user.user.id == for_user)
            {
                Some(messages::MessageTarget::format(&message.source))
            } else {
                Some(message.target.format())
            }
        }
        _ => None,
    }
}

// For listing targets, we iterate backwards through time; this allows us to just collect the
// first timestamp we see for each target and know that it's the most recent one
fn list_targets(
    server: &ClientServer,
    into: impl MessageSink,
    source: &wrapper::User,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    limit: Option<usize>,
) {
    let log = server.node().history();
    let mut found_targets = HashMap::new();

    for entry in log.entries_for_user_reverse(source.id()) {
        if matches!(to_ts, Some(ts) if entry.timestamp >= ts) {
            // Skip over until we hit the timestamp window we're interested in
            continue;
        }
        if matches!(from_ts, Some(ts) if entry.timestamp <= ts) {
            // We're iterating backwards through time; if we hit this then we've
            // passed the requested window and should stop
            break;
        }

        if let Some(target_name) = target_name_for_entry(source.id(), entry) {
            found_targets.entry(target_name).or_insert(entry.timestamp);
        }

        // If this pushes us past the the requested limit, stop
        if matches!(limit, Some(limit) if limit <= found_targets.len()) {
            break;
        }
    }

    // The appropriate cap here is Batch - chathistory is enabled because we got here,
    // but can be used without batch support.
    let batch = into
        .batch("chathistory-targets", ClientCapability::Batch)
        .start();

    for (target, timestamp) in found_targets {
        batch.send(message::ChatHistoryTarget::new(
            &target,
            &utils::format_timestamp(timestamp),
        ))
    }
}

fn send_history_for_target(
    server: &ClientServer,
    into: impl MessageSink,
    source: &wrapper::User,
    subcommand: &str,
    target: &str,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    backward_limit: Option<usize>,
    forward_limit: Option<usize>,
) -> CommandResult {
    let log = server.node().history();
    let mut backward_entries = Vec::new();
    let mut forward_entries = Vec::new();

    if backward_limit != Some(0) {
        let from_ts = if forward_limit == Some(0) {
            from_ts
        } else {
            // HACK: This is AROUND so we want to capture messages whose timestamp matches exactly
            // (it's a message in the middle of the range)
            from_ts.map(|from_ts| from_ts + 1)
        };

        for entry in log.entries_for_user_reverse(source.id()) {
            if matches!(from_ts, Some(ts) if entry.timestamp >= ts) {
                // Skip over until we hit the timestamp window we're interested in
                continue;
            }
            if matches!(to_ts, Some(ts) if entry.timestamp <= ts) {
                // If we hit this then we've passed the requested window and should stop
                break;
            }

            if let Some(event_target) = target_name_for_entry(source.id(), entry) {
                if event_target == target {
                    backward_entries.push(entry);
                }
            }

            if matches!(backward_limit, Some(limit) if limit <= backward_entries.len()) {
                break;
            }
        }
    }

    if forward_limit != Some(0) {
        for entry in log.entries_for_user(source.id()) {
            if matches!(from_ts, Some(ts) if entry.timestamp <= ts) {
                // Skip over until we hit the timestamp window we're interested in
                continue;
            }
            if matches!(to_ts, Some(ts) if entry.timestamp >= ts) {
                // If we hit this then we've passed the requested window and should stop
                break;
            }

            if let Some(event_target) = target_name_for_entry(source.id(), entry) {
                if event_target == target {
                    forward_entries.push(entry);
                }
            }

            if matches!(forward_limit, Some(limit) if limit <= forward_entries.len()) {
                break;
            }
        }
    }

    send_history_entries(into, subcommand, target, backward_entries, forward_entries)
}

fn send_history_entries<'a>(
    into: impl MessageSink,
    subcommand: &str,
    target: &str,
    backward_entries: Vec<&'a HistoryLogEntry>,
    forward_entries: Vec<&'a HistoryLogEntry>,
) -> CommandResult {
    if backward_entries.is_empty() && forward_entries.is_empty() {
        into.send(message::Fail::new(
            "CHATHISTORY",
            "INVALID_TARGET",
            &format!("{} {}", subcommand, target),
            &format!("Cannot fetch history from {}", target),
        ));
    } else {
        let batch = into
            .batch("chathistory", ClientCapability::Batch)
            .with_arguments(&[target])
            .start();

        // "The order of returned messages within the batch is implementation-defined, but SHOULD be
        // ascending time order or some approximation thereof, regardless of the subcommand used."
        // -- https://ircv3.net/specs/extensions/chathistory#returned-message-notes
        for entry in backward_entries
            .into_iter()
            .rev()
            .chain(forward_entries.into_iter())
        {
            entry.send_to(&batch, entry)?;
        }
    }

    Ok(())
}
