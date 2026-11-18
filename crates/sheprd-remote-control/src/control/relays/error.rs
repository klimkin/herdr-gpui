/// Why an offer of relays was refused.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("{relay} is not a relay address")]
    Endpoint {
        relay: String,
        #[source]
        source: sheprd_relay::endpoint::Error,
    },
    #[error("the source name is empty or too long")]
    Source,
    #[error("a relay's listen token is 16 to 255 characters, without spaces")]
    Token,
    #[error("too many computers share relays with this one")]
    TooManySources,
    // keep-sorted end
}
