//! Relays shared with this computer by Sheprd on another one, so a relay
//! the user adds once reaches every machine their Sheprd connects to
//! (`docs/relay-plan.md`, Choosing a relay).
//!
//! Each sharing computer is a source, and its offer replaces whatever it
//! shared before, whole: removing a relay there removes it here, and an
//! empty offer withdraws the source. The daemon listens on the relays in
//! its own `[remote] relays` and on every source's, each once. Relays
//! shared here never touch that setting, which stays this computer's own.
//!
//! A private relay's listen token travels with it, so the user's other
//! computers listen there too; it reaches only the machines Sheprd shares
//! with, over their SSH connection.

mod error;
mod token;

pub use error::Error;
pub use token::{ListenToken, MAX_TOKEN, MIN_TOKEN};

use serde::{Deserialize, Serialize};
use sheprd_relay::Endpoint;
use std::collections::BTreeMap;

/// The most computers that may share relays with this one.
pub const MAX_SOURCES: usize = 16;
/// The most relays one source may share.
pub const MAX_RELAYS: usize = 16;
/// The longest source name, in bytes.
pub const MAX_SOURCE_BYTES: usize = 128;

/// What Sheprd sends: the relays it listens on, named for the computer
/// it runs on, and the listen tokens of the private ones, keyed by relay
/// as `relays` gives it. An older daemon ignores the tokens.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    pub source: String,
    pub relays: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tokens: BTreeMap<String, ListenToken>,
}

/// Every source's relays, as the daemon keeps them, and their tokens by
/// source, then by endpoint.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Shared {
    sources: BTreeMap<String, Vec<Endpoint>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tokens: BTreeMap<String, BTreeMap<String, ListenToken>>,
}

impl Shared {
    /// Takes `offer`'s relays in place of what its source shared before.
    /// Returns whether anything changed; refuses an offer it cannot keep
    /// and leaves the rest as it was.
    pub fn offer(&mut self, offer: Offer) -> Result<bool, Error> {
        let source = offer.source.trim();
        if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err(Error::Source);
        }
        let mut relays: Vec<Endpoint> = Vec::new();
        let mut tokens = BTreeMap::new();
        for relay in offer.relays {
            let endpoint = Endpoint::parse(&relay).map_err(|source| Error::Endpoint {
                relay: relay.clone(),
                source,
            })?;
            if relays.len() < MAX_RELAYS && !relays.contains(&endpoint) {
                // A token counts only beside the relay it is for.
                if let Some(token) = offer.tokens.get(&relay) {
                    tokens.insert(endpoint.to_string(), token.clone());
                }
                relays.push(endpoint);
            }
        }
        if relays.is_empty() {
            let had_tokens = self.tokens.remove(source).is_some();
            return Ok(self.sources.remove(source).is_some() || had_tokens);
        }
        if !self.sources.contains_key(source) && self.sources.len() >= MAX_SOURCES {
            return Err(Error::TooManySources);
        }
        let before = self.sources.insert(source.to_owned(), relays.clone());
        let tokens_before = if tokens.is_empty() {
            self.tokens.remove(source)
        } else {
            self.tokens.insert(source.to_owned(), tokens.clone())
        };
        Ok(before.as_ref() != Some(&relays) || tokens_before.unwrap_or_default() != tokens)
    }

    /// The listen token some source shared for `endpoint`, if any.
    pub fn token(&self, endpoint: &Endpoint) -> Option<&ListenToken> {
        let key = endpoint.to_string();
        self.tokens.values().find_map(|tokens| tokens.get(&key))
    }

    /// Every source's relays, each once, by source name.
    pub fn endpoints(&self) -> Vec<Endpoint> {
        let mut all: Vec<Endpoint> = Vec::new();
        for endpoint in self.sources.values().flatten() {
            if !all.contains(endpoint) {
                all.push(endpoint.clone());
            }
        }
        all
    }

    /// This copy within bounds, as a file read back may not be.
    pub fn bounded(self) -> Self {
        let sources = self
            .sources
            .into_iter()
            .filter(|(source, relays)| {
                !source.trim().is_empty() && source.len() <= MAX_SOURCE_BYTES && !relays.is_empty()
            })
            .take(MAX_SOURCES)
            .map(|(source, mut relays)| {
                relays.truncate(MAX_RELAYS);
                (source, relays)
            })
            .collect::<BTreeMap<_, _>>();
        // Only tokens for relays a source still shares.
        let tokens = self
            .tokens
            .into_iter()
            .filter_map(|(source, tokens)| {
                let relays = sources.get(&source)?;
                let tokens: BTreeMap<_, _> = tokens
                    .into_iter()
                    .filter(|(endpoint, _)| {
                        relays.iter().any(|relay| relay.to_string() == *endpoint)
                    })
                    .collect();
                (!tokens.is_empty()).then_some((source, tokens))
            })
            .collect();
        Self { sources, tokens }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
