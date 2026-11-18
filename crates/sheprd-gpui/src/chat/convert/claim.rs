//! The panes whose terminal agent is moving into a chat, across every window
//! of this app. Each window runs its own convert job, so without a claim two
//! windows on one daemon could both stop the same agent and both type a
//! resume line, the second into the chat the first started.
//!
//! A claim names the daemon by its boot ID rather than by the window's
//! connection target, since two windows can reach one daemon by different
//! targets (the local session, and its socket named outright). Two daemons
//! sharing a boot ID and a pane ID would only make the second window wait,
//! which is safe; missing a real clash would not be.
//!
//! Only this process is covered: two Sheprd processes on one daemon do not
//! see each other's claims. A move whose claim was released has already
//! typed its line or given up, and the next worker reads the pane afresh
//! from the daemon, which then refuses a pane that runs a chat or no agent.

use std::sync::{Mutex, PoisonError};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    boot: String,
    pane: String,
}

/// The claims held now. A list, as only moves in flight are on it: at most
/// one per window.
#[derive(Debug)]
pub(crate) struct Claims(Mutex<Vec<Key>>);

/// The claims every window of this process shares.
pub(crate) static CLAIMS: Claims = Claims::new();

impl Claims {
    pub(crate) const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    /// Claims `pane` of the daemon that booted as `boot`, unless another
    /// move holds it. The claim lasts until it is dropped.
    pub(crate) fn claim(&self, boot: &str, pane: &str) -> Option<Claim<'_>> {
        let key = Key {
            boot: boot.to_owned(),
            pane: pane.to_owned(),
        };
        // Nothing panics while holding the lock, and the list stays valid
        // whatever a poisoning panic interrupted.
        let mut claimed = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if claimed.contains(&key) {
            return None;
        }
        claimed.push(key.clone());
        Some(Claim { claims: self, key })
    }

    #[cfg(test)]
    pub(crate) fn is_claimed(&self, boot: &str, pane: &str) -> bool {
        let claimed = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        claimed
            .iter()
            .any(|key| key.boot == boot && key.pane == pane)
    }
}

/// One pane claimed for a move; dropping it releases the pane.
#[derive(Debug)]
pub(crate) struct Claim<'a> {
    claims: &'a Claims,
    key: Key,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        let mut claimed = self.claims.0.lock().unwrap_or_else(PoisonError::into_inner);
        claimed.retain(|key| key != &self.key);
    }
}

#[cfg(test)]
mod tests;
