//! Typing into a pane is told apart from other use of the window: pointer
//! motion, wheels, and clicks are use, but only keys, text, and pastes can
//! leave a draft that `[chat] auto_convert` must not cut off.

use super::pending_input::typing;
use crate::sidebar::layout_tests::fixture_window;
use gpui::{MouseMoveEvent, ScrollEvent, TestAppContext, point, px};
use sheprd_client::protocol::{ClientMouseKind, ClientMousePosition, ClientPaneInputEvent};

#[test]
fn keys_text_and_pastes_type_and_the_pointer_does_not() {
    assert!(typing(&ClientPaneInputEvent::TextCommit("x".into())));
    assert!(typing(&ClientPaneInputEvent::Paste("x".into())));
    let pointer = ClientPaneInputEvent::Mouse {
        kind: ClientMouseKind::Moved,
        position: ClientMousePosition::Cell { column: 1, row: 1 },
        geometry: None,
        modifiers: 0,
        lines: 0,
    };
    assert!(!typing(&pointer));
}

#[gpui::test]
fn moving_the_pointer_or_the_wheel_is_use_but_not_typing(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let moved = MouseMoveEvent {
                position: point(px(40.), px(30.)),
                ..Default::default()
            };
            view.pane_pointer_moved(&moved, window, cx);
            let wheel = ScrollEvent {
                position: point(px(40.), px(30.)),
                ..Default::default()
            };
            view.scroll_wheel(&wheel, window, cx);
            assert!(view.last_input.is_some(), "the pointer is use");
            assert!(view.last_typed.is_none(), "the pointer types nothing");
        })
    });
}

#[gpui::test]
fn a_paste_is_typing(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    view.update(cx, |view, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("draft".into()));
        view.paste(cx);
        assert!(view.last_typed.is_some(), "a paste can leave a draft");
        assert!(view.last_input.is_some(), "and it is use too");
    });
}
