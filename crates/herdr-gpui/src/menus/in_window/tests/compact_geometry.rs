use super::*;

struct InsetWindow {
    view: Entity<HerdrWindow>,
    inset: Pixels,
}

impl Render for InsetWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p(self.inset).child(self.view.clone())
    }
}

#[gpui::test]
fn compact_popup_tracks_launcher_after_resizes_and_insets(cx: &mut TestAppContext) {
    let (root, cx) = cx.add_window_view(|window, cx| InsetWindow {
        view: cx.new(|cx| {
            let mut view = fixture_window(window, cx);
            view.config.show_menu_bar = false;
            view.focus.focus(window, cx);
            view
        }),
        inset: px(0.),
    });
    let view = root.read_with(cx, |root, _| root.view.clone());
    cx.simulate_resize(size(px(1200.), px(700.)));
    draw(cx);
    let button = cx.debug_bounds("application-menu-launcher").unwrap();
    cx.simulate_click(button.center(), Modifiers::none());
    for (width, inset) in [(360., 0.), (1200., 11.), (360., 11.), (1200., 0.)] {
        root.update(cx, |root, cx| {
            root.inset = px(inset);
            cx.notify();
        });
        cx.simulate_resize(size(px(width), px(700.)));
        for _ in 0..3 {
            draw(cx);
        }
        assert!(cx.debug_bounds("application-menu-bar").is_none());
        let launcher = cx.debug_bounds("application-menu-launcher").unwrap();
        let panel = cx.debug_bounds("application-menu-panel").unwrap();
        assert_eq!(panel.top(), launcher.bottom());
        assert!(panel.left() >= px(inset));
        assert!(panel.right() <= px(width - inset));
        assert!(panel.bottom() <= px(700. - inset));
        view.read_with(cx, |view, _| {
            assert_eq!(view.menu.application_bar.launcher.get(), launcher);
            assert_eq!(view.menu.page, Some(Page::Application));
        });
    }
}

#[gpui::test]
fn zen_f10_uses_current_content_edge_without_reviving_hidden_bar(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.simulate_resize(size(px(1000.), px(600.)));
    draw(cx);
    for show in [false, true] {
        view.update(cx, |view, cx| {
            view.config.show_menu_bar = show;
            cx.notify();
        });
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.command(crate::controls::Command::ToggleZenMode, window, cx);
            })
        });
        cx.run_until_parked();
        draw(cx);
        cx.simulate_keystrokes("f10");
        draw(cx);
        assert!(cx.debug_bounds("application-menu-bar").is_none());
        assert!(cx.debug_bounds("application-menu-launcher").is_none());
        assert!(cx.debug_bounds("application-item-File").is_some());
        let panel = cx.debug_bounds("application-menu-panel").unwrap();
        let content = view.read_with(cx, |view, _| view.menu.application_bar.content.get());
        assert!(panel.top() <= content.top() + px(8.));
        assert!(panel.bottom() <= content.bottom());
        cx.simulate_keystrokes("escape");
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.focus.is_focused(window));
                view.command(crate::controls::Command::ToggleZenMode, window, cx);
            })
        });
        cx.run_until_parked();
        draw(cx);
    }
}
