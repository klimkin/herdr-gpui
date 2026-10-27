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

#[derive(Debug)]
pub(super) enum FocusObservation {
    Settled(PaneKey),
    Unfenced(PaneKey),
}

#[derive(Debug)]
struct PendingSelection {
    source: Option<PaneKey>,
    destinations: VecDeque<PaneKey>,
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
    pending: Option<PendingSelection>,
}

#[derive(Debug)]
struct Cycle {
    /// The entry the last press went to.
    cursor: usize,
    /// The modifiers the cycle lasts for: releasing any of them commits.
    hold: Modifiers,
    observed: Option<PaneKey>,
}

impl RecentPanes {
    /// How many panes the order remembers; older ones fall off the back.
    pub(crate) const LIMIT: usize = 100;

    /// Forgets the panes of endpoints that restarted or are gone and records
    /// `focused` as the most recent pane. Only the selected endpoint's focus
    /// is passed: another endpoint's focus moves for other clients, not this
    /// user.
    pub(super) fn observe(
        &mut self,
        projections: &[Projection],
        focused: Option<FocusObservation>,
    ) {
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
        while let Some(gone) = self.boots.keys().find(|endpoint| !present(endpoint)) {
            let gone = gone.clone();
            self.forget(&gone);
        }
        while let Some(gone) = self.order.iter().find(|key| !present(&key.endpoint)) {
            let gone = gone.endpoint.clone();
            self.forget(&gone);
        }
        let final_closed = self
            .pending
            .as_ref()
            .and_then(|pending| pending.destinations.back())
            .is_some_and(|key| {
                projections.iter().any(|projection| {
                    projection.endpoint == key.endpoint
                        && projection.snapshot.is_some_and(|snapshot| {
                            self.boots.get(projection.endpoint) == Some(&snapshot.boot_id)
                                && !open(snapshot, &key.pane)
                        })
                })
            });
        if final_closed {
            self.pending = None;
        }
        if self.cycle.is_none() {
            self.order.retain(|key| {
                projections
                    .iter()
                    .find(|p| p.endpoint == key.endpoint)
                    .and_then(|p| p.snapshot)
                    .is_none_or(|snapshot| open(snapshot, &key.pane))
            });
        }
        let Some(focused) = focused else {
            return;
        };
        let key = match focused {
            FocusObservation::Settled(key) => key,
            FocusObservation::Unfenced(key) => {
                if let Some(pending) = &self.pending
                    && pending.destinations.back() != Some(&key)
                    && (pending.source.as_ref() == Some(&key)
                        || pending.destinations.contains(&key))
                {
                    return;
                }
                key
            }
        };
        if !self.reachable(projections, &key) {
            return;
        }
        self.pending = None;
        if let Some(cycle) = &mut self.cycle {
            cycle.observed = Some(key);
        } else {
            self.promote(key);
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
        let cursor = self.cycle.as_ref().map(|cycle| cycle.cursor);
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
    pub(crate) fn begin(&mut self, index: usize, modifiers: Modifiers, source: Option<PaneKey>) {
        let Some(key) = self.order.get(index).cloned() else {
            return;
        };
        let pending = self.pending.get_or_insert_with(|| PendingSelection {
            source,
            destinations: VecDeque::new(),
        });
        pending
            .destinations
            .retain(|destination| *destination != key);
        pending.destinations.push_back(key);
        if pending.destinations.len() > Self::LIMIT {
            pending.destinations.pop_front();
        }
        if let Some(cycle) = &mut self.cycle {
            cycle.cursor = index;
            cycle.observed = None;
            return;
        }
        let hold = Modifiers {
            shift: false,
            ..modifiers
        };
        self.cycle = Some(Cycle {
            cursor: index,
            hold,
            observed: None,
        });
        if !hold.modified() {
            self.finish();
        }
    }

    /// Commits the cycle once a modifier it holds is up.
    pub(crate) fn release(&mut self, modifiers: Modifiers) {
        if self
            .cycle
            .as_ref()
            .is_some_and(|cycle| !cycle.hold.is_subset_of(&modifiers))
        {
            self.finish();
        }
    }

    pub(crate) fn finish(&mut self) {
        let Some(cycle) = self.cycle.take() else {
            return;
        };
        if let Some(key) = self.order.get(cycle.cursor).cloned() {
            self.promote(key);
        }
        if self.pending.is_none()
            && let Some(observed) = cycle.observed
        {
            self.promote(observed);
        }
    }

    fn forget(&mut self, endpoint: &str) {
        self.boots.remove(endpoint);
        if let Some(cycle) = &mut self.cycle
            && cycle
                .observed
                .as_ref()
                .is_some_and(|key| key.endpoint == endpoint)
        {
            cycle.observed = None;
        }
        let invalidated = self.pending.as_ref().is_some_and(|pending| {
            pending
                .source
                .as_ref()
                .is_some_and(|key| key.endpoint == endpoint)
                || pending
                    .destinations
                    .back()
                    .is_some_and(|key| key.endpoint == endpoint)
        });
        if invalidated {
            self.pending = None;
        } else if let Some(pending) = &mut self.pending {
            pending.destinations.retain(|key| key.endpoint != endpoint);
        }
        if invalidated || self.order.iter().any(|key| key.endpoint == endpoint) {
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
