//! Opt-in checks on a real Linux window, driven through GPUI's event dispatch.
//! This exercises native layout/painting, not compositor-delivered input or IME.

use super::*;
use anyhow::{Context as _, Result, ensure};
use std::time::Duration;

async fn settle(cx: &mut AsyncApp) {
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
}

async fn wait_for(
    handle: WindowHandle<HerdrWindow>,
    ready: impl Fn(&HerdrWindow, &Window) -> bool,
    cx: &mut AsyncApp,
) -> Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if handle.update(cx, |view, window, _| ready(view, window))? {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return handle.update(cx, |view, window, _| {
                anyhow::bail!("native menu did not settle: viewport {:?}, bar {:?}, launcher {:?}, cover {:?}, page {:?}",
                    window.viewport_size(), view.menu.application_bar.bar.get(),
                    view.menu.application_bar.launcher.get(), view.menu.cover.get(), view.menu.page)
            })?;
        }
        settle(cx).await;
    }
}

fn key(handle: WindowHandle<HerdrWindow>, key: &str, cx: &mut AsyncApp) -> Result<()> {
    AnyWindowHandle::from(handle).update(cx, |_, window, cx| {
        let event = KeyDownEvent {
            keystroke: Keystroke::parse(key)?,
            is_held: false,
            prefer_character_input: false,
        };
        window.dispatch_event(PlatformInput::KeyDown(event), cx);
        Ok(())
    })?
}

pub(crate) async fn run(handle: WindowHandle<HerdrWindow>, cx: &mut AsyncApp) -> Result<()> {
    for (width, show_menu_bar) in [(1200., true), (360., true), (1200., false), (360., false)] {
        handle.update(cx, |view, window, cx| {
            view.dismiss_menu(window, cx);
            view.sidebar_visible = false;
            view.config.show_menu_bar = show_menu_bar;
            window.resize(size(px(width), px(500.)));
            view.focus.focus(window, cx);
            cx.notify();
        })?;
        // Native dispatch needs the post-transition tree even when the
        // compositor withholds frame callbacks from an inactive window.
        AnyWindowHandle::from(handle).update(cx, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })?;
        wait_for(
            handle,
            |view, window| {
                let bounds = if show_menu_bar {
                    view.menu.application_bar.bar.get()
                } else {
                    view.menu.application_bar.launcher.get()
                };
                bounds.size.width > px(0.)
                    && bounds.left() >= px(0.)
                    && bounds.right() <= window.viewport_size().width
                    && (!show_menu_bar
                        || view.menu.application_bar.buttons.borrow().len()
                            == if view.compact_application_bar(window) {
                                1
                            } else {
                                super::super::menus(&view.config).len()
                            })
            },
            cx,
        )
        .await
        .with_context(|| format!("chrome at width {width}, show_menu_bar={show_menu_bar}"))?;
        let (button, compact) = handle.update(cx, |view, window, _| -> Result<_> {
            if !show_menu_bar {
                let button = view.menu.application_bar.launcher.get();
                ensure!(button.size.width > px(0.), "compact menu was not painted");
                ensure!(
                    button.left() >= px(0.) && button.right() <= window.viewport_size().width,
                    "compact menu clipped"
                );
                return Ok((button, true));
            }
            let bar = view.menu.application_bar.bar.get();
            ensure!(bar.size.width > px(0.), "menu bar was not painted");
            ensure!(
                bar.right() <= window.viewport_size().width,
                "menu bar overflow"
            );
            let buttons = view.menu.application_bar.buttons.borrow();
            ensure!(
                buttons.len()
                    == if view.compact_application_bar(window) {
                        1
                    } else {
                        super::super::menus(&view.config).len()
                    }
            );
            for button in buttons.iter() {
                ensure!(
                    button.left() >= bar.left() && button.right() <= bar.right(),
                    "heading clipped"
                );
            }
            Ok((
                buttons.first().copied().context("menu button missing")?,
                view.compact_application_bar(window),
            ))
        })??;
        handle.update(cx, |_, window, _| {
            eprintln!("APPLICATION MENUS native: requested width {width}, actual viewport {:?}, show_menu_bar={show_menu_bar}", window.viewport_size());
        })?;
        AnyWindowHandle::from(handle).update(cx, |_, window, cx| {
            let position = button.center();
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    position,
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                }),
                cx,
            );
        })?;
        wait_for(
            handle,
            |view, _| matches!(view.menu.cover.get(), crate::menu::Cover::Panel(_)),
            cx,
        )
        .await
        .with_context(|| format!("menu at width {width}, show_menu_bar={show_menu_bar}"))?;
        if compact {
            key(handle, "enter", cx)?;
            settle(cx).await;
        }
        handle.update(cx, |view, window, _| -> Result<()> {
            ensure!(
                view.menu.page == Some(Page::Application),
                "button did not open menu"
            );
            ensure!(view.menu.focus.is_focused(window), "menu lost focus");
            let crate::menu::Cover::Panel(panel) = view.menu.cover.get() else {
                anyhow::bail!("menu did not paint")
            };
            // The cover includes the eight-pixel shadow margin.
            ensure!(
                panel.right() <= window.viewport_size().width + px(8.),
                "dropdown horizontal overflow: {panel:?}, viewport {:?}, bar {:?}",
                window.viewport_size(),
                view.menu.application_bar.bar.get()
            );
            ensure!(
                panel.bottom() <= window.viewport_size().height + px(8.),
                "dropdown vertical overflow: {panel:?}, viewport {:?}, bar {:?}",
                window.viewport_size(),
                view.menu.application_bar.bar.get()
            );
            Ok(())
        })??;
        let before = handle.update(cx, |view, _, _| view.input_probe)?;
        key(handle, "a", cx)?;
        key(handle, "down", cx)?;
        key(handle, "up", cx)?;
        key(handle, "enter", cx)?; // About, from the shared native menu action.
        wait_for(handle, |view, _| view.menu.page == Some(Page::About), cx).await?;
        handle.update(cx, |view, window, cx| -> Result<()> {
            ensure!(
                view.menu.page == Some(Page::About),
                "menu action did not dispatch"
            );
            ensure!(
                view.input_probe.text == before.text && view.input_probe.keys == before.keys,
                "menu leaked terminal input"
            );
            view.dismiss_menu(window, cx);
            Ok(())
        })??;
        settle(cx).await;
        key(handle, "f10", cx)?;
        settle(cx).await;
        key(handle, "escape", cx)?;
        handle.update(cx, |view, window, _| -> Result<()> {
            ensure!(
                view.menu.page.is_none() && view.focus.is_focused(window),
                "dismissal lost focus"
            );
            Ok(())
        })??;
    }
    crate::menu::verify_shortcut_editor(handle, cx).await
}
