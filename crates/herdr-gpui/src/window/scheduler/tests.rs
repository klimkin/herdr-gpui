#![allow(clippy::unwrap_used)]
use super::*;
use crate::connection::ConnectionWake;
use futures_lite::future::{block_on, pending, poll_once};

#[test]
fn connection_delivery_does_not_wait_for_maintenance() {
    let (wake, events) = ConnectionWake::channel();
    block_on(async {
        let mut wait = std::pin::pin!(next_tick(&events, pending()));
        assert!(poll_once(wait.as_mut()).await.is_none());
        wake.notify();
        assert_eq!(wait.await, Some(Tick::Connection));
    });
}

#[test]
fn overdue_maintenance_wins_without_consuming_connection_token() {
    let (wake, events) = ConnectionWake::channel();
    wake.notify();
    assert_eq!(
        block_on(next_tick(&events, async {})),
        Some(Tick::Maintenance)
    );
    assert_eq!(
        block_on(next_tick(&events, pending())),
        Some(Tick::Connection)
    );
}

#[test]
fn closed_event_receiver_finishes_wait_with_frozen_maintenance() {
    let (wake, events) = ConnectionWake::channel();
    drop(wake);
    assert_eq!(block_on(next_tick(&events, pending())), None);
}

#[gpui::test]
fn scheduler_projects_connection_with_maintenance_frozen(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let (wake, events) = ConnectionWake::channel();
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.endpoints[0].never_connect();
            view.connection_wake = wake.clone();
            view.endpoints[0].connection.wake = wake.clone();
            view._poll = HerdrWindow::spawn_scheduler(
                window,
                cx,
                events,
                wake.clone(),
                Duration::from_secs(3600),
            );
        });
    });
    cx.run_until_parked();
    view.update(cx, |view, _| {
        view.endpoints[0].connection.inbox.lock().unwrap().apply(
            herdr_client::ClientEvent::Disconnected {
                reason: "event arrived".into(),
                ssh: None,
            },
        );
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_ne!(view.live.error.as_deref(), Some("event arrived"))
    });
    wake.notify();
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.live.error.as_deref(), Some("event arrived"))
    });
}
