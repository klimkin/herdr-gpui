use super::*;

#[test]
fn menu_visibility_is_a_local_command_with_no_default_keybinding() {
    assert!(request(Command::ToggleMenuBar, &snapshot()).is_none());
    let info = COMMANDS.iter().find(|info| info.name == "toggle_menu_bar");
    if cfg!(any(target_os = "linux", windows)) {
        let info = info.unwrap();
        assert_eq!(info.command, Command::ToggleMenuBar);
        assert_eq!(info.message, "commands-toggle-menu-bar");
        assert!(info.base_shortcuts().is_empty());
    } else {
        assert!(info.is_none());
    }
}
