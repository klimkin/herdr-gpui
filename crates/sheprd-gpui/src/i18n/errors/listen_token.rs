//! A listen token piped to `sheprd remote relays token`:
//! `error-listen-token.ftl`.

use super::Localize;
use crate::{control::listen_token::Error, i18n::fl};

impl Localize for Error {
    fn localize(&self) -> String {
        match self {
            // keep-sorted start
            Self::Empty => fl!("error-listen-token-empty"),
            Self::Read(_) => fl!("error-listen-token-read"),
            Self::Terminal => fl!("error-listen-token-terminal"),
            Self::Text => fl!("error-listen-token-text"),
            Self::Token(_) => fl!("error-listen-token-token"),
            Self::TooLong => fl!("error-listen-token-too-long"),
            // keep-sorted end
        }
    }
}
