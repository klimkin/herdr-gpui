//! A private relay's listen token, Sheprd's side: asking a relay whether
//! it wants one (`GET /v1/info`), and keeping one for this computer's
//! daemon (`sheprd_remote_control::app::relay_tokens`). Everything here
//! blocks: run it off the UI thread.

mod error;

pub(crate) use error::Error;
pub(crate) use sheprd_remote_control::control::relays::ListenToken;

use sheprd_relay::{Endpoint, endpoint::Info};
use sheprd_remote_control::app::relay_tokens;
use std::time::Duration;

// Settings' tests put their own check in, never reaching a relay, so in
// a test build the network side goes unused.
/// How long a relay may take to say what it wants.
#[cfg_attr(test, allow(dead_code))]
const TIMEOUT: Duration = Duration::from_secs(10);
/// `/v1/info` is a few dozen bytes.
#[cfg_attr(test, allow(dead_code))]
const MAX_INFO: u64 = 4096;

/// Whether the relay at `relay` admits only computers with its listen
/// token. A relay speaking another protocol is refused.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn wants_token(relay: &str) -> Result<bool, Error> {
    let endpoint = Endpoint::parse(relay)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .max_redirects(0)
        .build()
        .into();
    let bytes = agent
        .get(endpoint.info().as_str())
        .call()
        .map_err(Error::Info)?
        .body_mut()
        .with_config()
        .limit(MAX_INFO)
        .read_to_vec()
        .map_err(Error::Info)?;
    let info: Info = serde_json::from_slice(&bytes).map_err(Error::Answer)?;
    if info.protocol != sheprd_relay::endpoint::PROTOCOL {
        return Err(Error::Protocol(info.protocol));
    }
    Ok(info.listen_token)
}

/// Keeps `token` for the relay at `relay` on this computer's daemon, or
/// with `None` forgets it; a running daemon listens with it at once.
pub(crate) fn save(relay: &str, token: Option<ListenToken>) -> Result<(), Error> {
    let endpoint = Endpoint::parse(relay)?;
    relay_tokens::set(&super::state_dir()?, &endpoint, token)?;
    Ok(())
}

/// This computer's tokens for the relays in `relays`, keyed as `relays`
/// spells them, to share with the user's other computers.
pub(crate) fn shared(relays: &[String]) -> std::collections::BTreeMap<String, ListenToken> {
    let Ok(dir) = super::state_dir() else {
        return Default::default();
    };
    let tokens = relay_tokens::load(&relay_tokens::file(&dir));
    relays
        .iter()
        .filter_map(|relay| Some((relay.clone(), tokens.for_relay(relay)?.clone())))
        .collect()
}
