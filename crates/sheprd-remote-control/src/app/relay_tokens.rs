//! This computer's listen tokens for private relays, kept out of the
//! config, in `STATE_DIR/relay/tokens.json`, readable by this user alone
//! (`docs/relay-plan.md`, Choosing a relay). Sheprd writes the file and
//! asks the daemon to read it again; with no daemon running, the daemon
//! reads it when it starts.

use super::edited::{locked, save};
use crate::control::{self, Request, Response, relays::ListenToken};
use serde::{Deserialize, Serialize};
use sheprd_relay::Endpoint;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// The most relays one computer keeps a token for.
pub const MAX_TOKENS: usize = 64;

/// Where the daemon whose state is `state_dir` keeps its tokens.
pub fn file(state_dir: &Path) -> PathBuf {
    state_dir.join("relay").join("tokens.json")
}

/// The tokens, keyed by endpoint as [`Endpoint`] writes it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    tokens: BTreeMap<String, ListenToken>,
}

impl Tokens {
    /// The token for `endpoint`, if this computer has one.
    pub fn get(&self, endpoint: &Endpoint) -> Option<&ListenToken> {
        self.tokens.get(&endpoint.to_string())
    }

    /// The token for the relay at `relay`, however the config spells it.
    pub fn for_relay(&self, relay: &str) -> Option<&ListenToken> {
        self.get(&Endpoint::parse(relay).ok()?)
    }

    /// Keeps `token` for `endpoint`, or with `None` forgets it. Refuses a
    /// new one past [`MAX_TOKENS`].
    pub fn set(&mut self, endpoint: &Endpoint, token: Option<ListenToken>) -> super::Result<()> {
        let key = endpoint.to_string();
        match token {
            Some(token) => {
                if !self.tokens.contains_key(&key) && self.tokens.len() >= MAX_TOKENS {
                    return Err(super::Error::RelayTokens { max: MAX_TOKENS });
                }
                self.tokens.insert(key, token);
            }
            None => {
                self.tokens.remove(&key);
            }
        }
        Ok(())
    }

    /// This copy within bounds, with only keys that are endpoints, as a
    /// file read back may not be.
    fn bounded(self) -> Self {
        let tokens = self
            .tokens
            .into_iter()
            .filter_map(|(key, token)| Some((Endpoint::parse(&key).ok()?.to_string(), token)))
            .take(MAX_TOKENS)
            .collect();
        Self { tokens }
    }
}

/// The tokens saved in `file`; none when it is missing or unreadable, which
/// costs only listening on a private relay until its token is given again.
/// Blocking.
pub fn load(file: &Path) -> Tokens {
    fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Tokens>(&bytes).ok())
        .map(Tokens::bounded)
        .unwrap_or_default()
}

/// Keeps `token` for `endpoint` in `state_dir`, or with `None` forgets it,
/// then has a running daemon listen with it at once. Blocking.
pub fn set(state_dir: &Path, endpoint: &Endpoint, token: Option<ListenToken>) -> super::Result<()> {
    let file = file(state_dir);
    let dir = file.parent().unwrap_or(state_dir);
    fs::create_dir_all(dir).map_err(|source| super::Error::Io {
        operation: "create",
        path: dir.to_owned(),
        source,
    })?;
    locked(&file, || {
        let mut tokens = load(&file);
        tokens.set(endpoint, token)?;
        save(&file, &tokens)
    })?;
    match control::request(&control::address(state_dir), &Request::RelayTokens) {
        Ok(Response::Done) | Err(control::Error::Connect(_)) => Ok(()),
        // A daemon older than tokens reads the file when it next starts.
        Ok(Response::Error { .. }) => Ok(()),
        Ok(_) => Err(super::Error::Unexpected),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
