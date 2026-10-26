#![allow(clippy::unwrap_used)]
use super::*;
use crate::{
    keymap::{Binding, DaemonKeys, Keymap},
    palette::Filter,
    sidebar::layout_tests::fixture_window,
    window::MockPeer,
};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use herdr_client::protocol::ClientShellSnapshot;
use serde_json::json;
use std::sync::Arc;

const BOOT: &str = "boot-v1";
const PANES: &[&str] = &["a", "b", "c", "d"];

/// The fixture's snapshot with `PANES` open and `focused` focused.
fn snapshot(focused: &str) -> ClientShellSnapshot {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
        "../../../../herdr-protocol/tests/fixtures/endpoint-snapshot-v1.json"
    ))
    .unwrap();
    let pane = snapshot.panes[0].clone();
    snapshot.panes = PANES
        .iter()
        .map(|id| {
            let mut pane = pane.clone();
            pane.pane_id = (*id).to_owned();
            pane
        })
        .collect();
    snapshot.focused_pane_id = Some(focused.to_owned());
    snapshot
}

/// The daemon reports `pane` focused, as the window's poll delivers it.
fn land(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext, pane: &str) {
    view.update(cx, |view, _| {
        view.live.snapshot = Some(Arc::new(snapshot(pane)));
        let endpoint = &mut view.endpoints[view.selected_endpoint];
        endpoint.live = view.live.clone();
        endpoint.sync_live();
    });
}

/// A focused window under the app's key bindings, connected to `peer`, that
/// has focused each of `PANES` in turn, so D is focused and C came before it.
fn connected<'a>(
    cx: &'a mut TestAppContext,
    peer: &MockPeer,
    keymap: Keymap,
) -> (Entity<HerdrWindow>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        let mut appearance = crate::app::InitialAppearance::default();
        appearance.config.keybindings = keymap;
        cx.set_global(appearance);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        fixture_window(window, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            peer.prepare(view, cx);
            window.focus(&view.focus, cx);
        })
    });
    for pane in PANES {
        land(&view, cx, pane);
    }
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        view.update(cx, |view, _| {
            // The daemon drew the surface at the size this window asks for.
            let size = view.options.surface_size;
            let surface = Arc::make_mut(view.live.surface.as_mut().unwrap());
            (surface.frame.width, surface.frame.height) = (size.cols, size.rows);
        });
        assert!(view.read(cx).input_ready());
    });
    (view, cx)
}

/// The panes the window asked the daemon to focus, in order. The daemon
/// answers one request at a time, so each is answered here. A focus sent from
/// here marks the end, so a request the window never made is missing.
fn focus_requests(peer: &mut MockPeer) -> Vec<String> {
    peer.client.handle.focus_pane(BOOT, "end").unwrap();
    let mut panes = Vec::new();
    loop {
        let request = peer.request();
        let id = request["id"].as_str().unwrap();
        peer.respond(BOOT, id, &json!({"id": id, "result": {"type": "ok"}}));
        if request["method"] != "pane.focus" {
            continue;
        }
        let pane = request["params"]["pane_id"].as_str().unwrap().to_owned();
        if pane == "end" {
            return panes;
        }
        panes.push(pane);
    }
}

fn tap(cx: &mut VisualTestContext, held: Modifiers, keystroke: &str) {
    cx.simulate_modifiers_change(held);
    cx.simulate_keystrokes(keystroke);
    cx.simulate_modifiers_change(Modifiers::none());
}

#[gpui::test]
fn holding_ctrl_walks_back_and_letting_go_settles(cx: &mut TestAppContext) {
    let mut peer = MockPeer::advertising(&["pane.focus"]);
    let (view, cx) = connected(cx, &peer, Keymap::default());

    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    land(&view, cx, "c");
    cx.simulate_keystrokes("ctrl-tab");
    land(&view, cx, "b");
    cx.simulate_modifiers_change(Modifiers::none());
    // Settled on B, so a tap goes back to D, the pane before it.
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c", "b", "d"]);
}

#[gpui::test]
fn ctrl_shift_tab_starts_from_the_oldest_pane(cx: &mut TestAppContext) {
    let mut peer = MockPeer::advertising(&["pane.focus"]);
    let (_, cx) = connected(cx, &peer, Keymap::default());

    tap(cx, Modifiers::control_shift(), "ctrl-shift-tab");

    assert_eq!(focus_requests(&mut peer), ["a"]);
}

#[gpui::test]
fn a_press_while_the_focus_is_landing_is_dropped(cx: &mut TestAppContext) {
    let mut peer = MockPeer::welcoming(
        &["pane.focus", "client_shell.surface.set"],
        &["surface_interest"],
    );
    let (view, cx) = connected(cx, &peer, Keymap::default());
    view.update(cx, |view, _| {
        // Fence focus changes on the surface, as a current daemon does.
        view.live.supports_surface = true;
        let endpoint = &mut view.endpoints[view.selected_endpoint];
        *endpoint.connection.inbox.lock().unwrap() = view.live.clone();
    });

    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    view.read_with(cx, |view, _| {
        assert!(view.live.activation.is_some());
        assert!(!view.input_ready());
    });
    cx.simulate_keystrokes("ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c"]);
    // The cycle is still on C, so the next press goes on to B.
    view.read_with(cx, |view, _| {
        let next = view.endpoints[view.selected_endpoint]
            .recent
            .peek(Step::Back, &snapshot("c"));
        assert_eq!(next, Some((2, "b".to_owned())));
    });
}

#[gpui::test]
fn an_open_palette_keeps_ctrl_tab_for_its_filters(cx: &mut TestAppContext) {
    let mut peer = MockPeer::advertising(&["pane.focus"]);
    let (view, cx) = connected(cx, &peer, Keymap::default());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.open_palette(Filter::All, window, cx));
        window.draw(cx).clear(cx);
    });

    tap(cx, Modifiers::control(), "ctrl-tab");

    view.read_with(cx, |view, _| {
        assert_eq!(
            view.menu.palette.as_ref().unwrap().filter,
            Filter::Navigation
        );
    });
    assert!(focus_requests(&mut peer).is_empty());
}

#[gpui::test]
fn a_rebound_shortcut_replaces_ctrl_tab(cx: &mut TestAppContext) {
    let mut peer = MockPeer::advertising(&["pane.focus"]);
    let overrides = [("recent_pane".to_owned(), Binding::One("alt-`".into()))].into();
    let keymap =
        Keymap::with_overrides(&overrides, &Default::default(), &DaemonKeys::default()).unwrap();
    let (view, cx) = connected(cx, &peer, keymap);

    cx.simulate_modifiers_change(Modifiers::alt());
    cx.simulate_keystrokes("alt-`");
    land(&view, cx, "c");
    cx.simulate_keystrokes("alt-`");
    cx.simulate_modifiers_change(Modifiers::none());
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c", "b"]);
}

#[gpui::test]
fn ime_composition_keeps_the_pane(cx: &mut TestAppContext) {
    let mut peer = MockPeer::advertising(&["pane.focus"]);
    let (view, cx) = connected(cx, &peer, Keymap::default());
    view.update(cx, |view, _| view.marked = "か".into());

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert!(focus_requests(&mut peer).is_empty());
}

/// Only a window built by `HerdrWindow::new` watches its activation.
#[cfg(feature = "integration-test")]
#[gpui::test]
fn leaving_the_window_settles_the_cycle(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        HerdrWindow::new(
            herdr_client::ConnectTarget::Socket("/unused-recent-panes-test.sock".into()),
            window,
            cx,
            true,
        )
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    view.update(cx, |view, _| {
        let recent = &mut view.endpoints[view.selected_endpoint].recent;
        for pane in ["a", "b", "c"] {
            recent.observe(Some(&snapshot(pane)));
        }
        let (index, pane) = recent.peek(Step::Back, &snapshot("c")).unwrap();
        assert_eq!(pane, "b");
        recent.begin(index, Modifiers::control());
    });

    cx.deactivate_window();

    view.update(cx, |view, _| {
        let recent = &mut view.endpoints[view.selected_endpoint].recent;
        recent.observe(Some(&snapshot("b")));
        // Settled on B, so the pane before it is C again.
        assert_eq!(
            recent.peek(Step::Back, &snapshot("b")),
            Some((1, "c".to_owned()))
        );
    });
}
