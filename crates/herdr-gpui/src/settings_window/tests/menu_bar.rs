use super::*;
use crate::config::preferences::Preference;
use gpui::{px, size};

#[gpui::test]
fn menu_bar_setting_is_searchable_and_saves_a_boolean(cx: &mut TestAppContext) {
    assert!(
        catalog::search("menu bar")
            .into_iter()
            .any(|index| catalog::CATALOG[index].id == "general-menu-bar")
    );
    let (view, cx) = cx.add_window_view(skill_fixture);
    let edits = Arc::new(Mutex::new(Vec::new()));
    view.update(cx, |view, cx| {
        view.section = Section::General;
        let captured = edits.clone();
        view.controls.preference_io = Some(controls::preferences::PreferenceIo::recording(
            move |edit| captured.lock().unwrap().push(edit),
        ));
        cx.notify();
    });
    cx.simulate_resize(size(px(960.), px(2000.)));
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    let button = cx.debug_bounds("general-menu-bar-toggle");
    if crate::menus::IN_WINDOW {
        cx.simulate_click(button.unwrap().center(), Default::default());
        cx.run_until_parked();
        assert_eq!(
            edits.lock().unwrap().last(),
            Some(&Preference::ShowMenuBar(false))
        );
    } else {
        assert!(button.is_none());
    }
}
