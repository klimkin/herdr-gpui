#![allow(clippy::unwrap_used)]

use crate::sidebar::layout_tests::fixture_window;
use gpui::TestAppContext;
use std::sync::{Arc, Mutex};

#[gpui::test]
fn rapid_visibility_choices_apply_at_once_and_persist_the_latest(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    let written = Arc::new(Mutex::new(Vec::new()));
    cx.update(|window, cx| {
        for show in [false, true, false, true, false] {
            let written = written.clone();
            view.update(cx, |view, cx| {
                view.set_menu_bar_with(
                    show,
                    move |value| {
                        written.lock().unwrap().push(value);
                        Ok(())
                    },
                    window,
                    cx,
                );
                assert_eq!(view.config.show_menu_bar, show);
            });
        }
    });
    cx.run_until_parked();
    assert_eq!(written.lock().unwrap().last(), Some(&false));
}

#[gpui::test]
fn changing_visibility_dismisses_popup_and_reports_save_failure(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.focus.focus(window, cx);
            view.open_application_menu(Some(0), window, cx);
            view.set_menu_bar_with(false, |_| Err(crate::Error::MissingHome), window, cx);
            assert!(view.menu.application.is_none());
            assert!(view.focus.is_focused(window));
        });
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(
            view.local_error
                .as_ref()
                .unwrap()
                .starts_with("Save GUI config:")
        );
    });
}
