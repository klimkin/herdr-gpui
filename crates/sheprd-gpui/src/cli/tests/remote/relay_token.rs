use super::*;

fn relay(text: &str) -> sheprd_relay::Endpoint {
    sheprd_relay::Endpoint::parse(text).unwrap()
}

#[test]
fn a_relay_token_is_kept_or_cleared_for_a_relay_address() {
    assert_eq!(
        command(&["remote", "relays", "token", "relay.example.org"]),
        ControlCommand::Remote(RemoteCommand::RelayToken {
            relay: relay("https://relay.example.org"),
            clear: false,
        })
    );
    let clear = parse(&[
        "remote",
        "relays",
        "token",
        "--clear",
        "https://relay.example.org",
        "--json",
        "--window",
        "2",
    ])
    .unwrap();
    assert_eq!(
        clear.command,
        ControlCommand::Remote(RemoteCommand::RelayToken {
            relay: relay("https://relay.example.org"),
            clear: true,
        })
    );
    assert!(clear.json);
    assert_eq!(clear.window, Some(2));
}

#[test]
fn a_relay_token_needs_a_relay_address_and_never_takes_the_token() {
    assert!(matches!(
        parse(&["remote", "relays", "token"]),
        Err(CliError::MissingArgument { .. })
    ));
    assert!(matches!(
        parse(&["remote", "relays"]),
        Err(CliError::MissingArgument { .. })
    ));
    assert!(matches!(
        parse(&["remote", "relays", "list"]),
        Err(CliError::UnknownCommand { .. })
    ));
    for bad in ["ftp://relay.example.org", "https://"] {
        assert!(
            matches!(
                parse(&["remote", "relays", "token", bad]),
                Err(CliError::InvalidValue("relay address"))
            ),
            "{bad}"
        );
    }
    let secret = "0123456789abcdef-secret";
    let error = parse(&["remote", "relays", "token", "relay.example.org", secret]).unwrap_err();
    assert!(matches!(error, CliError::InvalidValue(_)));
    assert!(!error.to_string().contains(secret), "{error}");
    assert!(
        parse(&[
            "remote",
            "relays",
            "token",
            "relay.example.org",
            "--token",
            secret
        ])
        .is_err()
    );
}
