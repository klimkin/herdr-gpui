use super::*;

const TOKEN: &str = "0123456789abcdef-secret";

fn token() -> ListenToken {
    ListenToken::parse(TOKEN).unwrap()
}

#[test]
fn a_listen_token_request_round_trips() {
    for op in [
        RemoteOp::RelayToken {
            relay: "https://relay.example.org".into(),
            token: Some(token()),
        },
        RemoteOp::RelayToken {
            relay: "https://relay.example.org".into(),
            token: None,
        },
    ] {
        let request = Request::Remote(Call {
            caller: Caller::default(),
            op,
        });
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<Request>(&text).unwrap(), request);
    }
    let clear = serde_json::to_string(&RemoteOp::RelayToken {
        relay: "https://relay.example.org".into(),
        token: None,
    })
    .unwrap();
    assert_eq!(
        clear,
        r#"{"op":"relay_token","relay":"https://relay.example.org"}"#
    );
    assert!(
        RemoteOp::RelayToken {
            relay: String::new(),
            token: None
        }
        .wait_seconds()
            > sheprd_remote_control::control::RESPONSE_TIMEOUT.as_secs()
    );
}

#[test]
fn a_listen_token_request_never_shows_its_token() {
    let request = Request::Remote(Call {
        caller: Caller::default(),
        op: RemoteOp::RelayToken {
            relay: "https://relay.example.org".into(),
            token: Some(token()),
        },
    });
    let debug = format!("{request:?}");
    assert!(!debug.contains(TOKEN), "{debug}");
    assert!(debug.contains("relay.example.org"), "{debug}");
}

#[test]
fn a_token_the_relay_would_refuse_is_refused_on_the_wire() {
    for token in [r#""short""#, r#""has a space in the middle""#, "17"] {
        let text = format!(
            r#"{{"method":"remote","caller":{{}},"op":{{"op":"relay_token","relay":"r","token":{token}}}}}"#
        );
        assert!(serde_json::from_str::<Request>(&text).is_err(), "{text}");
    }
}

#[test]
fn a_listen_token_reply_round_trips_without_the_token() {
    let response = Response::Remote {
        reply: Reply::RelayToken {
            relay: "https://relay.example.org".into(),
            token: true,
        },
    };
    let text = serde_json::to_string(&response).unwrap();
    assert!(text.contains(r#""kind":"relay_token""#), "{text}");
    assert_eq!(serde_json::from_str::<Response>(&text).unwrap(), response);
}
