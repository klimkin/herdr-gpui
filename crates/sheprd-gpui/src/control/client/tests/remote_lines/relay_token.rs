use super::*;
use crate::control::{client, listen_token};
use sheprd_remote_control::control::{RelayState, RelayStatus};

#[test]
fn status_says_which_relays_are_listened_on_with_a_token() {
    let mut status = status();
    let mut private: RelayStatus = RelayState::Listening { rtt_ms: Some(12) }.into();
    private.token = true;
    status
        .relays
        .insert("https://private.example.org".into(), private);
    status.relays.insert(
        "https://public.example.org".into(),
        RelayState::Connecting.into(),
    );
    let output = remote::output(&Reply::Status {
        enabled: true,
        status: Some(Box::new(status)),
        error: None,
    })
    .unwrap();
    assert!(
        output
            .text
            .contains("Relay: https://private.example.org — listening, 12 ms, with a listen token")
    );
    assert!(
        output
            .text
            .lines()
            .any(|line| line == "Relay: https://public.example.org — connecting")
    );
    let relays = &output.json["status"]["relays"];
    assert_eq!(relays["https://private.example.org"]["token"], true);
    // Spelled out even though the daemon leaves a false one out.
    assert_eq!(relays["https://public.example.org"]["token"], false);
}

#[test]
fn a_kept_or_forgotten_token_names_the_relay_and_never_the_token() {
    let kept = remote::output(&Reply::RelayToken {
        relay: "https://relay.example.org".into(),
        token: true,
    })
    .unwrap();
    assert_eq!(kept.status, 0);
    assert_eq!(
        kept.text,
        "Kept this computer's listen token for https://relay.example.org"
    );
    assert_eq!(
        kept.json,
        serde_json::json!({ "relay": "https://relay.example.org", "token": true })
    );
    let forgotten = remote::output(&Reply::RelayToken {
        relay: "https://relay.example.org".into(),
        token: false,
    })
    .unwrap();
    assert_eq!(
        forgotten.text,
        "Forgot this computer's listen token for https://relay.example.org"
    );
    assert_eq!(forgotten.json["token"], false);
}

#[test]
fn a_bad_piped_token_is_a_usage_error_and_a_failed_read_is_not() {
    for error in [
        listen_token::Error::Empty,
        listen_token::Error::Terminal,
        listen_token::Error::Text,
        listen_token::Error::TooLong,
    ] {
        assert_eq!(client::status(&error.into()), exit::USAGE);
    }
    let read = listen_token::Error::Read(std::io::Error::other("closed"));
    assert_eq!(client::status(&read.into()), exit::REFUSED);
}
