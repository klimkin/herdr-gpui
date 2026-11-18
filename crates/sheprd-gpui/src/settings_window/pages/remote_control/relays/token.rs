//! A private relay's listen token: asked for when a relay added here says
//! it wants one, or when a relay's Token button is clicked, and offered
//! with its relay when a `sheprd://relay` link brings both. The token goes
//! to the daemon's private file (`crate::remote::relay_token`), never to
//! the config, and on to the user's other computers with the relay.

use super::{SettingsWindow, ui};
use crate::{
    config::{
        preferences::Preference,
        remote::{Edit, Relay},
    },
    i18n::{Localize, fl},
    relay_sharing::RelaySharing,
    remote::relay_token::{self, ListenToken},
    search_input::SearchInput,
};
use gpui::{prelude::*, *};

/// Asks a relay whether it wants a token; blocking.
pub(in crate::settings_window) type Check = fn(&str) -> Result<bool, relay_token::Error>;
/// Keeps a relay's token, or with `None` forgets it; blocking.
pub(in crate::settings_window) type Keep =
    fn(&str, Option<ListenToken>) -> Result<(), relay_token::Error>;

/// The token being asked for, if any, and a relay a link offers.
pub(in crate::settings_window) struct TokenPrompt {
    /// The relay, as `[remote] relays` names it, whose token is asked for.
    pub(in crate::settings_window) relay: Option<String>,
    input: Option<Entity<SearchInput>>,
    /// Why the last token was not kept, in the user's language.
    pub(in crate::settings_window) error: Option<String>,
    /// A token is being kept.
    saving: bool,
    /// Bumped by each relay checked, each prompt opened, and each relay
    /// removed, so a relay's late answer never takes over a newer prompt.
    checks: u64,
    /// A relay and its token from a link, until the user adds or dismisses it.
    pub(in crate::settings_window) linked: Option<(String, ListenToken)>,
    pub(in crate::settings_window) check: Check,
    pub(in crate::settings_window) keep: Keep,
}

impl Default for TokenPrompt {
    fn default() -> Self {
        // Tests never reach a relay or this user's daemon: they put their
        // own in.
        #[cfg(test)]
        let (check, keep): (Check, Keep) = (|_| Ok(false), |_, _| Ok(()));
        #[cfg(not(test))]
        let (check, keep): (Check, Keep) = (relay_token::wants_token, relay_token::save);
        Self {
            relay: None,
            input: None,
            error: None,
            saving: false,
            checks: 0,
            linked: None,
            check,
            keep,
        }
    }
}

impl SettingsWindow {
    /// Asks for `relay`'s token in a row under the list.
    pub(in crate::settings_window) fn ask_relay_token(
        &mut self,
        relay: String,
        cx: &mut Context<Self>,
    ) {
        let prompt = &mut self.remote.token;
        prompt.checks += 1;
        prompt.relay = Some(relay);
        prompt.error = None;
        let input = prompt
            .input
            .get_or_insert_with(|| {
                let input = cx.new(SearchInput::new);
                input.update(cx, |input, cx| {
                    input.set_masked(true, cx);
                    input.set_placeholder(&fl!("settings-remote-relay-token-placeholder"), cx);
                });
                input
            })
            .clone();
        let (font, theme) = (self.config.terminal.clone(), self.theme.clone());
        input.update(cx, |input, cx| {
            input.clear(cx);
            input.set_appearance(font, theme, cx);
        });
        cx.notify();
    }

    /// Asks `address`, just added, whether it wants a token, off the UI
    /// thread, and asks the user for it when it does. A relay that does not
    /// answer stays listed, and its row says how it stands.
    pub(in crate::settings_window) fn check_relay_token(
        &mut self,
        address: String,
        cx: &mut Context<Self>,
    ) {
        let prompt = &mut self.remote.token;
        prompt.checks += 1;
        let (check, current) = (prompt.check, prompt.checks);
        let asked = address.clone();
        let work = cx.background_executor().spawn(async move { check(&asked) });
        cx.spawn(async move |this, cx| {
            let wants = work.await;
            let _ = this.update(cx, |this, cx| match wants {
                // Only while nothing newer was checked, opened, or removed:
                // a prompt the user is typing in is never replaced.
                Ok(true) if this.remote.token.checks == current => {
                    this.ask_relay_token(address, cx);
                }
                Ok(_) => {}
                Err(error) => tracing::info!(%error, "Could not ask a relay whether it is private"),
            });
        })
        .detach();
    }

    fn close_relay_token(&mut self, cx: &mut Context<Self>) {
        let prompt = &mut self.remote.token;
        prompt.relay = None;
        prompt.error = None;
        if let Some(input) = &prompt.input {
            input.update(cx, |input, cx| input.clear(cx));
        }
        cx.notify();
    }

    /// Keeps `token` for `relay`, or forgets it, off the UI thread; then
    /// `done` runs once it is kept, and every other computer hears of it.
    fn keep_relay_token(
        &mut self,
        relay: String,
        token: Option<ListenToken>,
        done: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let keep = self.remote.token.keep;
        self.remote.token.saving = true;
        self.remote.token.error = None;
        cx.notify();
        let work = cx
            .background_executor()
            .spawn(async move { keep(&relay, token) });
        cx.spawn(async move |this, cx| {
            let kept = work.await;
            let _ = this.update(cx, |this, cx| {
                this.remote.token.saving = false;
                match kept {
                    Ok(()) => {
                        RelaySharing::tokens_changed(cx);
                        done(this, cx);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Could not keep a relay's listen token");
                        this.remote.token.error = Some(error.localize());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Save's click: the token typed, once it could be one.
    pub(in crate::settings_window) fn save_relay_token(&mut self, cx: &mut Context<Self>) {
        let (Some(relay), Some(input)) =
            (self.remote.token.relay.clone(), &self.remote.token.input)
        else {
            return;
        };
        let Ok(token) = ListenToken::parse(input.read(cx).text()) else {
            self.remote.token.error = Some(fl!("settings-remote-relay-token-invalid"));
            cx.notify();
            return;
        };
        self.keep_relay_token(relay, Some(token), Self::close_relay_token, cx);
    }

    /// Forget's click: the relay admits this computer no longer.
    pub(in crate::settings_window) fn forget_relay_token(&mut self, cx: &mut Context<Self>) {
        let Some(relay) = self.remote.token.relay.clone() else {
            return;
        };
        self.keep_relay_token(relay, None, Self::close_relay_token, cx);
    }

    /// Saves `relays`, the list without `removed`, then forgets `removed`'s
    /// token, only once the list is saved: a failed save keeps the relay
    /// listed, and its token with it. Quietly: the relay is gone either way.
    pub(in crate::settings_window) fn remove_relay_and_token(
        &mut self,
        relays: Vec<Relay>,
        removed: String,
        cx: &mut Context<Self>,
    ) {
        // An answer about the removed relay, still on its way, is dropped.
        self.remote.token.checks += 1;
        if self.remote.token.relay.as_deref() == Some(removed.as_str()) {
            self.close_relay_token(cx);
        }
        let keep = self.remote.token.keep;
        let forget = move |cx: &mut Context<Self>| {
            cx.background_executor()
                .spawn(async move {
                    if let Err(error) = keep(&removed, None) {
                        tracing::debug!(%error, "Could not forget a removed relay's token");
                    }
                })
                .detach();
        };
        self.save_preference_then(Preference::Remote(Edit::Relays(relays)), forget, cx);
    }

    /// Shows a relay and token a link brought, for the user to add.
    pub(in crate::settings_window) fn offer_linked_relay(
        &mut self,
        relay: String,
        token: ListenToken,
        cx: &mut Context<Self>,
    ) {
        self.remote.token.linked = Some((relay, token));
        self.remote.token.error = None;
        cx.notify();
    }

    /// Add's click on a linked relay: its token first, so the daemon the
    /// new list restarts already has it, then the relay. The offer stays
    /// until the list naming the relay is saved, so a save that could not
    /// run, or failed, leaves Add to click again.
    pub(in crate::settings_window) fn add_linked_relay(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some((relay, token)) = self.remote.token.linked.clone() else {
            return;
        };
        let offer = (relay.clone(), token.clone());
        let address = relay.clone();
        self.keep_relay_token(
            relay,
            Some(token),
            move |this, cx| {
                if this.lists_relay(&address) {
                    this.drop_linked(&offer);
                    return;
                }
                let mut relays = this.config.remote.relays.clone();
                relays.push(address.into());
                // Runs inside the save's own update, so the offer is
                // dropped just after it.
                let added = move |cx: &mut Context<Self>| {
                    cx.spawn(async move |this, cx| {
                        let _ = this.update(cx, |this, cx| {
                            this.drop_linked(&offer);
                            cx.notify();
                        });
                    })
                    .detach();
                };
                this.save_preference_then(Preference::Remote(Edit::Relays(relays)), added, cx);
            },
            cx,
        );
    }

    /// Drops the linked offer, unless a newer link replaced `offer` since
    /// Add was clicked: that one stays to add.
    fn drop_linked(&mut self, offer: &(String, ListenToken)) {
        if self.remote.token.linked.as_ref() == Some(offer) {
            self.remote.token.linked = None;
        }
    }

    /// The row asking for a token, if one is asked for.
    pub(in crate::settings_window) fn relay_token_row(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let prompt = &self.remote.token;
        let relay = prompt.relay.as_ref()?;
        let theme = &self.theme;
        let idle = !prompt.saving;
        let shown = crate::notifications::safe_text(relay, 200);
        let field = div()
            .debug_selector(|| "remote-relay-token-field".into())
            .w(px(220.))
            .flex_none()
            .children(prompt.input.clone());
        let save = ui::button(
            theme,
            "remote-relay-token-save",
            fl!("settings-remote-relay-token-save"),
            idle,
        )
        .when(idle, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.save_relay_token(cx)))
        });
        // Offered whether or not the daemon runs: the token is in a file,
        // and forgetting one never kept does nothing.
        let forget = ui::button(
            theme,
            "remote-relay-token-forget",
            fl!("settings-remote-relay-token-forget"),
            idle,
        )
        .when(idle, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.forget_relay_token(cx)))
        });
        let cancel = ui::button(
            theme,
            "remote-relay-token-cancel",
            fl!("settings-remote-relay-token-cancel"),
            idle,
        )
        .when(idle, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.close_relay_token(cx)))
        });
        let row = ui::row(
            theme,
            "remote-relay-token",
            fl!("settings-remote-relay-token", relay = shown),
        )
        .description(
            prompt
                .error
                .clone()
                .unwrap_or_else(|| fl!("settings-remote-relay-token-note")),
        )
        .control(field)
        .control(save)
        .control(forget)
        .control(cancel);
        Some(row.into_any_element())
    }

    /// The row offering a relay a link brought, if any.
    pub(in crate::settings_window) fn linked_relay_row(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let prompt = &self.remote.token;
        let (relay, _) = prompt.linked.as_ref()?;
        let theme = &self.theme;
        // Not while another save runs: the list naming the relay would not
        // be saved.
        let idle = !prompt.saving && !self.busy();
        let add = ui::button(
            theme,
            "remote-relay-linked-add",
            fl!("settings-remote-relay-add-button"),
            idle,
        )
        .when(idle, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.add_linked_relay(cx)))
        });
        let dismiss = ui::button(
            theme,
            "remote-relay-linked-dismiss",
            fl!("settings-remote-relay-linked-dismiss"),
            idle,
        )
        .when(idle, |button| {
            button.on_click(cx.listener(|this, _, _, cx| {
                this.remote.token.linked = None;
                this.remote.token.error = None;
                cx.notify();
            }))
        });
        let note = prompt.error.clone().unwrap_or_else(|| {
            fl!(
                "settings-remote-relay-linked-note",
                relay = crate::notifications::safe_text(relay, 200)
            )
        });
        Some(
            ui::row(
                theme,
                "remote-relay-linked",
                fl!("settings-remote-relay-linked"),
            )
            .description(note)
            .control(add)
            .control(dismiss)
            .into_any_element(),
        )
    }
}
