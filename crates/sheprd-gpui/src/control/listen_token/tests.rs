#![allow(clippy::unwrap_used)]
use super::*;

const TOKEN: &str = "0123456789abcdef-secret";

#[test]
fn a_piped_token_is_read_trimmed() {
    let token = read(format!("  {TOKEN}\n").as_bytes()).unwrap();
    assert_eq!(token.expose(), TOKEN);
    assert!(!format!("{token:?}").contains(TOKEN));
}

#[test]
fn empty_input_is_refused() {
    for input in ["", "\n", " \t\r\n"] {
        assert!(
            matches!(read(input.as_bytes()), Err(Error::Empty)),
            "{input:?}"
        );
    }
}

#[test]
fn input_is_bounded() {
    let longest = format!("{TOKEN}{}", " ".repeat(MAX_INPUT - TOKEN.len()));
    assert_eq!(read(longest.as_bytes()).unwrap().expose(), TOKEN);
    let over = format!("{longest} ");
    assert!(matches!(read(over.as_bytes()), Err(Error::TooLong)));
    // An endless source is never read past the bound.
    assert!(matches!(read(std::io::repeat(b'a')), Err(Error::TooLong)));
}

#[test]
fn what_a_relay_would_refuse_is_refused_without_showing_it() {
    for input in ["too short", "has a space in the middle", &"a".repeat(256)] {
        let error = read(input.as_bytes()).unwrap_err();
        assert!(matches!(error, Error::Token(_)), "{input:?}");
        assert!(!error.to_string().contains(input), "{error}");
    }
    assert!(matches!(read(&[0xff, 0xfe][..]), Err(Error::Text)));
}

#[test]
fn a_failed_read_keeps_its_cause() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
    }
    let error = read(Broken).unwrap_err();
    assert!(matches!(error, Error::Read(_)));
    assert_eq!(
        std::error::Error::source(&error).unwrap().to_string(),
        "closed"
    );
}
