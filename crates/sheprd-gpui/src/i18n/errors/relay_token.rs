//! Why a relay's listen token could not be checked or kept, from
//! `error-relay-token.ftl`.

use super::Localize;
use crate::{i18n::fl, remote::relay_token::Error};

impl Localize for Error {
    fn localize(&self) -> String {
        match self {
            // keep-sorted start
            Self::Answer(_) => fl!("error-relay-token-answer"),
            Self::Endpoint(_) => fl!("error-relay-token-endpoint"),
            // The transport's own reason is shown as it is.
            Self::Info(source) => fl!("error-relay-token-info", error = source.to_string()),
            Self::Protocol(protocol) => {
                fl!(
                    "error-relay-token-protocol",
                    protocol = protocol.to_string()
                )
            }
            Self::Save(error) => error.localize(),
            // keep-sorted end
        }
    }
}
