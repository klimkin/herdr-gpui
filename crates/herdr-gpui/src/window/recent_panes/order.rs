//! The window's panes on every endpoint, most recently focused first, the way
//! an editor orders its recent files. The order is client-local and observed
//! from each endpoint's snapshots, and a pane ID never outlives its boot.

use crate::endpoint::Step;
use gpui::Modifiers;
use herdr_client::protocol::ClientShellSnapshot;
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PaneKey {
    pub(crate) endpoint: String,
    pub(crate) pane: String,
}

/// What the window knows of one endpoint: its snapshot, or `None` while it is
/// disconnected or disabled, when its panes are kept but cannot be reached.
#[derive(Clone, Copy)]
pub(crate) struct Projection<'a> {
    pub(crate) endpoint: &'a str,
    pub(crate) snapshot: Option<&'a ClientShellSnapshot>,
}

#[derive(Debug, Default)]
pub(crate) struct RecentPanes {
    /// The boot each endpoint's entries belong to.
    boots: HashMap<String, String>,
    /// While a cycle is open the order is frozen, so the panes it passes
    /// through are not recorded and `cursor` keeps indexing the same pane.
    cycle: Option<Cycle>,
    /// Known panes, the focused one first.
    order: VecDeque<PaneKey>,
}

#[derive(Clone, Copy, Debug)]
struct Cycle {
    /// The entry the last press went to.
    cursor: usize,
    /// The modifiers the cycle lasts for: releasing any of them commits.
    hold: Modifiers,
}

impl RecentPanes {
    /// How many panes the order remembers; older ones fall off the back.
    pub(crate) const LIMIT: usize = 100;

    /// Forgets the panes of endpoints that restarted or are gone and records
    /// `focused` as the most recent pane. Only the selected endpoint's focus
    /// is passed: another endpoint's focus moves for other clients, not this
    /// user.
    pub(crate) fn observe(&mut self, projections: &[Projection], focused: Option<PaneKey>) {
        for projection in projections {
            let Some(snapshot) = projection.snapshot else {
                continue;
            };
            match self.boots.get(projection.endpoint) {
                Some(boot) if *boot == snapshot.boot_id => continue,
                Some(_) => self.forget(projection.endpoint),
                None => {}
            }
            self.boots
                .insert(projection.endpoint.to_owned(), snapshot.boot_id.clone());
        }
        let present = |endpoint: &str| projections.iter().any(|p| p.endpoint == endpoint);
        self.boots.retain(|endpoint, _| present(endpoint));
        while let Some(gone) = self.order.iter().find(|key| !present(&key.endpoint)) {
            let gone = gone.endpoint.clone();
            self.forget(&gone);
        }
        if self.cycle.is_some() {
            return;
        }
        self.order.retain(|key| {
            projections
                .iter()
                .find(|p| p.endpoint == key.endpoint)
                .and_then(|p| p.snapshot)
                .is_none_or(|snapshot| open(snapshot, &key.pane))
        });
        if let Some(focused) = focused {
            self.promote(focused);
        }
    }

    /// The entry the next press goes to and its pane, without going: `Back`
    /// is older, `Forward` newer, and either wraps around. `current` and panes
    /// that cannot be reached are stepped over.
    pub(crate) fn peek(
        &self,
        step: Step,
        projections: &[Projection],
        current: Option<&PaneKey>,
    ) -> Option<(usize, PaneKey)> {
        let count = self.order.len();
        let cursor = self.cycle.map(|cycle| cycle.cursor);
        (1..=count)
            .map(|offset| match (step, cursor) {
                (Step::Back, Some(cursor)) => (cursor + offset) % count,
                // The front is usually `current`, but a focus that has not
                // been observed yet leaves something else there to go back to.
                (Step::Back, None) => offset - 1,
                (Step::Forward, cursor) => (cursor.unwrap_or(0) + count - offset) % count,
            })
            .find_map(|index| {
                let key = self.order.get(index)?;
                (current != Some(key) && self.reachable(projections, key))
                    .then(|| (index, key.clone()))
            })
    }

    /// The press to `index` was queued. A press that opens a cycle holds it
    /// for as long as `modifiers` stay down, apart from Shift, which only
    /// picks the direction. Without any, as from the palette or a menu, the
    /// press commits at once.
    pub(crate) fn begin(&mut self, index: usize, modifiers: Modifiers) {
        if let Some(cycle) = &mut self.cycle {
            cycle.cursor = index;
            return;
        }
        let hold = Modifiers {
            shift: false,
            ..modifiers
        };
        self.cycle = Some(Cycle {
            cursor: index,
            hold,
        });
        if !hold.modified() {
            self.finish();
        }
    }

    /// Commits the cycle once a modifier it holds is up.
    pub(crate) fn release(&mut self, modifiers: Modifiers) {
        if self
            .cycle
            .is_some_and(|cycle| !cycle.hold.is_subset_of(&modifiers))
        {
            self.finish();
        }
    }

    /// Ends the cycle on the pane it stopped on, whether or not that focus has
    /// landed yet. The next observed focus settles any difference.
    pub(crate) fn finish(&mut self) {
        if let Some(cycle) = self.cycle.take()
            && let Some(key) = self.order.remove(cycle.cursor)
        {
            self.order.push_front(key);
        }
    }

    fn forget(&mut self, endpoint: &str) {
        if self.order.iter().any(|key| key.endpoint == endpoint) {
            // The cursor indexes the order about to shrink.
            self.finish();
            self.order.retain(|key| key.endpoint != endpoint);
        }
    }

    fn promote(&mut self, key: PaneKey) {
        if self.order.front() == Some(&key) {
            return;
        }
        self.order.retain(|entry| *entry != key);
        self.order.push_front(key);
        self.order.truncate(Self::LIMIT);
    }

    fn reachable(&self, projections: &[Projection], key: &PaneKey) -> bool {
        projections.iter().any(|p| {
            p.endpoint == key.endpoint
                && p.snapshot.is_some_and(|snapshot| {
                    self.boots.get(p.endpoint) == Some(&snapshot.boot_id)
                        && open(snapshot, &key.pane)
                })
        })
    }
}

fn open(snapshot: &ClientShellSnapshot, pane: &str) -> bool {
    snapshot.panes.iter().any(|p| p.pane_id == pane)
}

#[cfg(test)]
mod tests;
