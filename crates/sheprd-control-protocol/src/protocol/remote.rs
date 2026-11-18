//! Remote control: the daemon's status and devices, the pairing dialog, the
//! revoke confirmation, restarting the daemon, and keeping a private
//! relay's listen token. The daemon's own types are reused as they are;
//! no pairing token ever crosses this socket, and a listen token crosses
//! it only from the caller, once, to be kept: no answer carries it back.
use serde::{Deserialize, Serialize};
use sheprd_remote_control::control::{Device, Status, relays::ListenToken};

/// Asking the daemon, and for `revoke` the device list first, takes up to
/// its own answer timeout a request.
const DAEMON_WAIT_SECONDS: u64 = 12;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum RemoteOp {
    Status,
    /// Opens the pairing dialog in the window.
    Pair {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address: Option<String>,
    },
    Devices,
    /// Opens the confirmation for revoking a device, by ID or name.
    Revoke {
        device: String,
    },
    /// Restarts the daemon from the binary beside the app.
    Restart,
    /// Keeps this computer's listen token for the relay at `relay`, or
    /// without one forgets it. `Debug` never shows the token.
    RelayToken {
        relay: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<ListenToken>,
    },
}

impl RemoteOp {
    pub fn wait_seconds(&self) -> u64 {
        match self {
            Self::Status | Self::Devices | Self::Revoke { .. } | Self::RelayToken { .. } => {
                DAEMON_WAIT_SECONDS
            }
            Self::Pair { .. } | Self::Restart => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    Status {
        /// Whether remote control is on in Settings.
        enabled: bool,
        /// What the daemon says, when it answers.
        #[serde(default)]
        status: Option<Box<Status>>,
        /// Why it did not, when it is on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Devices {
        devices: Vec<Device>,
    },
    /// A listen token was kept or forgotten for `relay`, as the app
    /// normalized its address; `token` is whether one is now kept.
    RelayToken {
        relay: String,
        token: bool,
    },
}

#[cfg(test)]
mod tests;
