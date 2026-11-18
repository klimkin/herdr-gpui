//! Why no listen token could be read from standard input.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("No listen token on standard input; pipe it in, or pass --clear to forget it")]
    Empty,
    #[error("Could not read the listen token from standard input")]
    Read(#[source] std::io::Error),
    #[error("Standard input is a terminal; pipe the listen token in so it is not shown")]
    Terminal,
    #[error("The listen token on standard input is not text")]
    Text,
    #[error("That is not a listen token: {0}")]
    Token(#[from] sheprd_remote_control::control::relays::Error),
    #[error(
        "Standard input holds more than {} bytes; a listen token is at most 255",
        super::MAX_INPUT
    )]
    TooLong,
    // keep-sorted end
}
