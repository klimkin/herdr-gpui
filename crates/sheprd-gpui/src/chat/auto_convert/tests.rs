//! Which new agents `[chat] auto_convert` moves, and when.

use super::*;
use sheprd_client::protocol::{ClientShellAgent, ClientShellPane};

const ENDPOINT: &str = "0123456789abcdef0123456789abcdef";

fn scope(boot: &str) -> Scope {
    Scope {
        boot: boot.into(),
        selection: 1,
        connection: 1,
    }
}

/// A snapshot of `agents`, each `(pane, agent, status, seq)`, every pane
/// listed among the panes too.
fn snapshot(agents: &[(&str, &str, AgentStatus, u64)]) -> ClientShellSnapshot {
    let mut snapshot = crate::sidebar::layout_tests::snapshot(1);
    let agent = snapshot.agents[0].clone();
    snapshot.focused_pane_id = None;
    snapshot.agents = agents
        .iter()
        .map(|&(id, name, status, seq)| ClientShellAgent {
            pane_id: id.into(),
            agent: Some(name.into()),
            agent_status: status,
            state_change_seq: seq,
            tokens: Vec::new(),
            focused: false,
            ..agent.clone()
        })
        .collect();
    snapshot.panes = agents.iter().map(|&(id, ..)| pane(id)).collect();
    snapshot
}

fn pane(id: &str) -> ClientShellPane {
    ClientShellPane {
        pane_id: id.into(),
        workspace_id: "w0".into(),
        tab_id: "t0".into(),
        label: None,
        cwd: None,
        foreground_cwd: None,
        focused: false,
        right_click_passthrough: false,
    }
}

fn with_pane(mut snapshot: ClientShellSnapshot, id: &str) -> ClientShellSnapshot {
    snapshot.panes.push(pane(id));
    snapshot
}

fn typed_into(pane: &str, at: Instant) -> Typed {
    Typed {
        at,
        pane: Some(pane.into()),
    }
}

fn tick(now: Instant) -> Tick<'static> {
    Tick {
        now,
        busy: false,
        last_typed: None,
    }
}

/// A tracker that has taken `baseline` as what already ran.
fn tracker(baseline: &ClientShellSnapshot, now: Instant) -> Tracker {
    let mut tracker = Tracker::default();
    assert_eq!(tracker.poll(scope("b1"), baseline, tick(now)), None);
    tracker
}

const IDLE: AgentStatus = AgentStatus::Idle;
const WORKING: AgentStatus = AgentStatus::Working;

#[test]
fn a_new_idle_agent_moves_once_it_has_settled() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let new = snapshot(&[("p1", "claude", IDLE, 1)]);
    assert_eq!(tracker.poll(scope("b1"), &new, tick(start)), None);
    let early = start + SETTLE - Duration::from_millis(1);
    assert_eq!(tracker.poll(scope("b1"), &new, tick(early)), None);
    assert_eq!(
        tracker.poll(scope("b1"), &new, tick(start + SETTLE)),
        Some("p1".into())
    );
    // Tried once, whatever came of it.
    let later = start + SETTLE * 3;
    assert_eq!(tracker.poll(scope("b1"), &new, tick(later)), None);
}

#[test]
fn agents_running_when_the_window_connected_stay() {
    let start = Instant::now();
    let running = snapshot(&[("p1", "claude", IDLE, 1)]);
    let mut tracker = tracker(&running, start);
    assert_eq!(
        tracker.poll(scope("b1"), &running, tick(start + SETTLE * 2)),
        None
    );
}

#[test]
fn a_new_daemon_or_connection_takes_a_new_baseline() {
    // As after the app restarts: nothing it finds running moves.
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let now = snapshot(&[("p1", "claude", IDLE, 1)]);
    assert_eq!(tracker.poll(scope("b2"), &now, tick(start)), None);
    assert_eq!(
        tracker.poll(scope("b2"), &now, tick(start + SETTLE * 2)),
        None
    );
}

#[test]
fn a_prompt_right_after_start_restarts_the_wait() {
    // `herdr agent start` then `herdr agent prompt`: the agent works first.
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let ready = snapshot(&[("p1", "claude", IDLE, 1)]);
    assert_eq!(tracker.poll(scope("b1"), &ready, tick(start)), None);
    let at = |secs| start + Duration::from_secs(secs);
    let working = snapshot(&[("p1", "claude", WORKING, 2)]);
    assert_eq!(tracker.poll(scope("b1"), &working, tick(at(3))), None);
    assert_eq!(tracker.poll(scope("b1"), &working, tick(at(30))), None);
    let done = snapshot(&[("p1", "claude", AgentStatus::Done, 3)]);
    assert_eq!(tracker.poll(scope("b1"), &done, tick(at(31))), None);
    assert_eq!(tracker.poll(scope("b1"), &done, tick(at(40))), None);
    assert_eq!(
        tracker.poll(scope("b1"), &done, tick(at(41))),
        Some("p1".into())
    );
}

#[test]
fn a_change_herdr_reports_while_idle_restarts_the_wait() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let first = snapshot(&[("p1", "claude", IDLE, 1)]);
    tracker.poll(scope("b1"), &first, tick(start));
    let moved = snapshot(&[("p1", "claude", IDLE, 2)]);
    let half = start + SETTLE / 2;
    assert_eq!(tracker.poll(scope("b1"), &moved, tick(half)), None);
    assert_eq!(
        tracker.poll(scope("b1"), &moved, tick(start + SETTLE)),
        None
    );
    assert_eq!(
        tracker.poll(scope("b1"), &moved, tick(half + SETTLE)),
        Some("p1".into())
    );
}

#[test]
fn agents_that_cannot_move_and_chats_are_left_alone() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut panes = snapshot(&[
        ("p1", "gemini", IDLE, 1),
        ("p2", "pi", IDLE, 1),
        ("p3", "acp", IDLE, 1),
        ("p4", "vim", IDLE, 1),
    ]);
    panes.agents[2].tokens = vec![(sheprd_chat_protocol::TOKEN.into(), ENDPOINT.into())];
    for at in [start, start + SETTLE, start + SETTLE * 2] {
        assert_eq!(tracker.poll(scope("b1"), &panes, tick(at)), None);
    }
}

#[test]
fn a_pane_a_tool_marked_keeps_its_terminal() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut kept = snapshot(&[("p1", "claude", IDLE, 1)]);
    kept.agents[0].tokens = vec![(KEEP_TOKEN.0.into(), KEEP_TOKEN.1.into())];
    tracker.poll(scope("b1"), &kept, tick(start));
    assert_eq!(tracker.poll(scope("b1"), &kept, tick(start + SETTLE)), None);
    // Clearing the token later does not bring it back.
    kept.agents[0].tokens.clear();
    assert_eq!(
        tracker.poll(scope("b1"), &kept, tick(start + SETTLE * 3)),
        None
    );
}

#[test]
fn typing_into_the_focused_pane_keeps_its_terminal() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut focused = snapshot(&[("p1", "claude", IDLE, 1)]);
    focused.focused_pane_id = Some("p1".into());
    tracker.poll(scope("b1"), &focused, tick(start));
    let typed = Tick {
        last_typed: Some(&typed_into("p1", start + Duration::from_secs(2))),
        ..tick(start + SETTLE)
    };
    assert_eq!(tracker.poll(scope("b1"), &focused, typed), None);
    assert_eq!(
        tracker.poll(scope("b1"), &focused, tick(start + SETTLE * 3)),
        None
    );
}

#[test]
fn input_before_the_agent_was_ready_does_not_count() {
    // Typing `claude` and Enter is how the agent started.
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut focused = snapshot(&[("p1", "claude", IDLE, 1)]);
    focused.focused_pane_id = Some("p1".into());
    tracker.poll(scope("b1"), &focused, tick(start));
    let typed = Tick {
        last_typed: Some(&typed_into("p1", start - Duration::from_secs(1))),
        ..tick(start + SETTLE)
    };
    assert_eq!(
        tracker.poll(scope("b1"), &focused, typed),
        Some("p1".into())
    );
}

#[test]
fn many_new_agents_move_one_at_a_time() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let names: Vec<String> = (0..10).map(|n| format!("p{n}")).collect();
    let agents: Vec<_> = names
        .iter()
        .map(|pane| (pane.as_str(), "claude", IDLE, 1))
        .collect();
    let many = snapshot(&agents);
    tracker.poll(scope("b1"), &many, tick(start));
    let mut moved = Vec::new();
    let mut at = start + SETTLE;
    for _ in 0..30 {
        // A move is under way every other tick.
        let busy = Tick {
            busy: true,
            ..tick(at)
        };
        assert_eq!(tracker.poll(scope("b1"), &many, busy), None);
        if let Some(pane) = tracker.poll(scope("b1"), &many, tick(at)) {
            moved.push(pane);
        }
        at += Duration::from_millis(500);
    }
    moved.sort();
    let mut expected = names.clone();
    expected.sort();
    assert_eq!(moved, expected);
}

#[test]
fn a_chat_moved_back_to_its_terminal_stays_there() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let new = snapshot(&[("p1", "claude", IDLE, 1)]);
    tracker.poll(scope("b1"), &new, tick(start));
    assert!(
        tracker
            .poll(scope("b1"), &new, tick(start + SETTLE))
            .is_some()
    );
    // The agent stops and its chat starts: for a moment the pane has none.
    let between = with_pane(snapshot(&[]), "p1");
    let mut chat = snapshot(&[("p1", "acp", IDLE, 2)]);
    chat.agents[0].tokens = vec![(sheprd_chat_protocol::TOKEN.into(), ENDPOINT.into())];
    // Moved back: Claude runs in the terminal again.
    let back = snapshot(&[("p1", "claude", IDLE, 3)]);
    let mut at = start + SETTLE;
    for snapshot in [&between, &chat, &between, &back] {
        at += SETTLE;
        assert_eq!(tracker.poll(scope("b1"), snapshot, tick(at)), None);
    }
    assert_eq!(
        tracker.poll(scope("b1"), &back, tick(at + SETTLE * 2)),
        None
    );
}

#[test]
fn a_busy_refusal_is_retried_a_bounded_number_of_times() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let new = snapshot(&[("p1", "claude", IDLE, 1)]);
    let mut at = start;
    tracker.poll(scope("b1"), &new, tick(at));
    for _ in 0..RETRIES {
        at += SETTLE;
        assert_eq!(tracker.poll(scope("b1"), &new, tick(at)), Some("p1".into()));
        assert!(tracker.retry("p1"));
        // Watched again from the start.
        assert_eq!(tracker.poll(scope("b1"), &new, tick(at)), None);
    }
    at += SETTLE;
    assert_eq!(tracker.poll(scope("b1"), &new, tick(at)), Some("p1".into()));
    assert!(!tracker.retry("p1"));
    assert_eq!(tracker.poll(scope("b1"), &new, tick(at + SETTLE * 2)), None);
}

#[test]
fn a_closed_pane_is_forgotten() {
    let start = Instant::now();
    let running = snapshot(&[("p1", "claude", IDLE, 1)]);
    let mut tracker = tracker(&running, start);
    tracker.poll(scope("b1"), &snapshot(&[]), tick(start));
    assert!(tracker.settled.is_empty());
    assert!(tracker.watching.is_empty());
}

#[test]
fn two_windows_on_one_daemon_move_a_new_agent_once() {
    // Each window keeps its own tracker, so both find the agent due at once;
    // the claim `convert_to_chat` takes lets only one of them move it.
    let claims = convert::claim::Claims::new();
    let start = Instant::now();
    let mut windows = [
        tracker(&snapshot(&[]), start),
        tracker(&snapshot(&[]), start),
    ];
    let new = snapshot(&[("p1", "claude", IDLE, 1)]);
    for tracker in &mut windows {
        tracker.poll(scope("b1"), &new, tick(start));
    }
    let due: Vec<_> = windows
        .iter_mut()
        .filter_map(|tracker| tracker.poll(scope("b1"), &new, tick(start + SETTLE)))
        .collect();
    assert_eq!(due, ["p1", "p1"]);
    let moves: Vec<_> = due
        .iter()
        .filter_map(|pane| claims.claim(&new.boot_id, pane))
        .collect();
    assert_eq!(moves.len(), 1, "exactly one window moves the agent");
    // The window that lost settled the pane as tried: it never comes back.
    for tracker in &mut windows {
        assert_eq!(
            tracker.poll(scope("b1"), &new, tick(start + SETTLE * 3)),
            None
        );
    }
}

#[test]
fn pointer_use_without_typing_lets_a_focused_agent_move() {
    // The window notes the pointer as use, but it carries no typing stamp.
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut focused = snapshot(&[("p1", "claude", IDLE, 1)]);
    focused.focused_pane_id = Some("p1".into());
    tracker.poll(scope("b1"), &focused, tick(start));
    assert_eq!(
        tracker.poll(scope("b1"), &focused, tick(start + SETTLE)),
        Some("p1".into())
    );
}

#[test]
fn typing_into_the_focused_pane_while_it_settles_keeps_its_terminal() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut focused = snapshot(&[("p1", "claude", IDLE, 1)]);
    focused.focused_pane_id = Some("p1".into());
    tracker.poll(scope("b1"), &focused, tick(start));
    // A key typed right at the deadline still wins over the move.
    let typed = Tick {
        last_typed: Some(&typed_into("p1", start + SETTLE)),
        ..tick(start + SETTLE)
    };
    assert_eq!(tracker.poll(scope("b1"), &focused, typed), None);
}

#[test]
fn typing_into_another_pane_then_focusing_the_agent_lets_it_move() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut focused = snapshot(&[("p1", "claude", IDLE, 1)]);
    focused.focused_pane_id = Some("p1".into());
    tracker.poll(scope("b1"), &focused, tick(start));
    // Typed into p2 while p1 settled, then p1 was clicked.
    let elsewhere = typed_into("p2", start + Duration::from_secs(2));
    let typed = Tick {
        last_typed: Some(&elsewhere),
        ..tick(start + SETTLE)
    };
    assert_eq!(
        tracker.poll(scope("b1"), &focused, typed),
        Some("p1".into())
    );
}

#[test]
fn typing_into_an_agent_that_lost_focus_still_keeps_its_terminal() {
    let start = Instant::now();
    let mut tracker = tracker(&snapshot(&[]), start);
    let mut other = snapshot(&[("p1", "claude", IDLE, 1)]);
    other.focused_pane_id = Some("p2".into());
    tracker.poll(scope("b1"), &other, tick(start));
    // Typed into p1, then focus moved on before the next poll.
    let draft = typed_into("p1", start + Duration::from_secs(2));
    let typed = Tick {
        last_typed: Some(&draft),
        ..tick(start + SETTLE)
    };
    assert_eq!(tracker.poll(scope("b1"), &other, typed), None);
}
