use super::*;

#[test]
fn a_claimed_pane_cannot_be_claimed_again_until_released() {
    let claims = Claims::new();
    let first = claims.claim("b1", "p1");
    assert!(first.is_some());
    assert!(claims.claim("b1", "p1").is_none(), "one move per pane");
    // Another pane, or the same pane ID on another daemon, is free.
    assert!(claims.claim("b1", "p2").is_some());
    assert!(claims.claim("b2", "p1").is_some());
    drop(first);
    assert!(!claims.is_claimed("b1", "p1"));
    assert!(claims.claim("b1", "p1").is_some());
}

#[test]
fn only_one_of_many_racing_threads_claims_a_pane() {
    let claims = Claims::new();
    let barrier = std::sync::Barrier::new(8);
    let won: usize = std::thread::scope(|scope| {
        let racers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    // Held until every racer has tried.
                    let claim = claims.claim("b1", "p1");
                    let won = claim.is_some();
                    barrier.wait();
                    usize::from(won)
                })
            })
            .collect();
        racers
            .into_iter()
            .map(|racer| racer.join().unwrap_or_default())
            .sum()
    });
    assert_eq!(won, 1);
    assert!(!claims.is_claimed("b1", "p1"));
}
