//! `remote`: remote control from a phone.
use super::{Args, CliError, unknown};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteCommand {
    /// Whether the daemon runs, where, and who is connected.
    Status,
    /// Open the pairing dialog; never prints a token.
    Pair {
        address: Option<String>,
    },
    Devices,
    /// Open the confirmation for revoking a device, by ID or name.
    Revoke {
        device: String,
    },
    /// Restart the daemon from the binary beside the app.
    Restart,
    /// Keep this computer's listen token for a relay, read from standard
    /// input when the command runs, or with `clear` forget it. The token
    /// is never an argument, so it stays out of shell history and `ps`.
    RelayToken {
        relay: sheprd_relay::Endpoint,
        clear: bool,
    },
}

pub(super) fn remote(args: &mut Args) -> Result<RemoteCommand, CliError> {
    let Some(verb) = args.verb() else {
        return Ok(RemoteCommand::Status);
    };
    Ok(match verb.to_str() {
        Some("status") => RemoteCommand::Status,
        Some("pair") => RemoteCommand::Pair {
            address: args.text_option("--address")?,
        },
        Some("devices" | "list") => RemoteCommand::Devices,
        Some("restart") => RemoteCommand::Restart,
        // A name may be typed unquoted, as `remote revoke Fabien's iPhone`.
        Some("revoke") => RemoteCommand::Revoke {
            device: args.words()?.ok_or(CliError::MissingArgument {
                command: "remote revoke",
                what: "DEVICE",
            })?,
        },
        Some("relays") => relays(args)?,
        _ => return Err(unknown("remote", verb)),
    })
}

/// `remote relays token ENDPOINT [--clear]`.
fn relays(args: &mut Args) -> Result<RemoteCommand, CliError> {
    let verb = args.verb().ok_or(CliError::MissingArgument {
        command: "remote relays",
        what: "token",
    })?;
    if verb != "token" {
        return Err(unknown("remote relays", verb));
    }
    let clear = args.flag("--clear");
    let relay = args.positional().ok_or(CliError::MissingArgument {
        command: "remote relays token",
        what: "ENDPOINT",
    })?;
    let relay = sheprd_relay::Endpoint::parse(&crate::cli::args::utf8(relay)?)
        .map_err(|_| CliError::InvalidValue("relay address"))?;
    // A second word is most likely the token itself: refused without
    // echoing it, as the leftover check would.
    if args.positional().is_some() {
        return Err(CliError::InvalidValue(
            "argument: pipe the listen token on standard input instead",
        ));
    }
    Ok(RemoteCommand::RelayToken { relay, clear })
}
