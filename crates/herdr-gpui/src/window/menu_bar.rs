//! Menu visibility shared by menus, the palette, and persisted config.
use super::{HerdrWindow, layout_save::LayoutSaves};
use crate::{
    app::InitialAppearance,
    config::{Config, preferences::Preference},
};
use gpui::{Context, Window};

impl HerdrWindow {
    pub(super) fn set_menu_bar(&mut self, show: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.set_menu_bar_with(
            show,
            |show| Config::save_preference(Preference::ShowMenuBar(show)),
            window,
            cx,
        );
    }

    fn set_menu_bar_with(
        &mut self,
        show: bool,
        save: impl FnOnce(bool) -> crate::Result<()> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.config.show_menu_bar == show {
            return;
        }
        if self.menu.application.is_some() {
            self.dismiss_menu(window, cx);
        }
        self.config.show_menu_bar = show;
        if cx.has_global::<InitialAppearance>() {
            cx.global_mut::<InitialAppearance>().config.show_menu_bar = show;
        }
        crate::menus::install(cx);
        cx.notify();
        let saves = cx.default_global::<LayoutSaves>().menu_bar.clone();
        saves.want(show);
        let saved = cx
            .background_executor()
            .spawn(async move { saves.write_latest(save) });
        cx.spawn(async move |this, cx| {
            if let Err(error) = saved.await {
                tracing::warn!(%error, "Could not save menu visibility");
                let _ = this.update(cx, |this, cx| {
                    this.local_error = Some(format!("Save GUI config: {error}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests;
