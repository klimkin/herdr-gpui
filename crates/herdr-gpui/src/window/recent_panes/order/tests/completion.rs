use super::*;

fn accept(recent: &mut RecentPanes, focused: &str) -> String {
    let (index, pane) = peek_local(recent, Step::Back, &focusing(focused)).unwrap();
    recent.begin(index, Modifiers::control(), Some(key(LOCAL, focused)));
    pane
}

fn unfenced(recent: &mut RecentPanes, focused: &str) {
    recent.observe(
        &projections(&[(LOCAL, Some(&focusing(focused)))]),
        Some(FocusObservation::Unfenced(key(LOCAL, focused))),
    );
}

#[test]
fn stale_source_and_intermediate_reports_cannot_undo_release() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    assert_eq!(accept(&mut recent, "d"), "b");
    unfenced(&mut recent, "d");
    unfenced(&mut recent, "c");
    recent.release(Modifiers::none());
    assert_eq!(order(&recent), ["b", "d", "c", "a"]);

    unfenced(&mut recent, "d");
    unfenced(&mut recent, "c");
    assert_eq!(order(&recent), ["b", "d", "c", "a"]);
    unfenced(&mut recent, "b");
    assert!(recent.pending.is_none());
    assert_eq!(order(&recent), ["b", "d", "c", "a"]);
    assert_eq!(accept(&mut recent, "b"), "d");
}

#[test]
fn unrelated_unfenced_focus_wins_without_waiting_for_destination() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    unfenced(&mut recent, "a");
    assert_eq!(order(&recent), ["d", "c", "b", "a"]);

    recent.release(Modifiers::none());

    assert_eq!(order(&recent), ["a", "c", "d", "b"]);
    assert_eq!(accept(&mut recent, "a"), "c");
}

#[test]
fn settled_focus_can_complete_at_an_intermediate_or_unrelated_pane() {
    for focused in ["d", "c", "a"] {
        let mut recent = visited(&["a", "b", "c", "d"]);
        assert_eq!(accept(&mut recent, "d"), "c");
        assert_eq!(accept(&mut recent, "d"), "b");
        observe_local(&mut recent, &focusing(focused));
        recent.release(Modifiers::none());
        assert_eq!(order(&recent)[0], focused);
        assert!(recent.pending.is_none());
    }
}

#[test]
fn a_new_accepted_press_clears_the_previous_completion_evidence() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    observe_local(&mut recent, &focusing("c"));
    assert_eq!(accept(&mut recent, "c"), "b");
    recent.release(Modifiers::none());

    assert_eq!(order(&recent), ["b", "d", "c", "a"]);
    unfenced(&mut recent, "c");
    assert_eq!(order(&recent), ["b", "d", "c", "a"]);
}

#[test]
fn wrapping_to_the_original_source_completes_pending_selection() {
    let mut recent = visited(&["a", "b", "c"]);
    assert_eq!(accept(&mut recent, "c"), "b");
    assert_eq!(accept(&mut recent, "c"), "a");
    assert_eq!(accept(&mut recent, "a"), "c");
    unfenced(&mut recent, "c");
    recent.release(Modifiers::none());

    assert!(recent.pending.is_none());
    assert_eq!(order(&recent), ["c", "b", "a"]);
}

#[test]
fn pending_destinations_remain_bounded_and_keep_the_last_visit() {
    let mut recent = visited(&["a", "b", "c"]);
    for focused in ["c", "b", "a"]
        .into_iter()
        .cycle()
        .take(RecentPanes::LIMIT * 2)
    {
        accept(&mut recent, focused);
    }
    let pending = recent.pending.as_ref().unwrap();
    assert!(pending.destinations.len() <= RecentPanes::LIMIT);
    assert_eq!(
        pending.destinations,
        [key(LOCAL, "c"), key(LOCAL, "b"), key(LOCAL, "a")]
    );
}

#[test]
fn identical_pane_ids_on_different_machines_keep_distinct_focus_evidence() {
    let mut recent = visited(&["a", "b"]);
    let remote = at("remote-boot", PANES, Some("b"));
    let endpoints = [(LOCAL, Some(&focusing("b"))), (REMOTE, Some(&remote))];
    observe(&mut recent, &endpoints, REMOTE);
    let projected = projections(&endpoints);
    let (index, destination) = recent
        .peek(Step::Back, &projected, Some(&key(REMOTE, "b")))
        .unwrap();
    assert_eq!(destination, key(LOCAL, "b"));
    recent.begin(index, Modifiers::control(), Some(key(REMOTE, "b")));
    recent.release(Modifiers::none());
    recent.observe(
        &projected,
        Some(FocusObservation::Unfenced(key(REMOTE, "b"))),
    );
    assert_eq!(order(&recent), ["b", "remote:b", "a"]);
    recent.observe(
        &projected,
        Some(FocusObservation::Unfenced(key(LOCAL, "b"))),
    );
    assert!(recent.pending.is_none());
}

#[test]
fn removing_or_restarting_the_pending_destination_cannot_restore_it() {
    for restart in [false, true] {
        let mut recent = across_endpoints();
        let local = focusing("b");
        let remote = remote("x");
        let projected = projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&remote))]);
        let (index, _) = recent
            .peek(Step::Back, &projected, Some(&key(LOCAL, "b")))
            .unwrap();
        recent.begin(index, Modifiers::control(), Some(key(LOCAL, "b")));
        recent.release(Modifiers::none());
        let replacement = at("new-boot", HOSTS, Some("y"));
        let endpoints = if restart {
            vec![(LOCAL, Some(&local)), (REMOTE, Some(&replacement))]
        } else {
            vec![(LOCAL, Some(&local))]
        };
        recent.observe(&projections(&endpoints), None);
        recent.finish();
        assert_eq!(order(&recent), ["b", "a"]);
        assert!(recent.pending.is_none());
    }
}

#[test]
fn each_windows_cycle_and_release_keep_independent_histories() {
    let mut first = visited(&["a", "b", "c"]);
    let mut second = visited(&["c", "a", "b"]);
    assert_eq!(accept(&mut first, "c"), "b");
    observe_local(&mut second, &focusing("a"));
    first.release(Modifiers::none());

    assert_eq!(order(&first), ["b", "c", "a"]);
    assert_eq!(order(&second), ["a", "b", "c"]);
}

#[test]
fn reversing_a_global_cycle_keeps_the_frozen_order_and_original_hold() {
    let mut recent = across_endpoints();
    let local = focusing("b");
    let remote = remote("x");
    let projected = projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&remote))]);
    let (index, target) = recent
        .peek(Step::Back, &projected, Some(&key(REMOTE, "x")))
        .unwrap();
    assert_eq!(target, key(LOCAL, "b"));
    recent.begin(index, Modifiers::control(), Some(key(REMOTE, "x")));
    recent.observe(&projected, Some(FocusObservation::Settled(target.clone())));
    let (index, reverse) = recent
        .peek(Step::Forward, &projected, Some(&target))
        .unwrap();
    assert_eq!(reverse, key(REMOTE, "x"));
    recent.begin(index, Modifiers::control_shift(), Some(target));
    recent.release(Modifiers::control());
    assert!(recent.cycle.is_some());
    assert_eq!(order(&recent), ["remote:x", "b", "a"]);
    recent.release(Modifiers::none());
    recent.observe(&projected, Some(FocusObservation::Unfenced(reverse)));
    assert_eq!(order(&recent), ["remote:x", "b", "a"]);
    assert!(recent.pending.is_none());
}

#[test]
fn removing_an_unremembered_source_discards_pending_evidence() {
    for released in [false, true] {
        let mut recent = visited(&["a", "b"]);
        let local = focusing("b");
        let remote = remote("x");
        let projected = projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&remote))]);
        recent.observe(&projected, None);
        recent.begin(0, Modifiers::control(), Some(key(REMOTE, "x")));
        if released {
            recent.release(Modifiers::none());
        }
        recent.observe(&projections(&[(LOCAL, Some(&local))]), None);

        assert!(recent.pending.is_none());
        assert!(recent.cycle.is_none());
        assert_eq!(order(&recent), ["b", "a"]);
    }
}

#[test]
fn a_closed_final_destination_allows_later_legacy_focus_to_be_remembered() {
    for released in [true, false] {
        let mut recent = visited(&["a", "b", "c", "d"]);
        assert_eq!(accept(&mut recent, "d"), "c");
        assert_eq!(accept(&mut recent, "d"), "b");
        assert_eq!(accept(&mut recent, "d"), "a");
        if released {
            recent.release(Modifiers::none());
        }
        let closed = at("boot", &["b", "c", "d"], Some("b"));
        recent.observe(
            &projections(&[(LOCAL, Some(&closed))]),
            Some(FocusObservation::Unfenced(key(LOCAL, "b"))),
        );
        if !released {
            assert_eq!(
                peek_local(&recent, Step::Back, &closed).map(|(_, pane)| pane),
                Some("d".into())
            );
            recent.release(Modifiers::none());
        }
        let manual = at("boot", &["b", "c", "d"], Some("c"));
        recent.observe(
            &projections(&[(LOCAL, Some(&manual))]),
            Some(FocusObservation::Unfenced(key(LOCAL, "c"))),
        );

        assert_eq!(
            peek_local(&recent, Step::Back, &manual).map(|(_, pane)| pane),
            Some("b".into())
        );
    }
}

#[test]
fn disconnect_keeps_legacy_pending_selection_through_stale_reports() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    assert_eq!(accept(&mut recent, "d"), "b");
    assert_eq!(accept(&mut recent, "d"), "a");
    recent.release(Modifiers::none());
    recent.observe(&projections(&[(LOCAL, None)]), None);
    for focused in ["d", "c", "b"] {
        unfenced(&mut recent, focused);
    }

    assert_eq!(
        peek_local(&recent, Step::Back, &focusing("b")).map(|(_, pane)| pane),
        Some("a".into())
    );
}
