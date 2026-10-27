use super::*;

#[test]
fn defaults_use_command_shift_m_with_the_border_on() {
    let preferences = Preferences::default();
    assert_eq!(preferences.shortcut, "CommandOrControl+Shift+M");
    assert!(preferences.border_enabled);
    assert_eq!(preferences.badge_position, BadgePosition::Se);
    assert!(parse_shortcut(&preferences.shortcut).is_ok());
}

#[test]
fn adding_label_positions_preserves_existing_settings() {
    let old = r#"{"shortcut":"Control+Alt+M","border_enabled":false}"#;
    let preferences: Preferences = serde_json::from_str(old).unwrap();
    assert_eq!(preferences.shortcut, "Control+Alt+M");
    assert!(!preferences.border_enabled);
    assert_eq!(preferences.badge_position, BadgePosition::Se);
    for position in ["n", "ne", "e", "se", "s", "sw", "w", "nw"] {
        let mut saved = serde_json::to_value(&preferences).unwrap();
        saved["badge_position"] = position.into();
        let decoded: Preferences = serde_json::from_value(saved).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap()["badge_position"],
            position
        );
    }
    assert!(serde_json::from_str::<BadgePosition>(r#""center""#).is_err());
}

#[test]
fn shortcut_validation_preserves_a_modifier_requirement() {
    assert!(parse_shortcut("M").is_err());
    assert!(parse_shortcut("Shift+M").is_err());
    assert!(parse_shortcut("CommandOrControl+Shift+M").is_ok());
    assert!(parse_shortcut("CommandOrControl+DefinitelyNotAKey").is_err());
}

#[test]
fn partial_preferences_use_defaults_and_ignore_future_fields() {
    let preferences: Preferences =
        serde_json::from_str(r#"{"border_enabled":false,"future_setting":true}"#).unwrap();
    assert_eq!(preferences.shortcut, Preferences::default().shortcut);
    assert!(!preferences.border_enabled);
    assert_eq!(preferences.badge_position, BadgePosition::Se);
    assert!(serde_json::from_str::<Preferences>(r#"{"border_enabled":"yes"}"#).is_err());
    assert!(serde_json::from_str::<Preferences>("not json").is_err());
}

#[test]
fn preferences_round_trip_all_values() {
    let original = Preferences {
        shortcut: "Super+Alt+K".into(),
        border_enabled: false,
        badge_position: BadgePosition::Nw,
    };
    let json = serde_json::to_vec_pretty(&original).unwrap();
    let restored: Preferences = serde_json::from_slice(&json).unwrap();
    assert_eq!(restored.shortcut, original.shortcut);
    assert_eq!(restored.border_enabled, original.border_enabled);
    assert_eq!(restored.badge_position, original.badge_position);
}

#[test]
fn shortcut_validation_rejects_empty_and_oversized_values() {
    assert!(parse_shortcut("").is_err());
    assert!(parse_shortcut(&format!("Super+{}", "M".repeat(101))).is_err());
    for value in [
        "Super+A",
        "Control+5",
        "Alt+Up",
        "Super+F5",
        "Super+Control+Alt+Shift+A",
    ] {
        assert!(parse_shortcut(value).is_ok(), "{value}");
    }
}

#[test]
fn bundled_tray_icons_decode_and_only_live_uses_a_template() {
    for muted in [None, Some(false), Some(true)] {
        let (image, template) = tray_image(muted);
        assert!(image.width() > 0);
        assert!(image.height() > 0);
        assert_eq!(template, muted == Some(false));
    }
}

fn indicator_status() -> Status {
    Status {
        microphone: Some(audio::Microphone {
            id: 1,
            name: "Test input".into(),
            muted: true,
        }),
        error: None,
        shortcut: Preferences::default().shortcut,
        shortcut_error: None,
        border_enabled: true,
        badge_position: BadgePosition::Se,
        preview: false,
    }
}

#[test]
fn a_previous_operation_error_does_not_hide_confirmed_mute() {
    let mut status = indicator_status();
    status.error = Some("A previous mute operation failed".into());
    assert!(status.overlay_visible());
    status.microphone.as_mut().unwrap().muted = false;
    assert!(!status.overlay_visible());
}

#[test]
fn preview_and_border_preference_respect_unavailable_inputs() {
    let mut status = indicator_status();
    status.border_enabled = false;
    assert!(!status.overlay_visible());
    status.border_enabled = true;
    status.microphone = None;
    status.error = Some("Input disconnected".into());
    assert!(!status.overlay_visible());
    status.preview = true;
    assert!(status.overlay_visible());
}
