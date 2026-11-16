use super::*;
use crate::config::remote::Relay;
use anyhow::Context;
use sheprd_remote_control::relay_label::{self, RelayLabel};

#[test]
fn saving_relay_edits_preserves_labels_and_plain_addresses() -> anyhow::Result<()> {
    use crate::config::{preferences::Preference, remote::Edit};

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("local.toml");
    fs::write(&path, "# mine\n")?;
    let labelled = Relay {
        endpoint: " https://fra3.example.org/ ".into(),
        label: RelayLabel {
            description: Some("not for public use".into()),
            name: Some("fra3".into()),
            tags: vec!["eu".into(), "private".into()],
        },
    };
    Config::save_preference_path(
        Preference::Remote(Edit::Relays(vec![
            "https://sfo3.example.org".into(),
            labelled.clone(),
            "https://fra3.example.org".into(),
        ])),
        &path,
    )?;
    let text = fs::read_to_string(&path)?;
    assert!(text.contains("# mine"), "{text}");
    let document: toml_edit::DocumentMut = text.parse()?;
    let relays = document["remote"]["relays"]
        .as_array()
        .context("saved relay list must be an array")?;
    assert_eq!(
        relays.get(0).and_then(|it| it.as_str()),
        Some("https://sfo3.example.org")
    );
    assert!(relays.get(1).is_some_and(|relay| relay.is_inline_table()));
    assert_eq!(
        Config::parse(&text)?.remote.relays,
        [
            "https://sfo3.example.org".into(),
            Relay {
                endpoint: "https://fra3.example.org".into(),
                ..labelled
            },
        ]
    );
    Ok(())
}

#[test]
fn saving_an_invalid_relay_label_leaves_the_file_alone() -> anyhow::Result<()> {
    use crate::config::{preferences::Preference, remote::Edit};

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("local.toml");
    let original = "[remote]\nrelays = ['https://sfo3.example.org']\n";
    fs::write(&path, original)?;
    let error = Config::save_preference_path(
        Preference::Remote(Edit::Relays(vec![Relay {
            endpoint: "https://fra3.example.org".into(),
            label: RelayLabel {
                name: Some(String::new()),
                ..RelayLabel::default()
            },
        }])),
        &path,
    )
    .err()
    .context("invalid relay label must be rejected")?;
    let crate::Error::Path {
        path: failed,
        source,
    } = error
    else {
        anyhow::bail!("relay validation error must retain its path");
    };
    assert_eq!(failed, path);
    assert!(matches!(
        *source,
        crate::Error::RemoteControl(crate::remote::Error::RelayLabel {
            source: relay_label::Error::Blank { field: "name" },
            ..
        })
    ));
    assert_eq!(fs::read_to_string(&path)?, original);
    Ok(())
}
