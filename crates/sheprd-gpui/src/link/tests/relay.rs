//! `sheprd://relay`: a private relay and its listen token, from the deploy
//! page, read strictly since any web page can send one.

use super::*;

const TOKEN: &str = "Zm9vYmFyYmF6cXV4LXRva2VuLW9mLTMyLWJ5dGVz";

fn relay_link(endpoint: &str, token: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("endpoint", endpoint)
        .append_pair("token", token)
        .finish();
    format!("sheprd://relay?{query}")
}

#[test]
fn a_relay_link_names_the_relay_as_the_config_keeps_it_and_its_token() {
    let link = Link::parse(&relay_link("https://RELAY.example.org/", TOKEN)).unwrap();
    let Link::Relay { relay, token } = &link else {
        panic!("{link:?}");
    };
    assert_eq!(relay, "https://relay.example.org");
    assert_eq!(token.expose(), TOKEN);
    assert_eq!(Link::parse(&link.to_url()).unwrap(), link);
    // The token never shows in what is logged.
    assert!(!format!("{link:?}").contains(TOKEN));
}

#[test]
fn a_relay_link_without_a_usable_relay_or_token_is_refused() {
    for (url, refusal) in [
        (
            relay_link("https://relay.example.org", "short"),
            Error::Token,
        ),
        (
            relay_link("https://relay.example.org", "has spaces in the token"),
            Error::Token,
        ),
        (relay_link("http://relay.example.org", TOKEN), Error::Relay),
        (relay_link("ftp://relay.example.org", TOKEN), Error::Relay),
        (
            "sheprd://relay?endpoint=relay.example.org".to_owned(),
            Error::MissingParameter("token"),
        ),
        (
            format!("{}&path=/tmp", relay_link("relay.example.org", TOKEN)),
            Error::UnknownParameter("path".into()),
        ),
        (
            format!("{}&token={TOKEN}", relay_link("relay.example.org", TOKEN)),
            Error::DuplicateParameter("token"),
        ),
    ] {
        assert_eq!(Link::parse(&url), Err(refusal), "{url}");
    }
}
