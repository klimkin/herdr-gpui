//! Typing into a pane an automatic move has started on stops the move if it
//! has not pressed Ctrl+C, which would clear the draft; once it has, the
//! move finishes rather than strand the stopped agent.

use super::start::local_window;
use crate::{
    SheprdWindow,
    chat::{convert::Job, existing::Scope},
    sidebar::layout_tests::fixture_window,
    window::MockPeer,
};
use std::sync::Arc;

/// The scope of the window's current daemon.
fn scope(view: &SheprdWindow) -> Scope {
    let endpoint = &view.endpoints[view.selected_endpoint];
    Scope {
        target: endpoint.connection.target.clone(),
        boot: view.live.snapshot.as_ref().unwrap().boot_id.clone(),
        selection: view.selection_epoch,
        connection: endpoint.generation,
    }
}

/// Gives Herdr's focus to `pane`.
fn focus(view: &mut SheprdWindow, pane: &str) {
    let snapshot = view.live.snapshot.as_mut().unwrap();
    Arc::make_mut(snapshot).focused_pane_id = Some(pane.into());
}

#[gpui::test]
fn typing_into_the_moving_pane_stops_an_automatic_move(cx: &mut gpui::TestAppContext) {
    let peer = MockPeer::advertising(&[]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            local_window(view, &peer, cx);
            focus(view, "w1:p1");
            let (mut job, _worker, _cancelled) = Job::waiting("w1:p1", scope(view));
            job.automatic = true;
            let stage = job.stage.clone();
            view.chats.converting = Some(job);
            view.note_typing();
            assert!(stage.stopped_by_typing(), "no Ctrl+C on a draft");
            // The job stays, holding its claim, until the worker answers.
            assert!(view.chats.converting.is_some());
            let typed = view.last_typed.as_ref().unwrap();
            assert_eq!(typed.pane.as_deref(), Some("w1:p1"));
        })
    });
}

#[gpui::test]
fn typing_elsewhere_or_into_a_move_asked_for_lets_it_go_on(cx: &mut gpui::TestAppContext) {
    let peer = MockPeer::advertising(&[]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            local_window(view, &peer, cx);
            focus(view, "w1:p2");
            let (mut job, _worker, _cancelled) = Job::waiting("w1:p1", scope(view));
            job.automatic = true;
            let stage = job.stage.clone();
            view.chats.converting = Some(job);
            view.note_typing();
            assert!(!stage.stopped_by_typing(), "another pane's draft");
            assert_eq!(
                view.last_typed.as_ref().unwrap().pane.as_deref(),
                Some("w1:p2")
            );

            // A move the user asked for is theirs to interrupt.
            focus(view, "w1:p1");
            let (job, _worker, _cancelled) = Job::waiting("w1:p1", scope(view));
            let stage = job.stage.clone();
            view.chats.converting = Some(job);
            view.note_typing();
            assert!(!stage.stopped_by_typing());
        })
    });
}

#[gpui::test]
fn typing_after_the_first_ctrl_c_lets_the_move_finish(cx: &mut gpui::TestAppContext) {
    let peer = MockPeer::advertising(&[]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            local_window(view, &peer, cx);
            focus(view, "w1:p1");
            let (mut job, _worker, cancelled) = Job::waiting("w1:p1", scope(view));
            job.automatic = true;
            let stage = job.stage.clone();
            view.chats.converting = Some(job);
            // The worker pressed Ctrl+C, and the agent is on its way out.
            assert!(stage.commit());
            view.note_typing();
            assert!(
                !stage.stopped_by_typing(),
                "the stopped agent needs its chat"
            );
            assert!(!cancelled.load(std::sync::atomic::Ordering::Acquire));
        })
    });
}
