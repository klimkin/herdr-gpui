#![allow(clippy::unwrap_used)]
use super::*;

/// A snapshot of `boot` with `panes` open and `focused` focused.
fn at(boot: &str, panes: &[&str], focused: Option<&str>) -> ClientShellSnapshot {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
        "../../../../herdr-protocol/tests/fixtures/endpoint-snapshot-v1.json"
    ))
    .unwrap();
    let pane = snapshot.panes[0].clone();
    snapshot.panes = panes
        .iter()
        .map(|id| {
            let mut pane = pane.clone();
            pane.pane_id = (*id).to_owned();
            pane
        })
        .collect();
    snapshot.boot_id = boot.into();
    snapshot.focused_pane_id = focused.map(str::to_owned);
    snapshot
}

const PANES: &[&str] = &["a", "b", "c", "d"];

fn focusing(focused: &str) -> ClientShellSnapshot {
    at("boot", PANES, Some(focused))
}

/// Focuses each of `panes` in turn, as clicks would.
fn visited(panes: &[&str]) -> RecentPanes {
    let mut recent = RecentPanes::default();
    for pane in panes {
        recent.observe(Some(&focusing(pane)));
    }
    recent
}

fn order(recent: &RecentPanes) -> Vec<&str> {
    recent.order.iter().map(String::as_str).collect()
}

/// One press from `focused` with `held` down, as the window runs it once the
/// request is queued, followed by the daemon's report that the focus landed.
fn press(recent: &mut RecentPanes, step: Step, focused: &str, held: Modifiers) -> Option<String> {
    let (index, pane) = recent.peek(step, &focusing(focused))?;
    recent.begin(index, held);
    recent.observe(Some(&focusing(&pane)));
    Some(pane)
}

/// Ctrl-Tab presses from `focused` with Ctrl held throughout.
fn walk(recent: &mut RecentPanes, mut focused: String, presses: usize) -> Vec<String> {
    (0..presses)
        .map(|_| {
            focused = press(recent, Step::Back, &focused, Modifiers::control()).unwrap();
            focused.clone()
        })
        .collect()
}

#[test]
fn focus_orders_panes_most_recent_first() {
    assert_eq!(order(&visited(&["a", "b", "c"])), ["c", "b", "a"]);
}

#[test]
fn a_repeated_snapshot_does_not_reorder() {
    let mut recent = visited(&["a", "b"]);
    recent.observe(Some(&focusing("b")));
    recent.observe(None);
    recent.observe(Some(&focusing("b")));

    assert_eq!(order(&recent), ["b", "a"]);
}

#[test]
fn a_tap_toggles_between_the_two_most_recent_panes() {
    let mut recent = visited(&["a", "b", "c"]);
    let mut focused = "c".to_owned();
    let mut landed = Vec::new();
    for _ in 0..3 {
        focused = press(&mut recent, Step::Back, &focused, Modifiers::control()).unwrap();
        recent.release(Modifiers::none());
        landed.push(focused.clone());
    }

    assert_eq!(landed, ["b", "c", "b"]);
    assert_eq!(order(&recent), ["b", "c", "a"]);
}

#[test]
fn holding_walks_back_and_the_release_commits_where_it_stopped() {
    let mut recent = visited(&["a", "b", "c", "d"]);

    assert_eq!(walk(&mut recent, "d".into(), 3), ["c", "b", "a"]);
    recent.release(Modifiers::none());

    assert_eq!(order(&recent), ["a", "d", "c", "b"]);
}

#[test]
fn the_reverse_first_press_reaches_the_oldest_pane() {
    let mut recent = visited(&["a", "b", "c", "d"]);

    let pane = press(&mut recent, Step::Forward, "d", Modifiers::control_shift());

    assert_eq!(pane.as_deref(), Some("a"));
}

#[test]
fn stepping_wraps_around() {
    let mut recent = visited(&["a", "b", "c"]);

    assert_eq!(walk(&mut recent, "c".into(), 4), ["b", "a", "c", "b"]);
}

#[test]
fn closed_and_focused_panes_are_stepped_over() {
    let recent = visited(&["a", "b", "c", "d"]);
    let without_c = at("boot", &["a", "b", "d"], Some("d"));
    // A click on c that the order has not observed yet.
    let on_c = focusing("c");

    assert_eq!(recent.peek(Step::Back, &without_c), Some((2, "b".into())));
    assert_eq!(recent.peek(Step::Back, &on_c), Some((2, "b".into())));
}

#[test]
fn nothing_to_switch_to_without_another_open_pane() {
    let recent = visited(&["a", "b"]);

    assert_eq!(
        recent.peek(Step::Back, &at("boot", &["b"], Some("b"))),
        None
    );
    assert_eq!(
        recent.peek(Step::Back, &at("other", PANES, Some("b"))),
        None
    );
}

#[test]
fn closing_a_pane_drops_it_from_the_order() {
    let mut recent = visited(&["a", "b", "c"]);
    recent.observe(Some(&at("boot", &["a", "c"], Some("c"))));

    assert_eq!(order(&recent), ["c", "a"]);
}

#[test]
fn a_new_boot_resets_the_order_and_ends_the_cycle() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());

    recent.observe(Some(&at("other", &["x", "y"], Some("x"))));

    assert!(recent.cycle.is_none());
    assert_eq!(order(&recent), ["x"]);
}

#[test]
fn the_order_is_frozen_while_a_cycle_is_open() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());
    recent.observe(Some(&at("boot", &["a", "b"], Some("a"))));

    assert_eq!(order(&recent), ["c", "b", "a"]);
}

#[test]
fn a_focus_landing_after_the_commit_changes_nothing() {
    let mut recent = visited(&["a", "b", "c"]);
    let (index, pane) = recent.peek(Step::Back, &focusing("c")).unwrap();
    recent.begin(index, Modifiers::control());
    recent.release(Modifiers::none());
    recent.observe(Some(&focusing(&pane)));

    assert_eq!(order(&recent), ["b", "c", "a"]);
}

#[test]
fn a_later_press_without_modifiers_keeps_the_original_hold() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());
    press(&mut recent, Step::Back, "b", Modifiers::none());

    assert!(recent.cycle.is_some());
    recent.release(Modifiers::none());
    assert!(recent.cycle.is_none());
    assert_eq!(order(&recent), ["a", "c", "b"]);
}

#[test]
fn a_press_without_a_hold_commits_at_once() {
    let mut recent = visited(&["a", "b", "c"]);
    let (index, _) = recent.peek(Step::Back, &focusing("c")).unwrap();
    recent.begin(index, Modifiers::none());

    assert!(recent.cycle.is_none());
    assert_eq!(order(&recent), ["b", "c", "a"]);
}

#[test]
fn the_cycle_lasts_while_its_modifiers_stay_down() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());

    recent.release(Modifiers::control());
    recent.release(Modifiers::control() | Modifiers::alt());

    assert!(recent.cycle.is_some());
}

#[test]
fn releasing_shift_alone_does_not_commit() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Forward, "c", Modifiers::control_shift());

    recent.release(Modifiers::control());
    assert!(recent.cycle.is_some());
    recent.release(Modifiers::none());
    assert!(recent.cycle.is_none());
}

#[test]
fn finishing_twice_changes_nothing_more() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());

    recent.finish();
    let committed = order(&recent).join(",");
    recent.finish();

    assert_eq!(order(&recent).join(","), committed);
    assert_eq!(committed, "b,c,a");
}

#[test]
fn the_order_is_bounded() {
    let panes: Vec<String> = (0..RecentPanes::LIMIT + 10)
        .map(|n| format!("p{n}"))
        .collect();
    let open: Vec<&str> = panes.iter().map(String::as_str).collect();
    let mut recent = RecentPanes::default();
    for pane in &open {
        recent.observe(Some(&at("boot", &open, Some(pane))));
    }

    assert_eq!(recent.order.len(), RecentPanes::LIMIT);
    assert_eq!(recent.order.front(), panes.last());
}

#[test]
fn a_click_during_the_hold_wins_after_the_commit() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    press(&mut recent, Step::Back, "d", Modifiers::control());
    recent.observe(Some(&focusing("a")));

    recent.release(Modifiers::none());
    assert_eq!(order(&recent), ["c", "d", "b", "a"]);
    recent.observe(Some(&focusing("a")));
    assert_eq!(order(&recent), ["a", "c", "d", "b"]);
}
