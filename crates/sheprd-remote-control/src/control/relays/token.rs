//! A relay's listen token: the operator's secret that keeps a private
//! relay to their own computers (`sheprd_relay::hub::access`). Dialers
//! never need one, so it stays on the computers that listen.

use super::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The shortest token a relay accepts.
pub const MIN_TOKEN: usize = 16;
/// The longest the relay's wire can carry.
pub const MAX_TOKEN: usize = 255;

/// A listen token, checked as a relay would take it. `Debug` never shows it.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ListenToken(String);

impl ListenToken {
    /// `text`, trimmed, when it could be a relay's listen token: 16 to 255
    /// bytes of printable ASCII without spaces, as the relay's token file
    /// would hold it.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim();
        if !(MIN_TOKEN..=MAX_TOKEN).contains(&text.len())
            || !text.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::Token);
        }
        Ok(Self(text.to_owned()))
    }

    /// The token, to send to its relay.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ListenToken {
    type Error = Error;

    fn try_from(text: String) -> Result<Self, Error> {
        Self::parse(&text)
    }
}

impl From<ListenToken> for String {
    fn from(token: ListenToken) -> Self {
        token.0
    }
}

impl fmt::Debug for ListenToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ListenToken(…)")
    }
}
