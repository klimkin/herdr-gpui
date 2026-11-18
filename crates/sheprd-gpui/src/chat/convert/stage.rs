//! How far a move's worker has gone, for typing that may stop an automatic
//! move. The worker commits before the first thing that acts on the pane:
//! its first Ctrl+C, or, when the shell is already in front because the
//! agent quit on its own, the resume line. Typing into the pane before that
//! keeps the draft and the terminal, so the move acts on nothing; typing
//! after it changes nothing, since the agent is already on its way out and
//! only the chat can continue its conversation.
//!
//! One atomic step decides which came first, so the worker never acts after
//! the window was told the move stopped, and the window never stops a move
//! that has acted.

use std::sync::atomic::{AtomicU8, Ordering};

const PREPARING: u8 = 0;
const COMMITTED: u8 = 1;
const TYPED: u8 = 2;

#[derive(Debug, Default)]
pub(crate) struct Stage(AtomicU8);

impl Stage {
    /// The worker is about to act on the pane: press Ctrl+C, or hand back
    /// its line. False when typing reached the pane first, and it may not.
    pub(in crate::chat) fn commit(&self) -> bool {
        match self
            .0
            .compare_exchange(PREPARING, COMMITTED, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(stage) => stage == COMMITTED,
        }
    }

    /// Typing reached the pane. True when the worker had not acted, and now
    /// never will; false when the move has acted and finishes.
    pub(crate) fn typed(&self) -> bool {
        self.0
            .compare_exchange(PREPARING, TYPED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Whether typing stopped the move before it acted.
    #[cfg(test)]
    pub(crate) fn stopped_by_typing(&self) -> bool {
        self.0.load(Ordering::Acquire) == TYPED
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
