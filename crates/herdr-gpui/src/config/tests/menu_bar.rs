use super::*;
use crate::config::preferences::Preference;

#[test]
fn menu_bar_defaults_visible_and_rejects_non_booleans() -> anyhow::Result<()> {
    assert!(Config::default().show_menu_bar);
    assert!(Config::parse("")?.show_menu_bar);
    assert!(Config::parse(DEFAULT_CONFIG)?.show_menu_bar);
    assert!(!Config::parse("show_menu_bar = false")?.show_menu_bar);
    for value in ["0", "'false'", "[]"] {
        assert!(Config::parse(&format!("show_menu_bar = {value}")).is_err());
    }
    Ok(())
}

#[test]
fn menu_visibility_persists_and_cli_edits_validate_it() -> anyhow::Result<()> {
    let temp = TempDirectory::new()?;
    let path = temp.0.join("config-gpui.toml");
    let local = path.with_extension("local.toml");
    let daemon = temp.0.join("absent.toml");
    fs::write(&local, "# Personal\ntheme = 'Nord' # Keep\n")?;
    for show in [false, true] {
        Config::save_preference_path(Preference::ShowMenuBar(show), &local)?;
        let text = fs::read_to_string(&local)?;
        assert!(text.contains("# Personal") && text.contains("# Keep"));
        assert_eq!(Config::parse(&text)?.show_menu_bar, show);
    }
    assert!(Config::edit_key_at(
        &local,
        &daemon,
        "show_menu_bar",
        Some(false.into())
    )?);
    assert_eq!(
        Config::effective_value_at(&path, "show_menu_bar")?,
        Some((toml::Value::Boolean(false), Layer::Local))
    );
    let before = fs::read_to_string(&local)?;
    assert!(Config::edit_key_at(&local, &daemon, "show_menu_bar", Some("false".into())).is_err());
    assert_eq!(fs::read_to_string(&local)?, before);
    Config::edit_key_at(&local, &daemon, "show_menu_bar", None)?;
    assert_eq!(
        Config::effective_value_at(&path, "show_menu_bar")?,
        Some((toml::Value::Boolean(true), Layer::Default))
    );
    Ok(())
}
