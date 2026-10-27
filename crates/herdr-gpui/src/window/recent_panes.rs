//! Ctrl-Tab through the panes this window focused on any endpoint: each press
//! while the shortcut's modifier stays held steps further back, and letting
//! go of it settles on the pane reached.

mod order;

use super::HerdrWindow;
use crate::{
    endpoint::{Endpoint, Step},
    navigation::NavigationTarget,
    state::{ConnectionStatus, LiveState},
};
use gpui::{Context, ModifiersChangedEvent, Window};
pub(crate) use order::RecentPanes;
use order::{FocusObservation, PaneKey, Projection};

impl HerdrWindow {
    pub(super) fn switch_recent_pane(
        &mut self,
        step: Step,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Mid-composition the IME owns the keyboard, as for double Shift.
        let endpoint = &self.endpoints[self.selected_endpoint];
        let unavailable = endpoint.outage().is_some()
            || matches!(
                self.live.status,
                ConnectionStatus::Disconnected | ConnectionStatus::Detached
            )
            || endpoint
                .connection
                .handle
                .as_ref()
                .is_none_or(|handle| handle.is_disconnected());
        if !self.marked.is_empty() || (!self.input_ready() && !unavailable) {
            return;
        }
        let current = self.focused_pane();
        let Some((index, key)) =
            self.recent_panes
                .peek(step, &self.projections(), current.as_ref())
        else {
            return;
        };
        let target = NavigationTarget::Pane(key.pane.as_str());
        // A press the focus fence refuses is dropped, as Back's is: key
        // repeat then paces itself at the daemon's round trip.
        let went = if key.endpoint == self.endpoints[self.selected_endpoint].id {
            self.navigate(target, cx)
        } else {
            self.navigate_endpoint(&key.endpoint, target, cx)
        };
        if went {
            self.recent_panes.begin(index, window.modifiers(), current);
            window.focus(&self.focus, cx);
        }
    }

    pub(super) fn recent_pane_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.recent_panes.release(event.modifiers);
    }

    /// Records the selected endpoint's focus once it has settled: while a
    /// switch or a focus change is still landing, the snapshot shows the pane
    /// being left, which must not move ahead of the one the user came from.
    pub(crate) fn observe_recent_panes(&mut self) {
        let settled = self.pending_navigation.is_none()
            && self.pending_toast.is_none()
            && !self.live.activation_pending();
        let focused = settled.then(|| self.focused_pane()).flatten().map(|key| {
            if self.live.supports_surface {
                FocusObservation::Settled(key)
            } else {
                FocusObservation::Unfenced(key)
            }
        });
        let projections = projections(&self.endpoints, self.selected_endpoint, &self.live);
        self.recent_panes.observe(&projections, focused);
    }

    fn focused_pane(&self) -> Option<PaneKey> {
        let pane = self.live.snapshot.as_ref()?.focused_pane_id.clone()?;
        Some(PaneKey {
            endpoint: self.endpoints[self.selected_endpoint].id.clone(),
            pane,
        })
    }

    fn projections(&self) -> Vec<Projection<'_>> {
        projections(&self.endpoints, self.selected_endpoint, &self.live)
    }
}

/// Each endpoint's snapshot as this window sees it. The selected one's is the
/// window's own copy, which the focus fence and navigation read.
fn projections<'a>(
    endpoints: &'a [Endpoint],
    selected: usize,
    live: &'a LiveState,
) -> Vec<Projection<'a>> {
    endpoints
        .iter()
        .enumerate()
        .map(|(index, endpoint)| {
            let live = if index == selected {
                live
            } else {
                &endpoint.live
            };
            Projection {
                endpoint: &endpoint.id,
                snapshot: live.snapshot.as_deref().filter(|_| {
                    endpoint.enabled
                        && live.status.is_connected()
                        && endpoint
                            .connection
                            .handle
                            .as_ref()
                            .is_some_and(|handle| !handle.is_disconnected())
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
