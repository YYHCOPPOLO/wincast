use crate::app_settings_key::AppSettingsKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bucket {
    Mirrored,
    External,
    Excluded,
}

pub fn coverage_bucket(key: AppSettingsKey) -> Bucket {
    if MIRRORED.contains(&key) {
        Bucket::Mirrored
    } else if EXCLUDED.iter().any(|(k, _)| *k == key) {
        Bucket::Excluded
    } else {
        Bucket::External
    }
}

pub fn excluded_reason(key: AppSettingsKey) -> Option<&'static str> {
    EXCLUDED.iter().find(|(k, _)| *k == key).map(|(_, r)| *r)
}

pub fn is_excluded_raw(raw: &str) -> bool {
    AppSettingsKey::ALL
        .iter()
        .any(|k| k.as_str() == raw && coverage_bucket(*k) == Bucket::Excluded)
}

/// Drop excluded keys so an import cannot grant capabilities.
pub fn filter_import(value: &serde_json::Value) -> serde_json::Value {
    let Some(obj) = value.as_object() else {
        return value.clone();
    };
    let mut out = serde_json::Map::new();
    for (k, v) in obj {
        if is_excluded_raw(k) {
            continue;
        }
        out.insert(k.clone(), v.clone());
    }
    serde_json::Value::Object(out)
}

const MIRRORED: &[AppSettingsKey] = &[
    AppSettingsKey::ClipboardRetention,
    AppSettingsKey::ClipboardDisabledApps,
    AppSettingsKey::HyperKey,
    AppSettingsKey::HyperKeyIncludesShift,
    AppSettingsKey::HyperKeyQuickPress,
    AppSettingsKey::EmojiSkinTone,
    AppSettingsKey::PopToRootTimeout,
    AppSettingsKey::Appearance,
    AppSettingsKey::CompactMode,
    AppSettingsKey::ShowFavoritesInCompactMode,
    AppSettingsKey::SearchScopes,
    AppSettingsKey::OpenOnCursorScreen,
    AppSettingsKey::PaletteDraggable,
    AppSettingsKey::FileSearchEnabled,
    AppSettingsKey::FileSearchScopes,
    AppSettingsKey::FileSearchIgnorePatterns,
    AppSettingsKey::NotesEnabled,
    AppSettingsKey::CustomCommandsEnabled,
    AppSettingsKey::CustomCommandsShowInLauncher,
    AppSettingsKey::SnippetsShowInLauncher,
    AppSettingsKey::WindowManagementEnabled,
    AppSettingsKey::WindowManagementShowInLauncher,
    AppSettingsKey::WindowGap,
    AppSettingsKey::WindowCycleOnRepeat,
    AppSettingsKey::QuicklinksEnabled,
    AppSettingsKey::QuicklinksShowInLauncher,
    AppSettingsKey::QuicklinkOpensNewWindow,
    AppSettingsKey::QuicklinkSelectionFallback,
    AppSettingsKey::QuicklinkConfirmsBeforeDelete,
    AppSettingsKey::ExtensionsShowInLauncher,
    AppSettingsKey::CalendarShowInLauncher,
    AppSettingsKey::JoinWindowMinutes,
    AppSettingsKey::AutoJoinConfirms,
    AppSettingsKey::MenuBarEvents,
    AppSettingsKey::MenuBarLinkedEventsOnly,
    AppSettingsKey::HideCurrentEvent,
    AppSettingsKey::SupportReminders,
    AppSettingsKey::UiLanguage,
];

const EXCLUDED: &[(AppSettingsKey, &'static str)] = &[
    (
        AppSettingsKey::SnippetsEnabled,
        "Doubles as keyword-expansion consent; an import must not enable keystroke listening.",
    ),
    (
        AppSettingsKey::ExtensionPackageManager,
        "Names a tool on this Mac; the machine a backup lands on may not have it.",
    ),
    (
        AppSettingsKey::ExtensionRegistries,
        "A registry is a source of executable code; adding one has to be a deliberate act.",
    ),
    (
        AppSettingsKey::ExtensionCustomSearchPaths,
        "Machine-local toolchain paths; the Mac a backup lands on may not have them.",
    ),
    (
        AppSettingsKey::ExtensionsEnabled,
        "Doubles as consent to run third-party JavaScript; an import must not switch it on.",
    ),
    (
        AppSettingsKey::PalettePosition,
        "Machine-local geometry: a point restored onto another display layout lands nowhere.",
    ),
    (
        AppSettingsKey::AutoSwitchInputSource,
        "Names a keyboard input source installed on this Mac; another Mac may not have it.",
    ),
    (
        AppSettingsKey::CalendarEnabled,
        "Doubles as consent to read your calendar; an import must not grant calendar access.",
    ),
    (
        AppSettingsKey::AutoJoinMeetings,
        "Arms the app to open meeting links unattended; an import must not switch that on.",
    ),
    (
        AppSettingsKey::CameraPreview,
        "Turns the camera on before a meeting; an import must not grant that.",
    ),
    (
        AppSettingsKey::AiEnabled,
        "No other AI setting travels in a backup, so an import would arm a feature it cannot configure.",
    ),
    (
        AppSettingsKey::AiConnections,
        "AI connection metadata stays on the Mac with the Keychain credentials it describes.",
    ),
    (
        AppSettingsKey::AiDefaultModel,
        "The default model names an external AI destination; importing must not choose one.",
    ),
    (
        AppSettingsKey::AiWebSearch,
        "Whether prompts may reach a search engine is a choice each Mac makes for itself.",
    ),
    (
        AppSettingsKey::AiSystemPrompt,
        "Standing instructions to a model are the one AI setting that changes every answer.",
    ),
    (
        AppSettingsKey::AiSystemPromptEnabled,
        "Governs whether a turn carries standing instructions at all.",
    ),
    (
        AppSettingsKey::AiRetention,
        "How long conversations survive is a decision about the chats on this Mac.",
    ),
    (
        AppSettingsKey::AiOpensTo,
        "Whether chat reopens on an existing conversation depends on the history this Mac holds.",
    ),
    (
        AppSettingsKey::AiNewChatAfter,
        "Paces the same decision as the setting it accompanies.",
    ),
    (
        AppSettingsKey::QuickActionsEnabled,
        "Grants keystroke delivery into other apps; a flag that grants a capability is never carried.",
    ),
    (
        AppSettingsKey::QuickActionModel,
        "Names an external AI destination; an import must not choose one.",
    ),
    (
        AppSettingsKey::QuickActionPreviews,
        "Says which actions may rewrite a document without showing the result first.",
    ),
    (
        AppSettingsKey::QuickActionLanguage,
        "Follows the language the person at this Mac reads, not the one who wrote the backup.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_settings_key::AppSettingsKey;
    use std::collections::HashSet;

    #[test]
    fn every_key_is_classified_once() {
        let mut seen = HashSet::new();
        for k in AppSettingsKey::ALL {
            let n = coverage_bucket(*k);
            assert!(seen.insert(*k), "duplicate {k:?}");
            assert!(n == Bucket::Mirrored || n == Bucket::External || n == Bucket::Excluded);
        }
        assert_eq!(seen.len(), AppSettingsKey::ALL.len());
        assert!(coverage_bucket(AppSettingsKey::SnippetsEnabled) == Bucket::Excluded);
        assert!(coverage_bucket(AppSettingsKey::AiEnabled) == Bucket::Excluded);
        assert_eq!(
            AppSettingsKey::ALL
                .iter()
                .filter(|k| coverage_bucket(**k) == Bucket::External)
                .count(),
            0,
            "AppSettingsKey has no externally-sourced members; launchAtLogin/showInMenuBar are not keys"
        );
        assert!(excluded_reason(AppSettingsKey::SnippetsEnabled)
            .unwrap()
            .contains("keystroke"));
        assert_eq!(
            coverage_bucket(AppSettingsKey::UiLanguage),
            Bucket::Mirrored
        );
    }

    #[test]
    fn ai_settings_do_not_serialize_into_backup() {
        assert_eq!(coverage_bucket(AppSettingsKey::AiEnabled), Bucket::Excluded);
        assert_eq!(
            coverage_bucket(AppSettingsKey::AiConnections),
            Bucket::Excluded
        );
    }

    #[test]
    fn import_cannot_enable_snippets() {
        let filtered = filter_import(&serde_json::json!({
            "snippetsEnabled": true,
            "compactMode": false,
            "aiEnabled": true,
            "calendarEnabled": true
        }));
        assert!(filtered.get("snippetsEnabled").is_none());
        assert!(filtered.get("aiEnabled").is_none());
        assert!(filtered.get("calendarEnabled").is_none());
        assert_eq!(filtered["compactMode"], false);
    }
}
