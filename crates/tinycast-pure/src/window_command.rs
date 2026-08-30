use crate::app_entry::{AppEntry, AppKind};
use crate::search_relevance::SearchFields;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WindowCommandId {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeftQuarter,
    TopRightQuarter,
    BottomLeftQuarter,
    BottomRightQuarter,
    FirstThreeFourths,
    LastThreeFourths,
    FirstThird,
    CenterThird,
    LastThird,
    FirstTwoThirds,
    LastTwoThirds,
    Maximize,
    AlmostMaximize,
    ReasonableSize,
    MaximizeHeight,
    MaximizeWidth,
    Center,
    CenterHalf,
    MakeLarger,
    MakeSmaller,
    Restore,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    NextDisplay,
    PreviousDisplay,
    ToggleFullscreen,
    PreviousSpace,
    NextSpace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WindowKind {
    Geometry,
    Restore,
    Fullscreen,
    Space,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WindowGroup {
    Halves,
    Quarters,
    Fourths,
    Thirds,
    Sizing,
    Moving,
    Fullscreen,
    Spaces,
}

impl WindowGroup {
    pub fn all() -> &'static [Self] {
        &[
            WindowGroup::Halves,
            WindowGroup::Quarters,
            WindowGroup::Fourths,
            WindowGroup::Thirds,
            WindowGroup::Sizing,
            WindowGroup::Moving,
            WindowGroup::Fullscreen,
            WindowGroup::Spaces,
        ]
    }

    pub fn title(self) -> &'static str {
        match self {
            WindowGroup::Halves => "Halves",
            WindowGroup::Quarters => "Quarters",
            WindowGroup::Fourths => "Fourths",
            WindowGroup::Thirds => "Thirds",
            WindowGroup::Sizing => "Sizing",
            WindowGroup::Moving => "Moving",
            WindowGroup::Fullscreen => "Fullscreen",
            WindowGroup::Spaces => "Spaces",
        }
    }
}

impl WindowCommandId {
    pub const ALL: &'static [WindowCommandId] = &[
        WindowCommandId::LeftHalf,
        WindowCommandId::RightHalf,
        WindowCommandId::TopHalf,
        WindowCommandId::BottomHalf,
        WindowCommandId::TopLeftQuarter,
        WindowCommandId::TopRightQuarter,
        WindowCommandId::BottomLeftQuarter,
        WindowCommandId::BottomRightQuarter,
        WindowCommandId::FirstThreeFourths,
        WindowCommandId::LastThreeFourths,
        WindowCommandId::FirstThird,
        WindowCommandId::CenterThird,
        WindowCommandId::LastThird,
        WindowCommandId::FirstTwoThirds,
        WindowCommandId::LastTwoThirds,
        WindowCommandId::Maximize,
        WindowCommandId::AlmostMaximize,
        WindowCommandId::ReasonableSize,
        WindowCommandId::MaximizeHeight,
        WindowCommandId::MaximizeWidth,
        WindowCommandId::Center,
        WindowCommandId::CenterHalf,
        WindowCommandId::MakeLarger,
        WindowCommandId::MakeSmaller,
        WindowCommandId::Restore,
        WindowCommandId::MoveLeft,
        WindowCommandId::MoveRight,
        WindowCommandId::MoveUp,
        WindowCommandId::MoveDown,
        WindowCommandId::NextDisplay,
        WindowCommandId::PreviousDisplay,
        WindowCommandId::ToggleFullscreen,
        WindowCommandId::PreviousSpace,
        WindowCommandId::NextSpace,
    ];

    pub fn all() -> &'static [Self] {
        Self::ALL
    }

    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.raw() == raw)
    }

    pub fn from_entry_id(entry_id: &str) -> Option<Self> {
        entry_id
            .strip_prefix("window-command:")
            .and_then(Self::from_raw)
    }

    pub fn raw(self) -> &'static str {
        match self {
            WindowCommandId::LeftHalf => "left-half",
            WindowCommandId::RightHalf => "right-half",
            WindowCommandId::TopHalf => "top-half",
            WindowCommandId::BottomHalf => "bottom-half",
            WindowCommandId::TopLeftQuarter => "top-left-quarter",
            WindowCommandId::TopRightQuarter => "top-right-quarter",
            WindowCommandId::BottomLeftQuarter => "bottom-left-quarter",
            WindowCommandId::BottomRightQuarter => "bottom-right-quarter",
            WindowCommandId::FirstThreeFourths => "first-three-fourths",
            WindowCommandId::LastThreeFourths => "last-three-fourths",
            WindowCommandId::FirstThird => "first-third",
            WindowCommandId::CenterThird => "center-third",
            WindowCommandId::LastThird => "last-third",
            WindowCommandId::FirstTwoThirds => "first-two-thirds",
            WindowCommandId::LastTwoThirds => "last-two-thirds",
            WindowCommandId::Maximize => "maximize",
            WindowCommandId::AlmostMaximize => "almost-maximize",
            WindowCommandId::ReasonableSize => "reasonable-size",
            WindowCommandId::MaximizeHeight => "maximize-height",
            WindowCommandId::MaximizeWidth => "maximize-width",
            WindowCommandId::Center => "center",
            WindowCommandId::CenterHalf => "center-half",
            WindowCommandId::MakeLarger => "make-larger",
            WindowCommandId::MakeSmaller => "make-smaller",
            WindowCommandId::Restore => "restore",
            WindowCommandId::MoveLeft => "move-left",
            WindowCommandId::MoveRight => "move-right",
            WindowCommandId::MoveUp => "move-up",
            WindowCommandId::MoveDown => "move-down",
            WindowCommandId::NextDisplay => "next-display",
            WindowCommandId::PreviousDisplay => "previous-display",
            WindowCommandId::ToggleFullscreen => "toggle-fullscreen",
            WindowCommandId::PreviousSpace => "previous-space",
            WindowCommandId::NextSpace => "next-space",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            WindowCommandId::LeftHalf => "Left Half",
            WindowCommandId::RightHalf => "Right Half",
            WindowCommandId::TopHalf => "Top Half",
            WindowCommandId::BottomHalf => "Bottom Half",
            WindowCommandId::TopLeftQuarter => "Top Left Quarter",
            WindowCommandId::TopRightQuarter => "Top Right Quarter",
            WindowCommandId::BottomLeftQuarter => "Bottom Left Quarter",
            WindowCommandId::BottomRightQuarter => "Bottom Right Quarter",
            WindowCommandId::FirstThreeFourths => "First Three Fourths",
            WindowCommandId::LastThreeFourths => "Last Three Fourths",
            WindowCommandId::FirstThird => "First Third",
            WindowCommandId::CenterThird => "Center Third",
            WindowCommandId::LastThird => "Last Third",
            WindowCommandId::FirstTwoThirds => "First Two Thirds",
            WindowCommandId::LastTwoThirds => "Last Two Thirds",
            WindowCommandId::Maximize => "Maximize",
            WindowCommandId::AlmostMaximize => "Almost Maximize",
            WindowCommandId::ReasonableSize => "Reasonable Size",
            WindowCommandId::MaximizeHeight => "Maximize Height",
            WindowCommandId::MaximizeWidth => "Maximize Width",
            WindowCommandId::Center => "Center",
            WindowCommandId::CenterHalf => "Center Half",
            WindowCommandId::MakeLarger => "Make Larger",
            WindowCommandId::MakeSmaller => "Make Smaller",
            WindowCommandId::Restore => "Restore Window",
            WindowCommandId::MoveLeft => "Move Left",
            WindowCommandId::MoveRight => "Move Right",
            WindowCommandId::MoveUp => "Move Up",
            WindowCommandId::MoveDown => "Move Down",
            WindowCommandId::NextDisplay => "Move to Next Display",
            WindowCommandId::PreviousDisplay => "Move to Previous Display",
            WindowCommandId::ToggleFullscreen => "Toggle Fullscreen",
            WindowCommandId::PreviousSpace => "Switch to Previous Space",
            WindowCommandId::NextSpace => "Switch to Next Space",
        }
    }

    pub fn kind(self) -> WindowKind {
        match self {
            WindowCommandId::Restore => WindowKind::Restore,
            WindowCommandId::ToggleFullscreen => WindowKind::Fullscreen,
            WindowCommandId::PreviousSpace | WindowCommandId::NextSpace => WindowKind::Space,
            _ => WindowKind::Geometry,
        }
    }

    pub fn group(self) -> WindowGroup {
        match self {
            WindowCommandId::LeftHalf
            | WindowCommandId::RightHalf
            | WindowCommandId::TopHalf
            | WindowCommandId::BottomHalf => WindowGroup::Halves,
            WindowCommandId::TopLeftQuarter
            | WindowCommandId::TopRightQuarter
            | WindowCommandId::BottomLeftQuarter
            | WindowCommandId::BottomRightQuarter => WindowGroup::Quarters,
            WindowCommandId::FirstThreeFourths | WindowCommandId::LastThreeFourths => {
                WindowGroup::Fourths
            }
            WindowCommandId::FirstThird
            | WindowCommandId::CenterThird
            | WindowCommandId::LastThird
            | WindowCommandId::FirstTwoThirds
            | WindowCommandId::LastTwoThirds => WindowGroup::Thirds,
            WindowCommandId::Maximize
            | WindowCommandId::AlmostMaximize
            | WindowCommandId::ReasonableSize
            | WindowCommandId::MaximizeHeight
            | WindowCommandId::MaximizeWidth
            | WindowCommandId::Center
            | WindowCommandId::CenterHalf
            | WindowCommandId::MakeLarger
            | WindowCommandId::MakeSmaller
            | WindowCommandId::Restore => WindowGroup::Sizing,
            WindowCommandId::MoveLeft
            | WindowCommandId::MoveRight
            | WindowCommandId::MoveUp
            | WindowCommandId::MoveDown
            | WindowCommandId::NextDisplay
            | WindowCommandId::PreviousDisplay => WindowGroup::Moving,
            WindowCommandId::ToggleFullscreen => WindowGroup::Fullscreen,
            WindowCommandId::PreviousSpace | WindowCommandId::NextSpace => WindowGroup::Spaces,
        }
    }

    pub fn cycles_on_repeat(self) -> bool {
        matches!(
            self,
            WindowCommandId::LeftHalf
                | WindowCommandId::RightHalf
                | WindowCommandId::TopHalf
                | WindowCommandId::BottomHalf
        )
    }

    pub fn resizes(self) -> bool {
        !matches!(
            self,
            WindowCommandId::MoveLeft
                | WindowCommandId::MoveRight
                | WindowCommandId::MoveUp
                | WindowCommandId::MoveDown
        )
    }

    pub fn entry_id(self) -> String {
        format!("window-command:{}", self.raw())
    }

    pub fn as_entry(self) -> AppEntry {
        let name = self.name().to_string();
        AppEntry {
            id: self.entry_id(),
            kind: AppKind::WindowCommand,
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
    use crate::palette_placement::DipRect;
    use crate::window_layout::{placement, LayoutInput};

    #[test]
    fn there_are_34_window_commands() {
        assert_eq!(WindowCommandId::all().len(), 34);
    }

    #[test]
    fn left_half_uses_work_area_and_gap() {
        let vis = DipRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 800.0,
        };
        let r = placement(
            WindowCommandId::LeftHalf,
            LayoutInput {
                visible: vis,
                window: vis,
                gap: 10.0,
                step: 0,
            },
        )
        .unwrap();
        assert!((r.x - 10.0).abs() < 0.01);
        assert!(r.w <= 500.0);
    }

    #[test]
    fn make_larger_then_smaller_round_trips_on_even_screen() {
        let vis = DipRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 800.0,
        };
        let window = DipRect {
            x: 200.0,
            y: 150.0,
            w: 400.0,
            h: 300.0,
        };
        let input = LayoutInput {
            visible: vis,
            window,
            gap: 0.0,
            step: 0,
        };
        let larger = placement(WindowCommandId::MakeLarger, input).unwrap();
        let back = placement(
            WindowCommandId::MakeSmaller,
            LayoutInput {
                visible: vis,
                window: larger,
                gap: 0.0,
                step: 0,
            },
        )
        .unwrap();
        assert!((back.x - window.x).abs() < 0.01);
        assert!((back.y - window.y).abs() < 0.01);
        assert!((back.w - window.w).abs() < 0.01);
        assert!((back.h - window.h).abs() < 0.01);
    }

    #[test]
    fn space_commands_are_not_geometry() {
        assert_eq!(WindowCommandId::NextSpace.kind(), WindowKind::Space);
        assert_eq!(WindowCommandId::Restore.kind(), WindowKind::Restore);
    }

    #[test]
    fn catalog_ids_match_v0102() {
        assert_eq!(WindowCommandId::LeftHalf.raw(), "left-half");
        assert_eq!(WindowCommandId::Restore.name(), "Restore Window");
        assert_eq!(WindowCommandId::NextSpace.name(), "Switch to Next Space");
        assert_eq!(WindowCommandId::NextSpace.kind(), WindowKind::Space);
        assert_eq!(WindowCommandId::Restore.kind(), WindowKind::Restore);
        assert_eq!(
            WindowCommandId::ToggleFullscreen.kind(),
            WindowKind::Fullscreen
        );
        assert!(WindowCommandId::LeftHalf.cycles_on_repeat());
        assert!(!WindowCommandId::Center.cycles_on_repeat());
        assert!(!WindowCommandId::MoveLeft.resizes());
        assert_eq!(
            WindowCommandId::LeftHalf.as_entry().id,
            "window-command:left-half"
        );
    }
}
