use super::*;

fn compact(cx: &mut TestAppContext) -> (Entity<HerdrWindow>, &mut VisualTestContext) {
    let (view, cx) = window(cx);
    view.update(cx, |view, cx| {
        view.config.show_menu_bar = false;
        cx.notify();
    });
    (view, cx)
}

#[gpui::test]
fn compact_launcher_reclaims_menu_row_and_restores_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.simulate_resize(size(px(1200.), px(700.)));
    draw(cx);
    let menu_height = cx.debug_bounds("application-menu-bar").unwrap().size.height;
    let before = cx.debug_bounds("terminal").unwrap();
    view.update(cx, |view, cx| {
        view.config.show_menu_bar = false;
        cx.notify();
    });
    draw(cx);
    assert!(cx.debug_bounds("application-menu-bar").is_none());
    assert!(cx.debug_bounds("application-menu-launcher").is_some());
    let after = cx.debug_bounds("terminal").unwrap();
    assert_eq!(after.top(), before.top() - menu_height);
    assert_eq!(after.size.height, before.size.height + menu_height);
    view.update(cx, |view, cx| {
        view.config.show_menu_bar = true;
        cx.notify();
    });
    draw(cx);
    assert!(cx.debug_bounds("application-menu-launcher").is_none());
    assert_eq!(cx.debug_bounds("terminal").unwrap(), before);
}

#[gpui::test]
fn launcher_and_f10_open_categories_and_restore_focus(cx: &mut TestAppContext) {
    let (view, cx) = compact(cx);
    cx.simulate_resize(size(px(1000.), px(600.)));
    draw(cx);
    let button = cx.debug_bounds("application-menu-launcher").unwrap();
    cx.simulate_click(button.center(), Modifiers::none());
    draw(cx);
    assert!(cx.debug_bounds("application-menu-bar").is_none());
    view.read_with(cx, |view, _| {
        assert!(view.menu.application.as_ref().unwrap().path.is_empty())
    });
    assert!(cx.debug_bounds("application-item-File").is_some());
    cx.simulate_keystrokes("a b c");
    view.read_with(cx, |view, _| {
        assert!(view.marked.is_empty());
        assert!(view.local_error.is_none());
        assert_eq!(view.menu.page, Some(Page::Application));
    });
    for dismiss in ["escape", "f10"] {
        cx.simulate_keystrokes(dismiss);
        cx.update(|window, cx| {
            assert!(view.read(cx).menu.page.is_none());
            assert!(view.read(cx).focus.is_focused(window));
        });
        draw(cx);
        cx.simulate_keystrokes("f10");
        draw(cx);
        assert!(cx.debug_bounds("application-item-File").is_some());
    }
    cx.simulate_click(button.center(), Modifiers::none());
    assert!(view.read_with(cx, |view, _| view.menu.page.is_none()));
}

#[gpui::test]
fn compact_menu_has_one_launcher_in_header_or_last_group(cx: &mut TestAppContext) {
    let (view, cx) = compact(cx);
    for settings in [
        "[ui]\ntab_bar_position = 'bottom'",
        "[ui]\ntab_bar_position = 'top'",
        "[ui]\ntab_bar_position = 'top'\nhide_tab_bar_when_single_tab = true",
    ] {
        view.update(cx, |view, cx| {
            view.settings.shared =
                Some(crate::herdr_settings::Settings::parse_text(settings).unwrap());
            cx.notify();
        });
        for width in [1200., 360.] {
            cx.simulate_resize(size(px(width), px(600.)));
            draw(cx);
            let button = cx.debug_bounds("application-menu-launcher").unwrap();
            assert!(
                button.left() >= px(0.) && button.right() <= px(width),
                "{settings} at width {width}: {button:?}"
            );
            let bar = cx
                .debug_bounds("titlebar")
                .or_else(|| cx.debug_bounds("strip-titlebar-trailing"))
                .unwrap();
            assert!(bar.contains(&button.center()));
        }
    }
    view.update(cx, |view, cx| {
        view.settings.shared = None;
        cx.notify();
    });
    cx.simulate_resize(size(px(1200.), px(600.)));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.command(crate::controls::Command::SplitEditor, window, cx);
        })
    });
    cx.run_until_parked();
    for width in [1200., 360.] {
        cx.simulate_resize(size(px(width), px(600.)));
        draw(cx);
        let button = cx.debug_bounds("application-menu-launcher").unwrap();
        assert!(button.left() >= px(0.) && button.right() <= px(width));
    }
}

#[gpui::test]
fn explicit_controls_move_from_menu_row_to_header(cx: &mut TestAppContext) {
    let (view, cx) = compact(cx);
    view.update(cx, |view, cx| {
        view.config.show_window_controls = true;
        cx.notify();
    });
    for width in [1200., 360.] {
        cx.simulate_resize(size(px(width), px(600.)));
        draw(cx);
        let launcher = cx.debug_bounds("application-menu-launcher").unwrap();
        let close = cx.debug_bounds("window-close").unwrap();
        assert!(close.right() <= px(width));
        assert!(close.left() > launcher.right());
    }
    view.update(cx, |view, cx| {
        view.config.show_menu_bar = true;
        cx.notify();
    });
    draw(cx);
    let bar = cx.debug_bounds("application-menu-bar").unwrap();
    assert!(bar.contains(&cx.debug_bounds("window-close").unwrap().center()));
}

#[gpui::test]
fn view_menu_checks_current_visibility(cx: &mut TestAppContext) {
    let (view, cx) = compact(cx);
    for show in [false, true] {
        view.update(cx, |view, _| view.config.show_menu_bar = show);
        cx.update(|window, cx| view.update(cx, |view, cx| {
            view.open_application_menu(Some(3), window, cx);
            let menu = view.menu.application.as_ref().unwrap();
            assert!(menu.current().unwrap().items.iter().any(|item| matches!(item,
                OwnedMenuItem::Action { name, checked, .. } if name == "Show Menu Bar" && *checked == show
            )));
            view.dismiss_menu(window, cx);
        }));
    }
}

#[gpui::test]
fn long_git_counts_yield_to_menu_controls_in_split_and_narrow_windows(cx: &mut TestAppContext) {
    let (view, cx) = compact(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.config.show_window_controls = true;
            view.git = crate::git::Git::fixture(
                crate::pull_request::Input {
                    checkout: None,
                    repo_key: Some(crate::sidebar::layout_tests::REPO_KEY.into()),
                    branch: "develop".into(),
                },
                crate::git::Status {
                    additions: u64::MAX,
                    deletions: u64::MAX,
                    untracked: 1,
                },
            );
            view.command(crate::controls::Command::SplitEditor, window, cx);
            cx.notify();
        })
    });
    cx.run_until_parked();
    for width in [1200., 640., 360.] {
        cx.simulate_resize(size(px(width), px(600.)));
        draw(cx);
        let menu = cx.debug_bounds("application-menu-launcher").unwrap();
        let close = cx.debug_bounds("window-close").unwrap();
        assert!(menu.left() >= px(0.) && menu.right() <= close.left());
        assert!(close.right() <= px(width));
        let room = cx
            .debug_bounds("titlebar-center")
            .or_else(|| cx.debug_bounds("strip-titlebar-room"))
            .unwrap();
        assert!(room.size.width >= px(40.));
    }
}
