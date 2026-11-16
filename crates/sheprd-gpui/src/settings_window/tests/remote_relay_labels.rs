use super::remote_control::{click, draw, page};
use super::*;
use crate::config::{
    preferences::Preference,
    remote::{Edit, Relay},
};
use sheprd_remote_control::relay_label::RelayLabel;

fn labelled() -> Relay {
    Relay {
        endpoint: "https://fra3.example.org/".into(),
        label: RelayLabel {
            description: Some("not for public use".into()),
            name: Some("fra3".into()),
            tags: vec!["eu".into(), "private".into()],
        },
    }
}

#[gpui::test]
fn adding_a_relay_keeps_existing_labels(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec![labelled()];
        cx.notify();
    });
    draw(cx);
    click(cx, "remote-relay-address");
    cx.simulate_input("https://sfo3.example.org");
    cx.run_until_parked();
    draw(cx);
    click(cx, "remote-relay-add-button");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![
            labelled(),
            "https://sfo3.example.org".into(),
        ]))]
    );
}

#[gpui::test]
fn removing_a_relay_keeps_remaining_labels(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec!["https://sfo3.example.org".into(), labelled()];
        cx.notify();
    });
    draw(cx);
    click(cx, "remote-relay-0-remove");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![labelled()]))]
    );
}

#[gpui::test]
fn a_labelled_relay_is_matched_by_its_normalized_endpoint(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec![labelled()];
        cx.notify();
    });
    draw(cx);
    click(cx, "remote-relay-address");
    cx.simulate_input("https://fra3.example.org");
    cx.run_until_parked();
    draw(cx);
    click(cx, "remote-relay-add-button");
    assert!(edits.lock().unwrap().is_empty());
}
