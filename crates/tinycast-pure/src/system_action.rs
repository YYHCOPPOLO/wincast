use crate::app_entry::{AppEntry, AppKind};
use crate::search_relevance::SearchFields;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SystemActionId {
    LockScreen,
    Sleep,
    SleepDisplays,
    Restart,
    ShutDown,
    LogOut,
    ShowScreenSaver,
    PlayPause,
    NextTrack,
    PreviousTrack,
    ToggleMute,
    VolumeUp,
    VolumeDown,
    SetVolume,
    Volume0,
    Volume25,
    Volume50,
    Volume75,
    Volume100,
    ShowDesktop,
    ToggleAppearance,
    ToggleStageManager,
    OpenTrash,
    EmptyTrash,
    EjectAllDisks,
    ToggleHiddenFiles,
    HideOtherApps,
    UnhideAllApps,
    QuitAllApps,
    DismissNotifications,
    ToggleBluetooth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Confirmation {
    None,
    Required {
        title: &'static str,
        message: &'static str,
    },
    Computed,
}

const SESSION_ENDING_MESSAGE: &str = "Applications with unsaved changes may ask you to save.";

impl SystemActionId {
    pub const ALL: &'static [SystemActionId] = &[
        SystemActionId::LockScreen,
        SystemActionId::Sleep,
        SystemActionId::SleepDisplays,
        SystemActionId::Restart,
        SystemActionId::ShutDown,
        SystemActionId::LogOut,
        SystemActionId::ShowScreenSaver,
        SystemActionId::PlayPause,
        SystemActionId::NextTrack,
        SystemActionId::PreviousTrack,
        SystemActionId::ToggleMute,
        SystemActionId::VolumeUp,
        SystemActionId::VolumeDown,
        SystemActionId::SetVolume,
        SystemActionId::Volume0,
        SystemActionId::Volume25,
        SystemActionId::Volume50,
        SystemActionId::Volume75,
        SystemActionId::Volume100,
        SystemActionId::ShowDesktop,
        SystemActionId::ToggleAppearance,
        SystemActionId::ToggleStageManager,
        SystemActionId::OpenTrash,
        SystemActionId::EmptyTrash,
        SystemActionId::EjectAllDisks,
        SystemActionId::ToggleHiddenFiles,
        SystemActionId::HideOtherApps,
        SystemActionId::UnhideAllApps,
        SystemActionId::QuitAllApps,
        SystemActionId::DismissNotifications,
        SystemActionId::ToggleBluetooth,
    ];

    pub fn all() -> &'static [Self] {
        Self::ALL
    }

    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.raw() == raw)
    }

    pub fn from_entry_id(entry_id: &str) -> Option<Self> {
        entry_id
            .strip_prefix("system-action:")
            .and_then(Self::from_raw)
    }

    pub fn raw(self) -> &'static str {
        match self {
            SystemActionId::LockScreen => "lock-screen",
            SystemActionId::Sleep => "sleep",
            SystemActionId::SleepDisplays => "sleep-displays",
            SystemActionId::Restart => "restart",
            SystemActionId::ShutDown => "shut-down",
            SystemActionId::LogOut => "log-out",
            SystemActionId::ShowScreenSaver => "show-screen-saver",
            SystemActionId::PlayPause => "play-pause",
            SystemActionId::NextTrack => "next-track",
            SystemActionId::PreviousTrack => "previous-track",
            SystemActionId::ToggleMute => "toggle-mute",
            SystemActionId::VolumeUp => "volume-up",
            SystemActionId::VolumeDown => "volume-down",
            SystemActionId::SetVolume => "set-volume",
            SystemActionId::Volume0 => "volume-0",
            SystemActionId::Volume25 => "volume-25",
            SystemActionId::Volume50 => "volume-50",
            SystemActionId::Volume75 => "volume-75",
            SystemActionId::Volume100 => "volume-100",
            SystemActionId::ShowDesktop => "show-desktop",
            SystemActionId::ToggleAppearance => "toggle-system-appearance",
            SystemActionId::ToggleStageManager => "toggle-stage-manager",
            SystemActionId::OpenTrash => "open-trash",
            SystemActionId::EmptyTrash => "empty-trash",
            SystemActionId::EjectAllDisks => "eject-all-disks",
            SystemActionId::ToggleHiddenFiles => "toggle-hidden-files",
            SystemActionId::HideOtherApps => "hide-all-apps-except-frontmost",
            SystemActionId::UnhideAllApps => "unhide-all-hidden-apps",
            SystemActionId::QuitAllApps => "quit-all-apps",
            SystemActionId::DismissNotifications => "dismiss-notifications",
            SystemActionId::ToggleBluetooth => "toggle-bluetooth",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SystemActionId::LockScreen => "Lock Screen",
            SystemActionId::Sleep => "Sleep",
            SystemActionId::SleepDisplays => "Sleep Displays",
            SystemActionId::Restart => "Restart",
            SystemActionId::ShutDown => "Shut Down",
            SystemActionId::LogOut => "Log Out",
            SystemActionId::ShowScreenSaver => "Show Screen Saver",
            SystemActionId::PlayPause => "Play / Pause",
            SystemActionId::NextTrack => "Next Track",
            SystemActionId::PreviousTrack => "Previous Track",
            SystemActionId::ToggleMute => "Toggle Mute",
            SystemActionId::VolumeUp => "Turn Volume Up",
            SystemActionId::VolumeDown => "Turn Volume Down",
            SystemActionId::SetVolume => "Set Volume…",
            SystemActionId::Volume0 => "Set Volume to 0%",
            SystemActionId::Volume25 => "Set Volume to 25%",
            SystemActionId::Volume50 => "Set Volume to 50%",
            SystemActionId::Volume75 => "Set Volume to 75%",
            SystemActionId::Volume100 => "Set Volume to 100%",
            SystemActionId::ShowDesktop => "Show Desktop",
            SystemActionId::ToggleAppearance => "Toggle System Appearance",
            SystemActionId::ToggleStageManager => "Toggle Stage Manager",
            SystemActionId::OpenTrash => "Open Trash",
            SystemActionId::EmptyTrash => "Empty Trash",
            SystemActionId::EjectAllDisks => "Eject All Disks",
            SystemActionId::ToggleHiddenFiles => "Toggle Hidden Files",
            SystemActionId::HideOtherApps => "Hide All Apps Except Frontmost",
            SystemActionId::UnhideAllApps => "Unhide All Hidden Apps",
            SystemActionId::QuitAllApps => "Quit All Applications",
            SystemActionId::DismissNotifications => "Dismiss Notifications",
            SystemActionId::ToggleBluetooth => "Toggle Bluetooth",
        }
    }

    pub fn confirmation(self) -> Confirmation {
        match self {
            SystemActionId::Restart => Confirmation::Required {
                title: "Restart your PC?",
                message: SESSION_ENDING_MESSAGE,
            },
            SystemActionId::ShutDown => Confirmation::Required {
                title: "Shut down your PC?",
                message: SESSION_ENDING_MESSAGE,
            },
            SystemActionId::LogOut => Confirmation::Required {
                title: "Log out now?",
                message: SESSION_ENDING_MESSAGE,
            },
            SystemActionId::EmptyTrash => Confirmation::Required {
                title: "Empty Trash?",
                message: "The items in the Trash will be permanently deleted.",
            },
            SystemActionId::QuitAllApps => Confirmation::Computed,
            _ => Confirmation::None,
        }
    }

    pub fn sf_symbol(self) -> &'static str {
        match self {
            SystemActionId::LockScreen => "lock",
            SystemActionId::Sleep => "moon.zzz",
            SystemActionId::SleepDisplays => "display",
            SystemActionId::Restart => "arrow.clockwise",
            SystemActionId::ShutDown => "power",
            SystemActionId::LogOut => "rectangle.portrait.and.arrow.right",
            SystemActionId::ShowScreenSaver => "rectangle.inset.filled",
            SystemActionId::PlayPause => "playpause",
            SystemActionId::NextTrack => "forward.end",
            SystemActionId::PreviousTrack => "backward.end",
            SystemActionId::ToggleMute => "speaker.slash",
            SystemActionId::VolumeUp => "speaker.plus",
            SystemActionId::VolumeDown => "speaker.minus",
            SystemActionId::SetVolume
            | SystemActionId::Volume0
            | SystemActionId::Volume25
            | SystemActionId::Volume50
            | SystemActionId::Volume75
            | SystemActionId::Volume100 => "speaker.wave.2",
            SystemActionId::ShowDesktop => "macwindow.on.rectangle",
            SystemActionId::ToggleAppearance => "circle.lefthalf.filled",
            SystemActionId::ToggleStageManager => "squares.leading.rectangle",
            SystemActionId::OpenTrash => "trash",
            SystemActionId::EmptyTrash => "trash.slash",
            SystemActionId::EjectAllDisks => "eject",
            SystemActionId::ToggleHiddenFiles => "eye.slash",
            SystemActionId::HideOtherApps => "eye.slash.circle",
            SystemActionId::UnhideAllApps => "eye.circle",
            SystemActionId::QuitAllApps => "xmark.circle",
            SystemActionId::DismissNotifications => "bell.slash",
            SystemActionId::ToggleBluetooth => "bluetooth",
        }
    }

    pub fn entry_id(self) -> String {
        format!("system-action:{}", self.raw())
    }

    pub fn as_entry(self) -> AppEntry {
        let name = self.name().to_string();
        AppEntry {
            id: self.entry_id(),
            kind: AppKind::SystemAction,
            name: name.clone(),
            fields: SearchFields {
                display_name: name,
                ..Default::default()
            },
            hotkey: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_confirms_pc_not_mac() {
        match SystemActionId::Restart.confirmation() {
            Confirmation::Required { title, .. } => assert!(title.contains("PC")),
            _ => panic!(),
        }
    }

    #[test]
    fn stage_manager_still_exists() {
        assert!(SystemActionId::all()
            .iter()
            .any(|a| a.raw() == "toggle-stage-manager"));
    }

    #[test]
    fn catalog_ids_match_v0102() {
        const EXPECTED: &[(&str, &str)] = &[
            ("lock-screen", "Lock Screen"),
            ("sleep", "Sleep"),
            ("sleep-displays", "Sleep Displays"),
            ("restart", "Restart"),
            ("shut-down", "Shut Down"),
            ("log-out", "Log Out"),
            ("show-screen-saver", "Show Screen Saver"),
            ("play-pause", "Play / Pause"),
            ("next-track", "Next Track"),
            ("previous-track", "Previous Track"),
            ("toggle-mute", "Toggle Mute"),
            ("volume-up", "Turn Volume Up"),
            ("volume-down", "Turn Volume Down"),
            ("set-volume", "Set Volume…"),
            ("volume-0", "Set Volume to 0%"),
            ("volume-25", "Set Volume to 25%"),
            ("volume-50", "Set Volume to 50%"),
            ("volume-75", "Set Volume to 75%"),
            ("volume-100", "Set Volume to 100%"),
            ("show-desktop", "Show Desktop"),
            ("toggle-system-appearance", "Toggle System Appearance"),
            ("toggle-stage-manager", "Toggle Stage Manager"),
            ("open-trash", "Open Trash"),
            ("empty-trash", "Empty Trash"),
            ("eject-all-disks", "Eject All Disks"),
            ("toggle-hidden-files", "Toggle Hidden Files"),
            (
                "hide-all-apps-except-frontmost",
                "Hide All Apps Except Frontmost",
            ),
            ("unhide-all-hidden-apps", "Unhide All Hidden Apps"),
            ("quit-all-apps", "Quit All Applications"),
            ("dismiss-notifications", "Dismiss Notifications"),
            ("toggle-bluetooth", "Toggle Bluetooth"),
        ];
        let actual: Vec<(&str, &str)> = SystemActionId::all()
            .iter()
            .copied()
            .map(|id| (id.raw(), id.name()))
            .collect();
        assert_eq!(actual, EXPECTED);
    }

    #[test]
    fn session_ending_confirmations_say_pc() {
        match SystemActionId::Restart.confirmation() {
            Confirmation::Required { title, message } => {
                assert_eq!(title, "Restart your PC?");
                assert!(!title.contains("Mac"));
                assert_eq!(
                    message,
                    "Applications with unsaved changes may ask you to save."
                );
            }
            other => panic!("{other:?}"),
        }
        match SystemActionId::ShutDown.confirmation() {
            Confirmation::Required { title, .. } => {
                assert_eq!(title, "Shut down your PC?");
                assert!(!title.contains("Mac"));
            }
            other => panic!("{other:?}"),
        }
        match SystemActionId::LogOut.confirmation() {
            Confirmation::Required { title, .. } => assert_eq!(title, "Log out now?"),
            other => panic!("{other:?}"),
        }
        match SystemActionId::EmptyTrash.confirmation() {
            Confirmation::Required { title, message } => {
                assert_eq!(title, "Empty Trash?");
                assert!(message.contains("permanently deleted"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            SystemActionId::QuitAllApps.confirmation(),
            Confirmation::Computed
        );
        assert_eq!(
            SystemActionId::LockScreen.confirmation(),
            Confirmation::None
        );
    }

    #[test]
    fn coordinator_cannot_skip_confirmation() {
        assert_ne!(
            SystemActionId::EmptyTrash.confirmation(),
            Confirmation::None
        );
    }

    #[test]
    fn as_entry_uses_system_action_kind() {
        let entry = SystemActionId::LockScreen.as_entry();
        assert_eq!(entry.id, "system-action:lock-screen");
        assert_eq!(entry.kind, AppKind::SystemAction);
        assert_eq!(entry.name, "Lock Screen");
        assert_eq!(entry.fields.display_name, "Lock Screen");
        assert!(entry.hotkey.is_none());
    }
}
