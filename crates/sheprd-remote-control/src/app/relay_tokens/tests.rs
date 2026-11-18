use super::*;

fn token(text: &str) -> ListenToken {
    ListenToken::parse(text).unwrap()
}

#[test]
fn a_token_is_kept_by_endpoint_however_the_relay_is_spelled() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = Endpoint::parse("relay.example.org").unwrap();
    set(dir.path(), &endpoint, Some(token("0123456789abcdef-x"))).unwrap();
    let tokens = load(&file(dir.path()));
    assert_eq!(
        tokens
            .for_relay("https://relay.example.org")
            .map(ListenToken::expose),
        Some("0123456789abcdef-x")
    );
    assert_eq!(tokens.for_relay("other.example.org"), None);

    set(dir.path(), &endpoint, None).unwrap();
    assert_eq!(load(&file(dir.path())), Tokens::default());
}

#[cfg(unix)]
#[test]
fn the_tokens_file_is_readable_by_this_user_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let endpoint = Endpoint::parse("relay.example.org").unwrap();
    set(dir.path(), &endpoint, Some(token("0123456789abcdef-x"))).unwrap();
    let mode = fs::metadata(file(dir.path())).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn tokens_are_bounded_and_a_damaged_file_keeps_none() {
    let mut tokens = Tokens::default();
    for index in 0..MAX_TOKENS {
        let endpoint = Endpoint::parse(&format!("r{index}.example.org")).unwrap();
        tokens
            .set(&endpoint, Some(token("0123456789abcdef-x")))
            .unwrap();
    }
    let one_more = Endpoint::parse("more.example.org").unwrap();
    assert!(matches!(
        tokens.set(&one_more, Some(token("0123456789abcdef-x"))),
        Err(crate::app::Error::RelayTokens { max: MAX_TOKENS })
    ));
    // Replacing one already kept is fine at the bound.
    let first = Endpoint::parse("r0.example.org").unwrap();
    tokens
        .set(&first, Some(token("fedcba9876543210-y")))
        .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.json");
    fs::write(&path, br#"{"tokens":{"https://a.example.org":"short"}}"#).unwrap();
    assert_eq!(load(&path), Tokens::default());
    fs::write(&path, br#"{"tokens":{"not a relay":"0123456789abcdef-x"}}"#).unwrap();
    assert_eq!(load(&path), Tokens::default());
}

#[test]
fn debug_never_shows_a_token() {
    let mut tokens = Tokens::default();
    let endpoint = Endpoint::parse("relay.example.org").unwrap();
    tokens
        .set(&endpoint, Some(token("0123456789abcdef-secret")))
        .unwrap();
    assert!(!format!("{tokens:?}").contains("secret"));
}
