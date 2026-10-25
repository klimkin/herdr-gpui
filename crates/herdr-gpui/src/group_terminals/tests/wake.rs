use super::*;

#[gpui::test]
fn parked_projection_waits_for_connection_wake(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    draw(cx);
    let peer = MockPeer::new();
    let (wake, events) = crate::connection::ConnectionWake::channel();
    view.update(cx, |view, _| {
        let group = view.group_slots()[0].id;
        let key = view.browser_key().unwrap();
        let target = view.endpoints[0].connection.target.clone();
        let shown = (**view.live.snapshot.as_ref().unwrap()).clone();
        let mut parked = parked(group, key, target, &peer, shown);
        parked.connection.wake = wake.clone();
        {
            let mut inbox = parked.connection.inbox.lock().unwrap();
            *inbox = parked.live.clone();
            inbox.error = Some("parked arrival".into());
            inbox.dirty = true;
        }
        view.browser.terminals.parked.push(parked);
        assert!(!view.poll_group_terminals_with_updates(false));
        assert!(view.browser.terminals.parked[0].live.error.is_none());
        wake.notify();
        events.try_recv().unwrap();
        assert!(view.poll_group_terminals_with_updates(true));
        assert_eq!(
            view.browser.terminals.parked[0].live.error.as_deref(),
            Some("parked arrival")
        );
    });
}
