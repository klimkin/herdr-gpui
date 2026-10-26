//! Ctrl-Tab through the panes an endpoint focused, most recent first, the
//! way an editor cycles its recent files. The order is client-local and
//! observed from the daemon's snapshots, like the Back and Forward trail,
//! and pane IDs never outlive their boot.

use super::Step;
use gpui::Modifiers;
use herdr_client::protocol::ClientShellSnapshot;
use std::collections::VecDeque;

#[derive(Debug, Default)]
pub(crate) struct RecentPanes {
    boot: String,
    /// Open panes, the focused one first.
    order: VecDeque<String>,
    /// While a cycle is open the order is frozen, so the panes it passes
    /// through are not recorded and `cursor` keeps indexing the same pane.
    cycle: Option<Cycle>,
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

    /// Records the snapshot's focus as the most recent pane. A snapshot
    /// without one, such as the empty state between connections, leaves the
    /// order alone.
    pub(crate) fn observe(&mut self, snapshot: Option<&ClientShellSnapshot>) {
        let Some(snapshot) = snapshot else {
            return;
        };
        if snapshot.boot_id != self.boot {
            *self = Self {
                boot: snapshot.boot_id.clone(),
                ..Self::default()
            };
        }
        if self.cycle.is_some() {
            return;
        }
        self.order.retain(|pane| open(snapshot, pane));
        if let Some(focused) = snapshot.focused_pane_id.as_deref() {
            self.promote(focused);
        }
    }

    /// The entry the next press goes to and its pane, without going: `Back`
    /// is older, `Forward` newer, and either wraps around. Panes that closed
    /// and the one already focused are stepped over.
    pub(crate) fn peek(
        &self,
        step: Step,
        snapshot: &ClientShellSnapshot,
    ) -> Option<(usize, String)> {
        if snapshot.boot_id != self.boot {
            return None;
        }
        let from = self.cycle.map_or(0, |cycle| cycle.cursor);
        let count = self.order.len();
        (1..count)
            .map(|offset| match step {
                Step::Back => (from + offset) % count,
                Step::Forward => (from + count - offset) % count,
            })
            .find_map(|index| {
                let pane = self.order.get(index)?;
                (snapshot.focused_pane_id.as_ref() != Some(pane) && open(snapshot, pane))
                    .then(|| (index, pane.clone()))
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
            && let Some(pane) = self.order.get(cycle.cursor).cloned()
        {
            self.promote(&pane);
        }
    }

    fn promote(&mut self, pane: &str) {
        if self.order.front().is_some_and(|front| front == pane) {
            return;
        }
        self.order.retain(|entry| entry != pane);
        self.order.push_front(pane.to_owned());
        self.order.truncate(Self::LIMIT);
    }
}

fn open(snapshot: &ClientShellSnapshot, pane: &str) -> bool {
    snapshot.panes.iter().any(|p| p.pane_id == pane)
}

#[cfg(test)]
mod tests;
