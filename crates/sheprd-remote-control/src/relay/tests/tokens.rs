//! A private relay admits this computer once it has the relay's listen
//! token, its own or one shared with it, without restarting the daemon.

use super::*;
use crate::app::relay_tokens;
use crate::control::relays::{ListenToken, Offer};
use secrecy::SecretString;

const TOKEN: &str = "a listen token of the operator";

async fn private_relay() -> String {
    let (http, _quic, _pin) = relay_with(RelayOptions {
        listen_token: Some(SecretString::from(TOKEN.replace(' ', "-"))),
        metrics: None,
        ..RelayOptions::default()
    })
    .await;
    format!("http://{http}")
}

fn token() -> ListenToken {
    ListenToken::parse(&TOKEN.replace(' ', "-")).unwrap()
}

async fn state_of(relays: &Relays) -> Status {
    for _ in 0..100 {
        if let Some(status) = relays.status().into_values().next()
            && !matches!(status.state, Status::Connecting)
        {
            return status.state;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("still connecting: {:?}", relays.status());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_private_relay_refuses_this_computer_until_its_token_is_set() {
    let endpoint = private_relay().await;
    let dir = tempfile::tempdir().unwrap();
    let mut shared = state(dir.path());
    let configured = Endpoint::parse(&endpoint).unwrap();
    let relays = Arc::new(Relays::load(dir.path(), vec![configured.clone().into()]).unwrap());
    Arc::get_mut(&mut shared).unwrap().relay = Some(Arc::clone(&relays));
    let (_stop, stopped) = watch::channel(false);
    relays.start(served(dir.path(), &shared).0, stopped);
    assert!(matches!(state_of(&relays).await, Status::Failed { .. }));
    assert!(!relays.status()[&endpoint].token);

    // The token reaches the running daemon, which reconnects with it.
    let path = dir.path().to_owned();
    tokio::task::spawn_blocking(move || {
        relay_tokens::set(&path, &configured, Some(token())).unwrap();
    })
    .await
    .unwrap();
    let reload = Arc::clone(&relays);
    tokio::task::spawn_blocking(move || reload.reload_tokens())
        .await
        .unwrap();
    listening(&relays).await;
    assert!(relays.status()[&endpoint].token);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shared_private_relay_is_listened_on_with_the_token_shared_beside_it() {
    let endpoint = private_relay().await;
    let dir = tempfile::tempdir().unwrap();
    let mut shared = state(dir.path());
    let relays = Arc::new(Relays::load(dir.path(), Vec::new()).unwrap());
    Arc::get_mut(&mut shared).unwrap().relay = Some(Arc::clone(&relays));
    let (_stop, stopped) = watch::channel(false);
    relays.start(served(dir.path(), &shared).0, stopped);
    let share = Arc::clone(&relays);
    let relay = endpoint.clone();
    tokio::task::spawn_blocking(move || {
        share.share(Offer {
            source: "studio".into(),
            tokens: [(relay.clone(), token())].into(),
            relays: vec![relay],
        })
    })
    .await
    .unwrap()
    .unwrap();
    listening(&relays).await;
    assert!(relays.status()[&endpoint].token);
    // The token is kept with the shared relay, readable by this user alone.
    let file = super::super::shared::file(dir.path());
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("tokens"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
