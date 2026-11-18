//! Why a relay's listen token could not be checked or kept.

/// Only [`super::wants_token`] makes the network's, unused in tests.
#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, allow(dead_code))]
pub enum Error {
    // keep-sorted start
    #[error("the relay's answer is not one a relay gives")]
    Answer(#[source] serde_json::Error),
    #[error("not a relay address")]
    Endpoint(#[from] sheprd_relay::endpoint::Error),
    #[error("the relay did not answer")]
    Info(#[source] ureq::Error),
    #[error("the relay speaks protocol {0}, which this Sheprd does not")]
    Protocol(u8),
    #[error(transparent)]
    Save(#[from] sheprd_remote_control::app::Error),
    // keep-sorted end
}
