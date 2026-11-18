//! The listen token `sheprd remote relays token` reads from standard
//! input. It is never an argument, so it stays out of shell history and
//! the process list, and nothing here prints or logs it.

mod error;

pub(crate) use error::Error;

use sheprd_remote_control::control::relays::ListenToken;
use std::io::Read;

/// The most read: a token is at most 255 bytes, so this leaves room for a
/// line ending or a token file's whitespace and refuses anything else.
pub(crate) const MAX_INPUT: usize = 1024;

/// The token in `input`, trimmed: at most [`MAX_INPUT`] bytes of UTF-8
/// that [`ListenToken::parse`] accepts.
pub(crate) fn read(input: impl Read) -> Result<ListenToken, Error> {
    let mut bytes = Vec::new();
    input
        .take(MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(Error::Read)?;
    if bytes.len() > MAX_INPUT {
        return Err(Error::TooLong);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::Text)?;
    if text.trim().is_empty() {
        return Err(Error::Empty);
    }
    Ok(ListenToken::parse(text)?)
}

#[cfg(test)]
mod tests;
