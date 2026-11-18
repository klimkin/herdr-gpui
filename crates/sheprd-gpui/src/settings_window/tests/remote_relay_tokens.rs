//! Settings > Remote control > Relays: a private relay's listen token,
//! asked for when a relay added wants one or its Token button is clicked,
//! and a relay a link brings with its token, added only on Add.

use super::remote_control::{Edits, click, draw, page};
use super::*;
use crate::{
    config::{preferences::Preference, remote::Edit},
    remote::relay_token::{self, ListenToken},
};
use std::cell::RefCell;

const TOKEN: &str = "0123456789abcdef-listen";

thread_local! {
    /// What the page asked to keep: each relay and its token, or `None`
    /// to forget it.
    static KEPT: RefCell<Vec<(String, Option<String>)>> = const { RefCell::new(Vec::new()) };
}

fn keep(relay: &str, token: Option<ListenToken>) -> Result<(), relay_token::Error> {
    let token = token.map(|token| token.expose().to_owned());
    KEPT.with(|kept| kept.borrow_mut().push((relay.to_owned(), token)));
    Ok(())
}

fn kept() -> Vec<(String, Option<String>)> {
    KEPT.with(|kept| kept.borrow_mut().drain(..).collect())
}

/// The page, with relays that all want a token and a recording keep.
fn private_page(
    cx: &mut TestAppContext,
) -> (Entity<SettingsWindow>, &mut VisualTestContext, Edits) {
    kept();
    let (view, cx, edits) = page(cx);
    view.update(cx, |view, _| {
        view.remote.token.check = |_| Ok(true);
        view.remote.token.keep = keep;
    });
    (view, cx, edits)
}

fn type_into(cx: &mut VisualTestContext, selector: &'static str, text: &str) {
    click(cx, selector);
    cx.simulate_input(text);
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn adding_a_private_relay_asks_for_its_token_and_keeps_a_valid_one(cx: &mut TestAppContext) {
    let (view, cx, edits) = private_page(cx);
    assert!(cx.debug_bounds("remote-relay-token").is_none());
    type_into(cx, "remote-relay-address", "https://private.example.org");
    click(cx, "remote-relay-add-button");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![
            "https://private.example.org".into()
        ]))]
    );
    assert!(cx.debug_bounds("remote-relay-token").is_some());

    // Too short: refused here, nothing kept.
    type_into(cx, "remote-relay-token-field", "short");
    click(cx, "remote-relay-token-save");
    assert!(kept().is_empty());
    assert!(view.read_with(cx, |view, _| view.remote.token.error.is_some()));

    view.update(cx, |view, cx| {
        view.ask_relay_token("https://private.example.org".into(), cx);
    });
    draw(cx);
    type_into(cx, "remote-relay-token-field", TOKEN);
    click(cx, "remote-relay-token-save");
    assert_eq!(
        kept(),
        [(
            "https://private.example.org".to_owned(),
            Some(TOKEN.to_owned())
        )]
    );
    assert!(cx.debug_bounds("remote-relay-token").is_none());
}

#[gpui::test]
fn an_open_relay_asks_for_nothing(cx: &mut TestAppContext) {
    let (view, cx, _) = private_page(cx);
    view.update(cx, |view, _| view.remote.token.check = |_| Ok(false));
    type_into(cx, "remote-relay-address", "https://open.example.org");
    click(cx, "remote-relay-add-button");
    assert!(cx.debug_bounds("remote-relay-token").is_none());
}

#[gpui::test]
fn a_listed_relays_token_button_asks_and_removing_it_forgets_its_token(cx: &mut TestAppContext) {
    let (view, cx, _) = private_page(cx);
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec!["https://relay.example.org".into()];
        cx.notify();
    });
    draw(cx);
    click(cx, "remote-relay-0-token");
    assert_eq!(
        view.read_with(cx, |view, _| view.remote.token.relay.clone()),
        Some("https://relay.example.org".to_owned())
    );
    click(cx, "remote-relay-token-cancel");
    assert!(cx.debug_bounds("remote-relay-token").is_none());
    assert!(kept().is_empty());

    click(cx, "remote-relay-0-remove");
    assert_eq!(kept(), [("https://relay.example.org".to_owned(), None)]);
}

#[gpui::test]
fn a_linked_relay_is_added_with_its_token_only_on_add(cx: &mut TestAppContext) {
    let (view, cx, edits) = private_page(cx);
    let offer = |view: &Entity<SettingsWindow>, cx: &mut VisualTestContext| {
        view.update(cx, |view, cx| {
            let token = ListenToken::parse(TOKEN).unwrap();
            view.offer_linked_relay("https://linked.example.org".into(), token, cx);
        });
        draw(cx);
    };
    offer(&view, cx);
    click(cx, "remote-relay-linked-dismiss");
    assert!(cx.debug_bounds("remote-relay-linked").is_none());
    assert!(kept().is_empty() && edits.lock().unwrap().is_empty());

    offer(&view, cx);
    click(cx, "remote-relay-linked-add");
    assert_eq!(
        kept(),
        [(
            "https://linked.example.org".to_owned(),
            Some(TOKEN.to_owned())
        )]
    );
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![
            "https://linked.example.org".into()
        ]))]
    );
    assert!(cx.debug_bounds("remote-relay-linked").is_none());
}

/// Every config write fails, as a full disk or a read-only file would.
fn failing_writes(view: &Entity<SettingsWindow>, cx: &mut VisualTestContext) {
    view.update(cx, |view, _| {
        view.controls.preference_io = Some(controls::preferences::PreferenceIo {
            write: Arc::new(|_| Err(std::io::Error::other("read-only").into())),
            load: Arc::new(skill_load),
        });
    });
}

#[gpui::test]
fn a_relay_whose_removal_failed_keeps_its_token(cx: &mut TestAppContext) {
    let (view, cx, _) = private_page(cx);
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec!["https://relay.example.org".into()];
        cx.notify();
    });
    draw(cx);
    failing_writes(&view, cx);
    click(cx, "remote-relay-0-remove");
    assert!(kept().is_empty());
}

#[gpui::test]
fn a_late_answer_never_takes_over_the_prompt_being_typed_in(cx: &mut TestAppContext) {
    let (view, cx, _) = private_page(cx);
    view.update(cx, |view, cx| {
        view.check_relay_token("https://first.example.org".into(), cx);
        // Before the first relay answers, the user opens another's token.
        view.ask_relay_token("https://second.example.org".into(), cx);
    });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.remote.token.relay.clone()),
        Some("https://second.example.org".to_owned())
    );

    // Nor does an answer about a relay removed meanwhile.
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec!["https://gone.example.org".into()];
        view.remote.token.relay = None;
        view.check_relay_token("https://gone.example.org".into(), cx);
        view.remove_remote_relay(0, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.remote.token.relay.clone()),
        None
    );
}

#[gpui::test]
fn a_linked_relay_stays_offered_until_its_list_is_saved(cx: &mut TestAppContext) {
    let (view, cx, edits) = private_page(cx);
    view.update(cx, |view, cx| {
        let token = ListenToken::parse(TOKEN).unwrap();
        view.offer_linked_relay("https://linked.example.org".into(), token, cx);
        // Another save is running: Add waits for it.
        view.saving = true;
        cx.notify();
    });
    draw(cx);
    click(cx, "remote-relay-linked-add");
    assert!(kept().is_empty());
    assert!(cx.debug_bounds("remote-relay-linked").is_some());

    // A save that fails keeps the offer, to add again.
    view.update(cx, |view, cx| {
        view.saving = false;
        cx.notify();
    });
    draw(cx);
    failing_writes(&view, cx);
    click(cx, "remote-relay-linked-add");
    assert_eq!(kept().len(), 1);
    assert!(edits.lock().unwrap().is_empty());
    assert!(cx.debug_bounds("remote-relay-linked").is_some());
}

#[gpui::test]
fn a_token_can_be_forgotten_while_the_daemon_is_off(cx: &mut TestAppContext) {
    let (view, cx, _) = private_page(cx);
    cx.update(|_, cx| crate::remote::Supervisor::set_health(cx, crate::remote::Health::Off, None));
    view.update(cx, |view, cx| {
        view.ask_relay_token("https://relay.example.org".into(), cx);
    });
    draw(cx);
    click(cx, "remote-relay-token-forget");
    assert_eq!(kept(), [("https://relay.example.org".to_owned(), None)]);
}

#[gpui::test]
fn a_newer_link_survives_the_earlier_ones_save(cx: &mut TestAppContext) {
    let (view, cx, edits) = private_page(cx);
    let offer = |view: &Entity<SettingsWindow>, cx: &mut VisualTestContext, relay: &str| {
        view.update(cx, |view, cx| {
            let token = ListenToken::parse(TOKEN).unwrap();
            view.offer_linked_relay(relay.into(), token, cx);
        });
    };
    offer(&view, cx, "https://first.example.org");
    view.update(cx, |view, cx| view.add_linked_relay(cx));
    // A second link arrives before the first relay's save finishes.
    offer(&view, cx, "https://second.example.org");
    cx.run_until_parked();
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![
            "https://first.example.org".into()
        ]))]
    );
    assert_eq!(
        view.read_with(cx, |view, _| view
            .remote
            .token
            .linked
            .clone()
            .map(|(relay, _)| relay)),
        Some("https://second.example.org".to_owned())
    );
}
