use crate::app_entry::AppKind;
use crate::palette_placement::DipRect;
use crate::theme;

pub const ID_OPEN: &str = "open";
pub const ID_FAVORITE: &str = "favorite";
pub const ID_MOVE_UP: &str = "move-up";
pub const ID_MOVE_DOWN: &str = "move-down";
pub const ID_RESET_RANKING: &str = "reset-ranking";
pub const ID_SHOW_IN_FOLDER: &str = "show-in-folder";
pub const ID_COPY_PATH: &str = "copy-path";
pub const ID_QUIT: &str = "quit";
pub const ID_UNINSTALL: &str = "uninstall";

/// Inset from the palette's bottom-trailing corner so the menu's own radius isn't clipped.
pub const MENU_INSET: f32 = 8.0;
pub const MENU_ROW_GAP: f32 = 1.0;
pub const FOOTER_BUTTON_GAP: f32 = 2.0;
pub const PRIMARY_BUTTON_WIDTH: f32 = 176.0;
pub const ACTIONS_BUTTON_WIDTH: f32 = 126.0;
pub const ACTIONS_SHORTCUT: &str = "Ctrl+K";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuItem {
    pub id: &'static str,
    pub label: String,
    pub shortcut: Option<&'static str>,
}

impl MenuItem {
    pub fn is_destructive(&self) -> bool {
        matches!(self.id, ID_UNINSTALL | ID_QUIT)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionContext {
    pub kind: AppKind,
    pub is_favorite: bool,
    pub can_move_up: bool,
    pub can_move_down: bool,
    pub has_ranking: bool,
    pub running: bool,
}

impl ActionContext {
    pub fn for_kind(kind: AppKind) -> Self {
        Self {
            kind,
            is_favorite: false,
            can_move_up: false,
            can_move_down: false,
            has_ranking: false,
            running: false,
        }
    }
}

/// Default Actions rows for a kind (not favorite, no ranking, not running).
pub fn launcher_actions(kind: AppKind) -> Vec<MenuItem> {
    actions_for(ActionContext::for_kind(kind))
}

/// Snapshot of the Actions menu for the current selection.
pub fn actions_for(ctx: ActionContext) -> Vec<MenuItem> {
    let mut items = vec![item(ID_OPEN, ctx.kind.open_verb(), Some("↵"))];
    items.push(item(
        ID_FAVORITE,
        if ctx.is_favorite {
            "Remove from Favorites"
        } else {
            "Add to Favorites"
        },
        Some("Ctrl+Shift+F"),
    ));
    if ctx.can_move_up {
        items.push(item(ID_MOVE_UP, "Move Favorite Up", Some("Ctrl+Alt+Up")));
    }
    if ctx.can_move_down {
        items.push(item(
            ID_MOVE_DOWN,
            "Move Favorite Down",
            Some("Ctrl+Alt+Down"),
        ));
    }
    if ctx.has_ranking {
        items.push(item(ID_RESET_RANKING, "Reset Ranking", None));
    }
    if ctx.kind.can_reveal_in_folder() {
        items.push(item(
            ID_SHOW_IN_FOLDER,
            "Show in Folder",
            Some("Ctrl+Enter"),
        ));
    }
    if can_copy_path(ctx.kind) {
        items.push(item(ID_COPY_PATH, "Copy Path", Some("Ctrl+Alt+C")));
    }
    if ctx.running && ctx.kind == AppKind::Application {
        items.push(item(ID_QUIT, "Quit Application", Some("Ctrl+Shift+Q")));
    }
    if ctx.kind == AppKind::Application {
        items.push(item(ID_UNINSTALL, "Uninstall Application", None));
    }
    items
}

fn can_copy_path(kind: AppKind) -> bool {
    matches!(kind, AppKind::Application | AppKind::SystemSettings)
}

fn item(id: &'static str, label: &'static str, shortcut: Option<&'static str>) -> MenuItem {
    MenuItem {
        id,
        label: label.to_string(),
        shortcut,
    }
}

/// At most one in-window menu.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum OpenMenu {
    #[default]
    None,
    Actions,
    ClipboardFilter,
    AiModel,
    AppMenu,
}

impl OpenMenu {
    pub fn is_open(self) -> bool {
        self != OpenMenu::None
    }

    pub fn toggle_actions(self) -> Self {
        match self {
            OpenMenu::Actions => OpenMenu::None,
            OpenMenu::None | OpenMenu::ClipboardFilter | OpenMenu::AiModel | OpenMenu::AppMenu => {
                OpenMenu::Actions
            }
        }
    }
}

/// ⌘K has no compact-bar anchor; empty lists and non-actionable rows swallow the chord.
pub fn can_open_actions(expanded: bool, has_rows: bool) -> bool {
    expanded && has_rows
}

pub fn menu_row_height() -> f32 {
    theme::size::MENU_ICON + theme::spacing::MD * 2.0
}

pub fn menu_header_height() -> f32 {
    theme::spacing::XS + theme::spacing::XS / 2.0 + 16.0
}

pub fn menu_height(item_count: usize, has_header: bool) -> f32 {
    let pad = theme::spacing::SM * 2.0;
    let header = if has_header {
        menu_header_height() + MENU_ROW_GAP
    } else {
        0.0
    };
    let rows = item_count as f32 * menu_row_height();
    let gaps = item_count.saturating_sub(1) as f32 * MENU_ROW_GAP;
    pad + header + rows + gaps
}

/// Actions popover, anchored `.bottomTrailing` of the palette.
pub fn actions_menu_frame(
    panel_w: f32,
    panel_h: f32,
    item_count: usize,
    has_header: bool,
) -> DipRect {
    popover_menu_frame(
        panel_w,
        panel_h,
        item_count,
        has_header,
        theme::size::MENU_WIDTH,
        false,
    )
}

/// Clipboard type filter, anchored `.topTrailing` under the header button.
pub fn clipboard_filter_menu_frame(
    panel_w: f32,
    panel_h: f32,
    item_count: usize,
    has_header: bool,
) -> DipRect {
    popover_menu_frame(
        panel_w,
        panel_h,
        item_count,
        has_header,
        theme::size::CLIPBOARD_FILTER_MENU_WIDTH,
        true,
    )
}

pub fn menu_frame(
    kind: OpenMenu,
    panel_w: f32,
    panel_h: f32,
    item_count: usize,
    has_header: bool,
) -> DipRect {
    match kind {
        OpenMenu::ClipboardFilter | OpenMenu::AiModel => {
            clipboard_filter_menu_frame(panel_w, panel_h, item_count, has_header)
        }
        _ => actions_menu_frame(panel_w, panel_h, item_count, has_header),
    }
}

fn popover_menu_frame(
    panel_w: f32,
    panel_h: f32,
    item_count: usize,
    has_header: bool,
    w: f32,
    top_trailing: bool,
) -> DipRect {
    let h = menu_height(item_count, has_header);
    let x = (panel_w - MENU_INSET - w).max(MENU_INSET);
    let y = if top_trailing {
        let below_header = theme::size::COMPACT_HEIGHT;
        if below_header + h > panel_h - MENU_INSET {
            (panel_h - MENU_INSET - h).max(MENU_INSET)
        } else {
            below_header
        }
    } else {
        (panel_h - MENU_INSET - h).max(MENU_INSET)
    };
    DipRect { x, y, w, h }
}

pub fn menu_button_rect(panel_h: f32) -> DipRect {
    let bar_h = theme::size::BOTTOM_BAR_HEIGHT;
    let bar_y = panel_h - bar_h;
    let inset = theme::spacing::MD;
    let d = theme::size::MENU_BUTTON;
    DipRect {
        x: inset,
        y: bar_y + (bar_h - d) / 2.0,
        w: d,
        h: d,
    }
}

pub fn menu_line_rects(circle: DipRect) -> (DipRect, DipRect) {
    let x = circle.x + (circle.w - 14.0) / 2.0;
    let mid = circle.y + circle.h / 2.0;
    let top = DipRect {
        x,
        y: mid - 1.5 - 1.5,
        w: 14.0,
        h: 1.5,
    };
    let bot = DipRect {
        x,
        y: mid + 1.5,
        w: 8.0,
        h: 1.5,
    };
    (top, bot)
}

pub fn point_in(rect: DipRect, x: f32, y: f32) -> bool {
    x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
}

pub fn menu_row_at(
    frame: DipRect,
    has_header: bool,
    item_count: usize,
    x: f32,
    y: f32,
) -> Option<usize> {
    if !point_in(frame, x, y) || item_count == 0 {
        return None;
    }
    let mut cursor = frame.y + theme::spacing::SM;
    if has_header {
        cursor += menu_header_height() + MENU_ROW_GAP;
    }
    let row_h = menu_row_height();
    for i in 0..item_count {
        if y >= cursor && y < cursor + row_h {
            return Some(i);
        }
        cursor += row_h + MENU_ROW_GAP;
    }
    None
}

pub fn menu_row_rect(frame: DipRect, has_header: bool, index: usize) -> DipRect {
    let mut y = frame.y + theme::spacing::SM;
    if has_header {
        y += menu_header_height() + MENU_ROW_GAP;
    }
    y += index as f32 * (menu_row_height() + MENU_ROW_GAP);
    DipRect {
        x: frame.x + theme::spacing::SM,
        y,
        w: (frame.w - theme::spacing::SM * 2.0).max(0.0),
        h: menu_row_height(),
    }
}

pub fn menu_header_rect(frame: DipRect) -> DipRect {
    DipRect {
        x: frame.x + theme::spacing::LG,
        y: frame.y + theme::spacing::SM,
        w: (frame.w - theme::spacing::LG * 2.0).max(0.0),
        h: menu_header_height(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionGroupRects {
    pub capsule: DipRect,
    pub primary: DipRect,
    pub actions: DipRect,
}

pub fn action_group_rects_measured(
    panel_w: f32,
    panel_h: f32,
    primary_w: f32,
    actions_w: f32,
) -> Option<ActionGroupRects> {
    if panel_h < theme::size::COMPACT_HEIGHT + theme::size::BOTTOM_BAR_HEIGHT {
        return None;
    }
    let bar_h = theme::size::BOTTOM_BAR_HEIGHT;
    let bar_y = panel_h - bar_h;
    let cap_h = theme::size::BAR_BUTTON_HEIGHT;
    let pad = theme::spacing::XS;
    let inset = theme::spacing::MD;
    let capsule_w = pad * 2.0 + primary_w + FOOTER_BUTTON_GAP + actions_w;
    let capsule = DipRect {
        x: panel_w - inset - capsule_w,
        y: bar_y + (bar_h - cap_h) / 2.0,
        w: capsule_w,
        h: cap_h,
    };
    let primary = DipRect {
        x: capsule.x + pad,
        y: capsule.y,
        w: primary_w,
        h: cap_h,
    };
    let actions = DipRect {
        x: primary.x + primary_w + FOOTER_BUTTON_GAP,
        y: capsule.y,
        w: actions_w,
        h: cap_h,
    };
    Some(ActionGroupRects {
        capsule,
        primary,
        actions,
    })
}

pub fn action_group_rects(panel_w: f32, panel_h: f32) -> Option<ActionGroupRects> {
    action_group_rects_measured(
        panel_w,
        panel_h,
        PRIMARY_BUTTON_WIDTH,
        ACTIONS_BUTTON_WIDTH,
    )
}

pub fn clamp_menu_selection(selection: usize, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        selection.min(count - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_actions_include_uninstall_and_favorite() {
        let items = launcher_actions(AppKind::Application);
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Uninstall Application"));
        assert!(
            labels.contains(&"Toggle Favorite") || labels.iter().any(|l| l.contains("Favorite"))
        );
    }

    #[test]
    fn app_actions_include_open_show_in_folder_and_copy_path() {
        let items = launcher_actions(AppKind::Application);
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Open Application"));
        assert!(labels.contains(&"Show in Folder"));
        assert!(labels.contains(&"Copy Path"));
        assert!(labels.contains(&"Add to Favorites"));
        let uninstall = items.iter().find(|i| i.id == ID_UNINSTALL).unwrap();
        assert_eq!(uninstall.shortcut, None);
        let show = items.iter().find(|i| i.id == ID_SHOW_IN_FOLDER).unwrap();
        assert_eq!(show.shortcut, Some("Ctrl+Enter"));
        let fav = items.iter().find(|i| i.id == ID_FAVORITE).unwrap();
        assert_eq!(fav.shortcut, Some("Ctrl+Shift+F"));
        let copy = items.iter().find(|i| i.id == ID_COPY_PATH).unwrap();
        assert_eq!(copy.shortcut, Some("Ctrl+Alt+C"));
    }

    #[test]
    fn command_actions_omit_uninstall_and_show_in_folder() {
        let items = launcher_actions(AppKind::Command);
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Run Command"));
        assert!(!labels.contains(&"Uninstall Application"));
        assert!(!labels.contains(&"Show in Folder"));
        assert!(!labels.contains(&"Copy Path"));
        assert!(labels.iter().any(|l| l.contains("Favorite")));
    }

    #[test]
    fn favorite_and_ranking_rows_follow_context() {
        let mut ctx = ActionContext::for_kind(AppKind::Application);
        ctx.is_favorite = true;
        ctx.can_move_up = true;
        ctx.can_move_down = true;
        ctx.has_ranking = true;
        ctx.running = true;
        let items = actions_for(ctx);
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Remove from Favorites"));
        assert!(!labels.contains(&"Add to Favorites"));
        assert!(labels.contains(&"Move Favorite Up"));
        assert!(labels.contains(&"Move Favorite Down"));
        assert!(labels.contains(&"Reset Ranking"));
        assert!(labels.contains(&"Quit Application"));
    }

    #[test]
    fn actions_menu_is_276_dip_bottom_trailing() {
        assert_eq!(theme::size::MENU_WIDTH, 276.0);
        let frame = actions_menu_frame(750.0, 475.0, 5, true);
        assert_eq!(frame.w, 276.0);
        assert_eq!(frame.x, 750.0 - MENU_INSET - 276.0);
        assert!(frame.x > 750.0 / 2.0);
        assert!((frame.y + frame.h - (475.0 - MENU_INSET)).abs() < 0.01);
    }

    #[test]
    fn clipboard_filter_menu_is_200_dip_top_trailing() {
        assert_eq!(theme::size::CLIPBOARD_FILTER_MENU_WIDTH, 200.0);
        let frame = clipboard_filter_menu_frame(750.0, 475.0, 5, true);
        assert_eq!(frame.w, 200.0);
        assert_eq!(frame.x, 750.0 - MENU_INSET - 200.0);
        assert_eq!(frame.y, theme::size::COMPACT_HEIGHT);
        assert_eq!(
            menu_frame(OpenMenu::ClipboardFilter, 750.0, 475.0, 5, true).w,
            200.0
        );
    }

    #[test]
    fn one_open_menu_at_a_time_toggles_actions() {
        let mut open = OpenMenu::None;
        open = open.toggle_actions();
        assert_eq!(open, OpenMenu::Actions);
        assert!(open.is_open());
        open = open.toggle_actions();
        assert_eq!(open, OpenMenu::None);
        assert!(!can_open_actions(false, true));
        assert!(!can_open_actions(true, false));
        assert!(can_open_actions(true, true));
    }

    #[test]
    fn footer_actions_capsule_is_right_aligned() {
        let group = action_group_rects(750.0, 475.0).unwrap();
        assert!(group.actions.x > group.primary.x);
        assert!(group.capsule.x > 750.0 / 2.0);
        assert!(point_in(
            group.actions,
            group.actions.x + 4.0,
            group.actions.y + 4.0
        ));
        assert!(!point_in(
            group.actions,
            group.primary.x + 4.0,
            group.primary.y + 4.0
        ));
        assert!(action_group_rects(750.0, theme::size::COMPACT_HEIGHT).is_none());
    }

    #[test]
    fn menu_row_hit_maps_index() {
        let frame = actions_menu_frame(750.0, 475.0, 3, true);
        let r0 = menu_row_rect(frame, true, 0);
        assert_eq!(menu_row_at(frame, true, 3, r0.x + 8.0, r0.y + 4.0), Some(0));
        let r2 = menu_row_rect(frame, true, 2);
        assert_eq!(menu_row_at(frame, true, 3, r2.x + 8.0, r2.y + 4.0), Some(2));
        assert_eq!(menu_row_at(frame, true, 3, 10.0, 10.0), None);
        assert_eq!(clamp_menu_selection(9, 3), 2);
        assert_eq!(clamp_menu_selection(0, 0), 0);
    }

    #[test]
    fn menu_circle_uses_md_inset() {
        let r = menu_button_rect(475.0);
        assert_eq!(r.x, theme::spacing::MD);
        assert_eq!(r.w, theme::size::MENU_BUTTON);
    }

    #[test]
    fn measured_action_group_sits_in_md_margin() {
        let g = action_group_rects_measured(750.0, 475.0, 100.0, 80.0).unwrap();
        assert!((g.capsule.x + g.capsule.w - (750.0 - theme::spacing::MD)).abs() < 0.5);
    }

    #[test]
    fn hamburger_lines_are_14_and_8() {
        let c = DipRect {
            x: 8.0,
            y: 400.0,
            w: 36.0,
            h: 36.0,
        };
        let (a, b) = menu_line_rects(c);
        assert_eq!(a.w, 14.0);
        assert_eq!(b.w, 8.0);
        assert_eq!(a.h, 1.5);
    }
}
