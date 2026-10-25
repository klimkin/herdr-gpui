use super::*;

#[test]
fn publication_wakes_even_when_dirty_and_consumes_latest_state() {
    let (wake, events) = ConnectionWake::channel();
    let bridge = ConnectionBridge::new_with_wake(ConnectTarget::Local, wake.clone());
    for reason in ["first", "last"] {
        publish_event(
            &bridge.inbox,
            &wake,
            Some(ClientEvent::Disconnected {
                reason: reason.into(),
                ssh: None,
            }),
        );
    }
    assert_eq!(events.len(), 1);
    events.try_recv().unwrap();
    assert!(
        bridge.inbox.try_lock().is_ok(),
        "publisher must release inbox before waking"
    );
    assert_eq!(bridge.take_update().unwrap().error.as_deref(), Some("last"));
    assert!(bridge.take_update().is_none());
    publish_event(&bridge.inbox, &wake, None);
    events.try_recv().unwrap();
    assert!(
        bridge.take_update().is_some(),
        "side-channel answer wakes consumers"
    );
}

#[test]
fn busy_inbox_defers_consumed_wake_until_next_scheduler_turn() {
    let (wake, events) = ConnectionWake::channel();
    let bridge = ConnectionBridge::new_with_wake(ConnectTarget::Local, wake.clone());
    publish_event(
        &bridge.inbox,
        &wake,
        Some(ClientEvent::Disconnected {
            reason: "busy".into(),
            ssh: None,
        }),
    );
    events.try_recv().unwrap();
    let held = bridge.inbox.lock().unwrap();
    assert!(bridge.take_update().is_none());
    assert!(
        events.try_recv().is_err(),
        "contention must not spin the UI"
    );
    assert!(wake.take_deferred_inbox_read());
    assert!(!wake.take_deferred_inbox_read());
    drop(held);
    assert_eq!(bridge.take_update().unwrap().error.as_deref(), Some("busy"));
}

#[test]
fn startup_reset_and_retired_publishers_keep_current_inbox_fenced() {
    let (wake, events) = ConnectionWake::channel();
    let mut bridge = ConnectionBridge::new_with_wake(ConnectTarget::Local, wake.clone());
    let retired = bridge.inbox.clone();
    bridge.detach(false);
    events.try_recv().unwrap();
    bridge.take_update().unwrap();
    publish_event(
        &retired,
        &wake,
        Some(ClientEvent::Disconnected {
            reason: "retired".into(),
            ssh: None,
        }),
    );
    events.try_recv().unwrap();
    assert!(bridge.take_update().is_none());
    assert_eq!(
        bridge.inbox.lock().unwrap().status,
        ConnectionStatus::Detached
    );
    let mut options = ConnectOptions::default();
    options.surface_size.cols = 0;
    bridge.reconnect(options, false, true);
    events.try_recv().unwrap();
    assert_eq!(
        bridge.take_update().unwrap().status,
        ConnectionStatus::Disconnected
    );
}

#[test]
fn closed_window_never_blocks_connection_publication() {
    let (wake, events) = ConnectionWake::channel();
    drop(events);
    let bridge = ConnectionBridge::new_with_wake(ConnectTarget::Local, wake.clone());
    publish_event(
        &bridge.inbox,
        &wake,
        Some(ClientEvent::Disconnected {
            reason: "window closed".into(),
            ssh: None,
        }),
    );
    assert_eq!(
        bridge.take_update().unwrap().error.as_deref(),
        Some("window closed")
    );
}
