//! Ctrl-Tab through the selected endpoint's recently used panes: each press
//! while the shortcut's modifier stays held steps further back, and letting
//! go of it settles on the pane reached.

use super::HerdrWindow;
use crate::{endpoint::Step, navigation::NavigationTarget};
use gpui::{Context, ModifiersChangedEvent, Window};

impl HerdrWindow {
    pub(super) fn switch_recent_pane(
        &mut self,
        step: Step,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Mid-composition the IME owns the keyboard, as for double Shift.
        if !self.marked.is_empty() {
            return;
        }
        let Some(snapshot) = self.live.snapshot.clone() else {
            return;
        };
        let Some((index, pane)) = self.endpoints[self.selected_endpoint]
            .recent
            .peek(step, &snapshot)
        else {
            return;
        };
        // A press the focus fence refuses is dropped, as Back's is: key
        // repeat then paces itself at the daemon's round trip.
        if self.navigate(NavigationTarget::Pane(&pane), cx) {
            self.endpoints[self.selected_endpoint]
                .recent
                .begin(index, window.modifiers());
            window.focus(&self.focus, cx);
        }
    }

    pub(super) fn recent_pane_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        for endpoint in &mut self.endpoints {
            endpoint.recent.release(event.modifiers);
        }
    }

    /// Settles every open cycle. A window that loses focus is never told the
    /// modifier came up.
    pub(super) fn finish_recent_cycles(&mut self) {
        for endpoint in &mut self.endpoints {
            endpoint.recent.finish();
        }
    }
}

#[cfg(test)]
mod tests;
