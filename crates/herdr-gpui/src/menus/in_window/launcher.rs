//! Compact menu access in the shared header and merged tab strip.
use super::*;
use crate::fonts::StyledFont;
use gpui::prelude::*;

impl HerdrWindow {
    pub(crate) fn compact_menu_controls(&self) -> bool {
        super::super::IN_WINDOW && !self.config.show_menu_bar && self.config.show_window_controls
    }

    pub(crate) fn render_application_launcher(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        if !super::super::IN_WINDOW || self.config.show_menu_bar {
            return None;
        }
        let active = self.menu.application.is_some();
        let anchor = self.menu.application_bar.launcher.clone();
        let view = cx.entity().downgrade();
        Some(
            div()
                .id("application-menu-launcher")
                .debug_selector(|| "application-menu-launcher".into())
                .role(Role::Button)
                .aria_label("Menu")
                .aria_expanded(active)
                .on_a11y_action(
                    AccessibleAction::Expand,
                    crate::accessibility::update(view.clone(), |this, window, cx| {
                        this.open_application_menu(None, window, cx)
                    }),
                )
                .on_a11y_action(
                    AccessibleAction::Collapse,
                    crate::accessibility::update(view, |this, window, cx| {
                        this.dismiss_menu(window, cx)
                    }),
                )
                .map(|button| {
                    self.chrome_stop(crate::window::chrome_focus::Region::TitleBar, button)
                })
                .block_mouse_except_scroll()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .h(px(28.))
                .px(px(8.))
                .text_font(&self.config.ui)
                .text_size(px(self.config.ui.size))
                .text_color(rgb(self.theme.foreground))
                .rounded(px(crate::config::corners::CONTROL))
                .when(active, |button| button.bg(rgb(self.theme.active)))
                .hover(|button| button.bg(rgb(self.theme.active)))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    if this.menu.application.is_some() {
                        this.dismiss_menu(window, cx);
                    } else {
                        this.open_application_menu(None, window, cx);
                    }
                }))
                .child("Menu")
                .child(
                    canvas(
                        move |bounds, window, _| {
                            if anchor.replace(bounds) != bounds {
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                ),
        )
    }
}
