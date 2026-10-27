use super::*;
use control::{ConfigCommand, ControlCommand, Invocation};

#[test]
fn compact_menu_commands_preserve_global_cli_options() {
    for (args, command) in [
        (
            vec![
                "config",
                "set",
                "show_menu_bar",
                "false",
                "--json",
                "--window",
                "7",
            ],
            ControlCommand::Config(ConfigCommand::Set {
                key: "show_menu_bar".into(),
                value: "false".into(),
            }),
        ),
        (
            vec!["run", "toggle_menu_bar", "--json", "--window", "7"],
            ControlCommand::Run {
                name: "toggle_menu_bar".into(),
            },
        ),
    ] {
        assert_eq!(
            LaunchOptions::parse(args).unwrap().mode,
            LaunchMode::Control(Invocation {
                command,
                json: true,
                window: Some(7),
            })
        );
    }
}
