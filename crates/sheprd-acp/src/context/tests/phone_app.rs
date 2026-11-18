use super::*;

const PHONE_WEB: Origin = Origin::new(Device::PhoneWeb, Via::Relay);

#[test]
fn only_the_phone_app_follows_its_template_with_sizes_and_effects() {
    assert_eq!(Kind::after(PHONE), Some(Kind::PhoneApp));
    for device in [
        Device::PhoneWeb,
        Device::Desktop,
        Device::Terminal,
        Device::Other,
    ] {
        assert_eq!(Kind::after(Origin::new(device, Via::Local)), None);
    }
}

#[test]
fn the_phone_app_is_told_about_sizes_and_effects_and_the_web_app_is_not() {
    let app = Context::default().before(Some(PHONE)).unwrap();
    assert!(app.contains("Their screen is small"), "{app}");
    assert!(app.contains(":big["), "{app}");
    assert!(app.contains(":::slam"), "{app}");
    assert!(app.contains("::confetti"), "{app}");
    assert!(app.contains("until tapped"), "{app}");
    // After the phone's own text, so the place reads first.
    assert!(app.find("Their screen is small") < app.find(":big["));

    let web = Context::default().before(Some(PHONE_WEB)).unwrap();
    assert!(web.contains("Their screen is small"), "{web}");
    for directive in [":big[", ":::", "::confetti"] {
        assert!(!web.contains(directive), "{directive}: {web}");
    }
}

#[test]
fn moving_from_the_web_app_to_the_phone_app_tells_about_effects() {
    let mut context = Context::default();
    assert!(!context.before(Some(PHONE_WEB)).unwrap().contains(":big["));
    assert!(context.before(Some(PHONE)).unwrap().contains(":big["));
    assert_eq!(context.before(Some(PHONE)), None);
}
