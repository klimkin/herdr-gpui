use super::*;

fn offer(source: &str, relays: &[&str]) -> Offer {
    Offer {
        source: source.into(),
        relays: relays.iter().map(|relay| (*relay).to_owned()).collect(),
        ..Offer::default()
    }
}

fn names(shared: &Shared) -> Vec<String> {
    shared.endpoints().iter().map(ToString::to_string).collect()
}

#[test]
fn an_offer_replaces_what_its_source_shared_before_and_leaves_others() {
    let mut shared = Shared::default();
    assert!(
        shared
            .offer(offer("studio", &["relay.example.org", "b.example.org"]))
            .unwrap()
    );
    assert!(shared.offer(offer("laptop", &["c.example.org"])).unwrap());
    assert!(shared.offer(offer("studio", &["b.example.org"])).unwrap());
    assert_eq!(
        names(&shared),
        ["https://c.example.org", "https://b.example.org"]
    );
    // The same relays again change nothing.
    assert!(
        !shared
            .offer(offer("studio", &["https://b.example.org"]))
            .unwrap()
    );
}

#[test]
fn an_empty_offer_withdraws_its_source() {
    let mut shared = Shared::default();
    shared
        .offer(offer("studio", &["relay.example.org"]))
        .unwrap();
    assert!(shared.offer(offer("studio", &[])).unwrap());
    assert!(shared.endpoints().is_empty());
    assert!(!shared.offer(offer("studio", &[])).unwrap());
}

#[test]
fn relays_named_twice_are_listened_on_once() {
    let mut shared = Shared::default();
    shared
        .offer(offer(
            "studio",
            &["relay.example.org", "wss://relay.example.org/"],
        ))
        .unwrap();
    shared
        .offer(offer("laptop", &["https://relay.example.org"]))
        .unwrap();
    assert_eq!(names(&shared), ["https://relay.example.org"]);
}

#[test]
fn a_bad_offer_is_refused_whole() {
    let mut shared = Shared::default();
    shared
        .offer(offer("studio", &["relay.example.org"]))
        .unwrap();
    let refused = shared.offer(offer("studio", &["b.example.org", "http://203.0.113.9"]));
    assert!(matches!(refused, Err(Error::Endpoint { .. })));
    assert!(matches!(
        shared.offer(offer(" ", &["b.example.org"])),
        Err(Error::Source)
    ));
    assert_eq!(names(&shared), ["https://relay.example.org"]);
}

#[test]
fn sources_and_their_relays_are_bounded() {
    let mut shared = Shared::default();
    for source in 0..MAX_SOURCES {
        shared
            .offer(offer(&format!("s{source}"), &["relay.example.org"]))
            .unwrap();
    }
    let refused = shared.offer(offer("one-more", &["relay.example.org"]));
    assert!(matches!(refused, Err(Error::TooManySources)));
    // A source already known may still change what it shares.
    assert!(shared.offer(offer("s0", &["b.example.org"])).unwrap());

    let many: Vec<String> = (0..MAX_RELAYS + 4)
        .map(|n| format!("r{n}.example.org"))
        .collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    let mut one = Shared::default();
    one.offer(offer("studio", &many)).unwrap();
    assert_eq!(one.endpoints().len(), MAX_RELAYS);
}

fn token(text: &str) -> ListenToken {
    ListenToken::parse(text).unwrap()
}

#[test]
fn a_listen_token_is_checked_as_a_relay_takes_it() {
    assert!(ListenToken::parse("0123456789abcdef").is_ok());
    assert!(matches!(ListenToken::parse("short"), Err(Error::Token)));
    assert!(matches!(
        ListenToken::parse("0123456789 abcdef"),
        Err(Error::Token)
    ));
    assert!(ListenToken::parse(&"x".repeat(MAX_TOKEN)).is_ok());
    assert!(ListenToken::parse(&"x".repeat(MAX_TOKEN + 1)).is_err());
    assert_eq!(
        format!("{:?}", token("0123456789abcdef-secret")),
        "ListenToken(…)"
    );
}

#[test]
fn a_token_is_kept_only_beside_the_relay_it_is_for_and_goes_with_it() {
    let mut shared = Shared::default();
    let mut with_token = offer("studio", &["relay.example.org"]);
    with_token.tokens = [
        ("relay.example.org".to_owned(), token("0123456789abcdef-a")),
        ("other.example.org".to_owned(), token("0123456789abcdef-b")),
    ]
    .into();
    assert!(shared.offer(with_token.clone()).unwrap());
    let relay = Endpoint::parse("https://relay.example.org").unwrap();
    let other = Endpoint::parse("other.example.org").unwrap();
    assert_eq!(
        shared.token(&relay).map(ListenToken::expose),
        Some("0123456789abcdef-a")
    );
    assert_eq!(shared.token(&other), None);
    // The same offer again changes nothing; a new token does.
    assert!(!shared.offer(with_token.clone()).unwrap());
    with_token.tokens = [("relay.example.org".to_owned(), token("0123456789abcdef-c"))].into();
    assert!(shared.offer(with_token).unwrap());
    // Without its token, the relay stays and the token goes.
    assert!(
        shared
            .offer(offer("studio", &["relay.example.org"]))
            .unwrap()
    );
    assert_eq!(shared.token(&relay), None);
    assert_eq!(names(&shared), ["https://relay.example.org"]);
}

#[test]
fn an_older_offer_or_file_without_tokens_still_reads() {
    let offer: Offer =
        serde_json::from_str(r#"{"source":"studio","relays":["relay.example.org"]}"#).unwrap();
    assert!(offer.tokens.is_empty());
    let shared: Shared =
        serde_json::from_str(r#"{"sources":{"studio":["https://relay.example.org"]}}"#).unwrap();
    assert_eq!(names(&shared), ["https://relay.example.org"]);
    // And an offer with none sends no field an older daemon would see.
    let sent = serde_json::to_value(offer).unwrap();
    assert!(sent.get("tokens").is_none());
}

#[test]
fn a_file_keeps_tokens_only_for_relays_their_source_still_shares() {
    let text = r#"{"sources":{"studio":["https://relay.example.org"]},
        "tokens":{"studio":{"https://other.example.org":"0123456789abcdef-a"},
                  "gone":{"https://relay.example.org":"0123456789abcdef-b"}}}"#;
    let shared = serde_json::from_str::<Shared>(text).unwrap().bounded();
    assert_eq!(
        shared.token(&Endpoint::parse("relay.example.org").unwrap()),
        None
    );
    assert!(!serde_json::to_string(&shared).unwrap().contains("tokens"));
}
