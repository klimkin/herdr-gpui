use super::*;

#[test]
fn typing_before_the_move_acts_stops_it() {
    let stage = Stage::default();
    assert!(stage.typed());
    assert!(!stage.commit());
    assert!(!stage.commit());
    assert!(stage.stopped_by_typing());
}

#[test]
fn typing_after_the_move_acted_lets_it_finish() {
    let stage = Stage::default();
    assert!(stage.commit());
    assert!(!stage.typed(), "the agent is already stopping");
    assert!(stage.commit(), "later rounds still press their keys");
    assert!(!stage.stopped_by_typing());
}

#[test]
fn typing_racing_the_move_has_exactly_one_winner() {
    for _ in 0..200 {
        let stage = std::sync::Arc::new(Stage::default());
        let typist = {
            let stage = stage.clone();
            std::thread::spawn(move || stage.typed())
        };
        let pressed = stage.commit();
        let typed = typist.join().unwrap();
        assert_ne!(pressed, typed);
    }
}
