//! `[chat] auto_convert`: moving each newly started terminal agent into a
//! chat on its own, as `chat convert` would, once it has settled.
//!
//! Only an agent that appears after the window first saw its daemon counts
//! as new, so turning the setting on, restarting the app, or reconnecting
//! moves nothing already running. A pane is tried at most once: a refusal
//! leaves its terminal, and a chat moved back to its terminal stays there.
//! The agent must have been idle, with no change Herdr reports, for
//! [`SETTLE`], so a tool that prompts the agent as soon as it is ready (as
//! `herdr agent start` followed by `herdr agent prompt` does) is never cut
//! off: its prompt makes the agent work, which starts the wait again.
//! Typing into the pane from this window, or a `sheprd_terminal=keep` token
//! on the pane, keeps its terminal. One agent moves at a time, so many
//! started together move one after another rather than all at once.

use super::convert;
use crate::SheprdWindow;
use sheprd_client::{
    herdr::convert::{Error as Refusal, movable},
    protocol::{AgentStatus, ClientShellSnapshot},
};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

/// How long a new agent must stay idle and unchanged before it moves. Longer
/// than an automation takes to send its first prompt once the agent is ready.
pub(crate) const SETTLE: Duration = Duration::from_secs(10);
/// The pane token a tool sets (`herdr pane report-metadata PANE --source ID
/// --token sheprd_terminal=keep`) to keep a pane's agent in its terminal.
pub(crate) const KEEP_TOKEN: (&str, &str) = ("sheprd_terminal", "keep");
/// How many times a pane whose agent turned busy just as it was to move is
/// watched again, before its terminal is left alone.
pub(crate) const RETRIES: u8 = 2;

/// The daemon an answer belongs to: its boot, and the window's selection
/// and connection counters, as [`super::existing::Scope`] has them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Scope {
    pub(crate) boot: String,
    pub(crate) selection: u64,
    pub(crate) connection: u64,
}

/// A new agent waiting to settle: the change counter it showed, and since
/// when it has shown it idle.
#[derive(Clone, Copy, Debug)]
struct Watch {
    seq: u64,
    idle_since: Option<Instant>,
}

/// What one tick knows beyond the snapshot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tick<'a> {
    pub(crate) now: Instant,
    /// Whether a move is already under way, which the next one waits for.
    pub(crate) busy: bool,
    /// When this window last typed, composed, or pasted, and into which
    /// pane. Pointer motion, wheels, and clicks do not count: they leave no
    /// draft, and the window notes them for any pane it shows.
    pub(crate) last_typed: Option<&'a Typed>,
}

/// Typing, composing, or pasting into a pane: what can leave a draft there.
/// It names the pane that had Herdr's focus then, so typing into one pane
/// and clicking another later does not count against the second.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Typed {
    pub(crate) at: Instant,
    pub(crate) pane: Option<String>,
}

impl Typed {
    /// Whether this typing went into `pane` at or after `since`.
    pub(crate) fn went_into(&self, pane: &str, since: Instant) -> bool {
        self.pane.as_deref() == Some(pane) && self.at >= since
    }
}

/// The new agents of one daemon, and the panes that are done with.
#[derive(Debug, Default)]
pub(crate) struct Tracker {
    scope: Option<Scope>,
    /// Times each pane was watched again after a busy refusal.
    retried: HashMap<String, u8>,
    /// Panes not to move: running at the baseline, tried, or kept.
    settled: HashSet<String>,
    watching: HashMap<String, Watch>,
}

impl Tracker {
    /// Forgets everything, so the next scope starts from a new baseline.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Watches `pane` again after its agent was found busy as it was to
    /// move, unless it has been [`RETRIES`] times. False when it stays.
    pub(crate) fn retry(&mut self, pane: &str) -> bool {
        let retried = self.retried.entry(pane.to_owned()).or_default();
        if *retried >= RETRIES {
            return false;
        }
        *retried += 1;
        self.settled.remove(pane)
    }

    /// The pane to move now, if one is due. It is settled at once, so it
    /// is never tried again whatever the move's outcome.
    pub(crate) fn poll(
        &mut self,
        scope: Scope,
        snapshot: &ClientShellSnapshot,
        tick: Tick<'_>,
    ) -> Option<String> {
        if self.scope.as_ref() != Some(&scope) {
            // What runs when the window first sees a daemon is not new.
            self.scope = Some(scope);
            self.watching.clear();
            self.settled = snapshot
                .agents
                .iter()
                .map(|agent| agent.pane_id.clone())
                .collect();
            return None;
        }
        // A pane that leaves the agent list for a moment, as one does while
        // its agent stops or a chat moves back, keeps its place; only a
        // closed pane is forgotten.
        self.settled
            .retain(|pane| snapshot.panes.iter().any(|known| &known.pane_id == pane));
        self.watching
            .retain(|pane, _| snapshot.agents.iter().any(|agent| &agent.pane_id == pane));
        self.retried
            .retain(|pane, _| snapshot.panes.iter().any(|known| &known.pane_id == pane));
        let mut due = None;
        for agent in &snapshot.agents {
            let pane = &agent.pane_id;
            if self.settled.contains(pane) {
                continue;
            }
            let kept = agent
                .tokens
                .iter()
                .any(|(name, value)| (name.as_str(), value.as_str()) == KEEP_TOKEN);
            match movable(agent) {
                _ if kept => {
                    self.settled.insert(pane.clone());
                    continue;
                }
                Ok(_) => {}
                // An agent not yet named may still become one that moves.
                Err(Refusal::NoAgent) => continue,
                Err(_) => {
                    self.settled.insert(pane.clone());
                    continue;
                }
            }
            let idle = matches!(agent.agent_status, AgentStatus::Idle | AgentStatus::Done);
            let watch = self.watching.entry(pane.clone()).or_insert(Watch {
                seq: agent.state_change_seq,
                idle_since: None,
            });
            if watch.seq != agent.state_change_seq || !idle {
                *watch = Watch {
                    seq: agent.state_change_seq,
                    idle_since: idle.then_some(tick.now),
                };
                continue;
            }
            let since = *watch.idle_since.get_or_insert(tick.now);
            if tick
                .last_typed
                .is_some_and(|typed| typed.went_into(pane, since))
            {
                // Someone is typing in this terminal: it is theirs.
                self.settled.insert(pane.clone());
                self.watching.remove(pane);
                continue;
            }
            if due.is_none() && !tick.busy && tick.now.duration_since(since) >= SETTLE {
                due = Some(pane.clone());
            }
        }
        if let Some(pane) = &due {
            self.settled.insert(pane.clone());
            self.watching.remove(pane);
        }
        due
    }
}

impl SheprdWindow {
    /// Runs every tick: starts moving the next settled new agent into a
    /// chat when `[chat] auto_convert` is on. A refusal is logged, not
    /// shown, since nobody asked for this move; the terminal stays.
    pub(super) fn poll_chat_auto_convert(&mut self, now: Instant) {
        let enabled = self.config.chat.auto_convert
            && self.config.features.acp_chats
            && self.live.status.is_connected();
        let (Some(snapshot), Some(endpoint), true) = (
            self.live.snapshot.as_deref(),
            self.endpoints.get(self.selected_endpoint),
            enabled,
        ) else {
            self.chats.auto_convert.reset();
            return;
        };
        let scope = Scope {
            boot: snapshot.boot_id.clone(),
            selection: self.selection_epoch,
            connection: endpoint.generation,
        };
        let tick = Tick {
            now,
            busy: self.chats.converting.is_some() || self.chats.converting_core.is_some(),
            last_typed: self.last_typed.as_ref(),
        };
        let Some(pane) = self.chats.auto_convert.poll(scope, snapshot, tick) else {
            return;
        };
        match self.convert_to_chat(Some(&pane)) {
            Ok((_, agent)) => {
                if let Some(job) = &mut self.chats.converting {
                    job.automatic = true;
                }
                tracing::info!(pane, agent = agent.name(), "Moving a new agent into a chat");
            }
            Err(error) => tracing::info!(pane, %error, "A new agent stays in its terminal"),
        }
    }

    /// Stops the automatic move of `pane` if its worker has not pressed
    /// Ctrl+C yet, since what was just typed there is a draft Ctrl+C would
    /// clear. The worker then hands back its refusal, and the pane stays in
    /// its terminal. A move past its first key finishes: the agent is on its
    /// way out, and only its chat can continue the conversation.
    pub(crate) fn typed_into(&mut self, pane: &str) {
        let Some(job) = &self.chats.converting else {
            return;
        };
        if !job.automatic || job.pane != pane {
            return;
        }
        if job.stage.typed() {
            tracing::info!(pane, "Typing into a new agent keeps it in its terminal");
        } else {
            tracing::debug!(
                pane,
                "Typed into an agent already stopping; its move finishes"
            );
        }
    }

    /// A move [`Self::poll_chat_auto_convert`] started that did not finish.
    /// An agent that turned busy before it was stopped is watched again, as
    /// when a tool prompted it at that moment; anything else is logged and
    /// leaves the terminal.
    pub(super) fn auto_convert_failed(&mut self, pane: &str, error: &convert::Error) {
        let busy = matches!(error, convert::Error::Convert(Refusal::Busy(_)));
        if busy && self.chats.auto_convert.retry(pane) {
            tracing::debug!(
                pane,
                "A new agent turned busy; waiting for it to settle again"
            );
            return;
        }
        tracing::info!(pane, %error, "A new agent stays in its terminal");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
