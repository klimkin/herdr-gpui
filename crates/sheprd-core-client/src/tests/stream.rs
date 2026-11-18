use crate::stream::{End, Link, Route, Stream};
use std::{sync::Arc, time::Duration};

/// A stream whose connection worker already ended every route but this
/// one's bytes, then let go of the outbound queue.
fn orphaned(bytes: &[u8]) -> Stream {
    let (outbound, queue) = crossbeam_channel::bounded(1);
    drop(queue);
    let route = Arc::new(Route::new());
    route.push(bytes).unwrap();
    Stream {
        channel: 5,
        link: Link { outbound },
        route,
    }
}

#[test]
fn bytes_read_as_the_connection_drops_are_kept() {
    let stream = orphaned(b"held");
    // Their credit cannot be sent, but the bytes arrived before the loss.
    let chunk = stream.read(Duration::ZERO).unwrap();
    assert_eq!(chunk.bytes, b"held");
    stream.route.end(End::Lost);
    let chunk = stream.read(Duration::ZERO).unwrap();
    assert!(chunk.bytes.is_empty());
    assert!(matches!(chunk.end, Some(End::Lost)));
}
