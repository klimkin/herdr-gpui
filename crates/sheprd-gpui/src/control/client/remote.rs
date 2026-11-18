//! `remote`: the remote control daemon's status, devices, and restart, the
//! window's pairing and revoke dialogs, and a private relay's listen token.
use super::{Context, Output, unexpected};
use crate::{
    cli::control::RemoteCommand,
    control::protocol::{
        Request, Response,
        remote::{RemoteOp, Reply},
    },
};
use sheprd_remote_control::control::{Device, Status};
use std::io::IsTerminal as _;

pub(super) fn remote(command: RemoteCommand, context: &Context) -> crate::Result<Output> {
    let op = match command {
        RemoteCommand::Status => RemoteOp::Status,
        RemoteCommand::Pair { address } => RemoteOp::Pair { address },
        RemoteCommand::Devices => RemoteOp::Devices,
        RemoteCommand::Revoke { device } => RemoteOp::Revoke { device },
        RemoteCommand::Restart => RemoteOp::Restart,
        RemoteCommand::RelayToken { relay, clear } => RemoteOp::RelayToken {
            relay: relay.to_string(),
            token: if clear { None } else { Some(piped_token()?) },
        },
    };
    match context.call(Request::Remote, op)? {
        Response::Done { message } => Ok(Output::message(message)),
        Response::Remote { reply } => output(&reply),
        response => Err(unexpected(response)),
    }
}

/// The listen token piped on standard input. A terminal is refused: what
/// is typed there would be shown on screen.
fn piped_token() -> crate::Result<sheprd_remote_control::control::relays::ListenToken> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err(crate::control::listen_token::Error::Terminal.into());
    }
    Ok(crate::control::listen_token::read(stdin.lock())?)
}

/// What a reply prints, as text and as JSON.
pub(super) fn output(reply: &Reply) -> crate::Result<Output> {
    match reply {
        Reply::Status {
            enabled,
            status,
            error,
        } => {
            let mut json = serde_json::json!({
                "enabled": enabled,
                "running": status.is_some(),
                "status": status,
                "error": error,
            });
            // The daemon leaves `token` out when false; it is spelled out
            // here so a script need not know that.
            // `pointer_mut`, as indexing would turn a missing status into `{}`.
            if let Some(relays) = json
                .pointer_mut("/status/relays")
                .and_then(serde_json::Value::as_object_mut)
            {
                for relay in relays
                    .values_mut()
                    .filter_map(|relay| relay.as_object_mut())
                {
                    relay.entry("token").or_insert(false.into());
                }
            }
            Output::data(
                status_text(*enabled, status.as_deref(), error.as_deref()),
                &json,
            )
        }
        Reply::Devices { devices } => {
            let json = serde_json::json!({ "devices": devices });
            if devices.is_empty() {
                return Ok(Output::data("No paired devices", &json)?.nothing());
            }
            let now = crate::remote::now_ms();
            let lines: Vec<String> = devices.iter().map(|device| line(device, now)).collect();
            Output::data(lines.join("\n"), &json)
        }
        Reply::RelayToken { relay, token } => {
            let json = serde_json::json!({ "relay": relay, "token": token });
            let text = if *token {
                format!("Kept this computer's listen token for {relay}")
            } else {
                format!("Forgot this computer's listen token for {relay}")
            };
            Output::data(text, &json)
        }
    }
}

fn status_text(enabled: bool, status: Option<&Status>, error: Option<&str>) -> String {
    let Some(status) = status else {
        return match (enabled, error) {
            (false, _) => "Remote control is off".to_owned(),
            (true, Some(error)) => format!("Remote control is on but not running: {error}"),
            (true, None) => "Remote control is on but not running".to_owned(),
        };
    };
    let mut lines = vec![
        format!(
            "Running on port {} (version {})",
            status.port, status.version
        ),
        format!(
            "Devices: {} paired, {} connected",
            status.devices, status.connected
        ),
        format!("Chats: {}", status.chats),
    ];
    if !status.herdr {
        lines.push("Herdr is not running on this computer".to_owned());
    }
    if !status.fingerprint.is_empty() {
        lines.push(format!("Certificate authority: {}", status.fingerprint));
    }
    lines.extend(status.addresses.iter().map(
        |address| match status.address_details.get(address) {
            Some(details) => format!(
                "Address: {address} — {}{}",
                details.kind.label(),
                if details.interfaces.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", details.interfaces.join(", "))
                }
            ),
            None => format!("Address: {address}"),
        },
    ));
    lines.extend(
        status
            .relays
            .iter()
            .map(|(relay, status)| relay_line(relay, status)),
    );
    lines.join("\n")
}

/// One relay: what it is called and where, how it stands, then its
/// description and tags on lines of their own.
fn relay_line(relay: &str, status: &sheprd_remote_control::control::RelayStatus) -> String {
    use sheprd_remote_control::control::RelayState;
    let label = &status.label;
    let state = match &status.state {
        RelayState::Connecting => "connecting".to_owned(),
        RelayState::Listening { rtt_ms: None } => "listening".to_owned(),
        RelayState::Listening {
            rtt_ms: Some(rtt_ms),
        } => format!("listening, {rtt_ms} ms"),
        RelayState::Failed { error } => format!("failed: {error}"),
    };
    let state = if status.token {
        format!("{state}, with a listen token")
    } else {
        state
    };
    let mut line = match &label.name {
        Some(name) => format!("Relay: {name} ({relay}) — {state}"),
        None => format!("Relay: {relay} — {state}"),
    };
    if let Some(description) = &label.description {
        line.push_str("\n  ");
        line.push_str(description);
    }
    if !label.tags.is_empty() {
        line.push_str("\n  Tags: ");
        line.push_str(&label.tags.join(", "));
    }
    line
}

/// One device as a line of text: its ID for `remote revoke`, its name, and
/// how it stands.
pub(super) fn line(device: &Device, now_ms: u64) -> String {
    let seen = match device.last_seen_ms {
        _ if device.connected => "connected".to_owned(),
        Some(seen) => format!(
            "last seen {}",
            crate::ago::english(now_ms.saturating_sub(seen) / 1000)
        ),
        None => "never connected".to_owned(),
    };
    let push = if device.push {
        "notifications on"
    } else {
        "notifications off"
    };
    let line = format!("{}  {}  {seen}  {push}", device.id.0, device.name);
    match crate::remote::hosts_line(device) {
        Some(hosts) => format!("{line}  {hosts}"),
        None => line,
    }
}
