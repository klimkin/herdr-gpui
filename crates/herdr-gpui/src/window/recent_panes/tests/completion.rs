use super::*;

fn support_fences(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) {
    view.update(cx, |view, _| {
        view.live.supports_surface = true;
        *view.endpoints[view.selected_endpoint]
            .connection
            .inbox
            .lock()
            .unwrap() = view.live.clone();
    });
}

fn complete_after_release(cx: &mut TestAppContext, ack_first: bool) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    support_fences(&view, cx);
    tap(cx, Modifiers::control(), "ctrl-tab");
    assert!(!view.read_with(cx, |view, _| view.input_ready()));
    let mut stale = snapshot("d");
    stale.revision += 1;
    let frame = surface_event(&view, cx, &stale);
    publish(&view, cx, [ClientEvent::Snapshot(Arc::new(stale)), frame]);
    assert!(!view.read_with(cx, |view, _| view.input_ready()));
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.recent_panes
                .peek(Step::Back, &view.projections(), Some(&key(LOCAL, "d"))),
            Some((0, key(LOCAL, "c")))
        );
    });

    let mut target = snapshot("c");
    target.revision += 2;
    let (ack, focused) = barrier_response(&mut peer, &view, cx, target.revision);
    assert_eq!(focused, ["c"]);
    let frame = surface_event(&view, cx, &target);
    if ack_first {
        publish(&view, cx, [ack]);
        assert!(!view.read_with(cx, |view, _| view.input_ready()));
        publish(&view, cx, [ClientEvent::Snapshot(Arc::new(target))]);
        assert!(!view.read_with(cx, |view, _| view.input_ready()));
        publish(&view, cx, [frame]);
    } else {
        publish(&view, cx, [ClientEvent::Snapshot(Arc::new(target)), frame]);
        assert!(!view.read_with(cx, |view, _| view.input_ready()));
        publish(&view, cx, [ack]);
    }
    assert!(view.read_with(cx, |view, _| view.input_ready()));
    assert!(view.read_with(cx, |view, _| {
        view.live.activation.as_ref().unwrap().focus.is_none()
    }));
    tap(cx, Modifiers::control(), "ctrl-tab");
    assert_eq!(focus_requests(&mut peer), ["d"]);
}

#[gpui::test]
fn release_before_ack_then_surface_keeps_the_accepted_focus(cx: &mut TestAppContext) {
    complete_after_release(cx, true);
}

#[gpui::test]
fn release_before_surface_then_ack_keeps_the_accepted_focus(cx: &mut TestAppContext) {
    complete_after_release(cx, false);
}

#[gpui::test]
fn coalesced_completion_and_unrelated_focus_win_on_release(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    support_fences(&view, cx);
    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    let mut target = snapshot("c");
    target.revision += 1;
    let (ack, focused) = barrier_response(&mut peer, &view, cx, target.revision);
    assert_eq!(focused, ["c"]);
    let target_frame = surface_event(&view, cx, &target);
    let mut unrelated = snapshot("a");
    unrelated.revision += 2;
    let unrelated_frame = surface_event(&view, cx, &unrelated);
    publish(
        &view,
        cx,
        [
            ack,
            ClientEvent::Snapshot(Arc::new(target)),
            target_frame,
            ClientEvent::Snapshot(Arc::new(unrelated)),
            unrelated_frame,
        ],
    );
    assert!(view.read_with(cx, |view, _| view.input_ready()));
    cx.simulate_modifiers_change(Modifiers::none());
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c"]);
}

#[gpui::test]
fn legacy_stale_focus_after_release_cannot_reorder_the_final_choice(cx: &mut TestAppContext) {
    let mut peer = MockPeer::advertising(&["pane.focus"]);
    let (view, cx) = connected(cx, &peer, Keymap::default());
    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab ctrl-tab");
    land(&view, cx, "d");
    cx.simulate_modifiers_change(Modifiers::none());
    land(&view, cx, "c");
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.recent_panes
                .peek(Step::Back, &view.projections(), Some(&key(LOCAL, "c"))),
            Some((0, key(LOCAL, "b")))
        );
    });
    land(&view, cx, "b");
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c", "b", "d"]);
}

#[gpui::test]
fn fragmented_unrelated_focus_during_hold_becomes_the_next_toggle(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    arrive(&view, cx, showing(&["b", "c", "d"], "d"));
    support_fences(&view, cx);
    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    let mut target = showing(&["b", "c", "d"], "c");
    target.revision += 1;
    let (ack, focused) = barrier_response(&mut peer, &view, cx, target.revision);
    assert_eq!(focused, ["c"]);
    let frame = surface_event(&view, cx, &target);
    publish(
        &view,
        cx,
        [ack, ClientEvent::Snapshot(Arc::new(target)), frame],
    );
    assert!(view.read_with(cx, |view, _| view.input_ready()));

    let mut unrelated = snapshot("a");
    unrelated.revision += 2;
    let frame = surface_event(&view, cx, &unrelated);
    publish(&view, cx, [ClientEvent::Snapshot(Arc::new(unrelated))]);
    assert!(!view.read_with(cx, |view, _| view.input_ready()));
    publish(&view, cx, [frame]);
    assert!(view.read_with(cx, |view, _| view.input_ready()));
    cx.simulate_modifiers_change(Modifiers::none());
    tap(cx, Modifiers::control(), "ctrl-tab");

    let mut target = snapshot("c");
    target.revision += 3;
    let (ack, focused) = barrier_response(&mut peer, &view, cx, target.revision);
    assert_eq!(focused, ["c"]);
    let frame = surface_event(&view, cx, &target);
    publish(
        &view,
        cx,
        [ack, ClientEvent::Snapshot(Arc::new(target)), frame],
    );
    assert!(view.read_with(cx, |view, _| view.input_ready()));
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["a"]);
}
