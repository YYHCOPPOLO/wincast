//! Manual sample sheet for the Slint primitives. Not a test: it asserts
//! nothing and writes no files. Run with
//! `cargo run -p tinycast-ui --example gallery`.

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use tinycast_pure::i18n::{listening_label, UiLang};
use tinycast_ui::{GalleryRow, GalleryWindow, MenuItem};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()?;

    let ui = GalleryWindow::new()?;
    tinycast_ui::apply_appearance(&ui, 0);

    ui.set_window_title("Tinycast 组件样张".into());
    ui.set_gallery_title("组件样张".into());
    ui.set_light_label("浅色".into());
    ui.set_dark_label("深色".into());
    ui.set_placeholder("搜索应用、命令和文件".into());
    ui.set_search_symbol("\u{E721}".into());
    ui.set_section_title("应用程序".into());

    let rows = vec![
        row("照片", "⌘1", "", "\u{E91B}", true),
        row("日历", "⌘2", "", "\u{E787}", false),
        row("备忘录", "", "", "\u{E70B}", false),
        row("邮件", "", "↵", "\u{E715}", false),
        row("音乐", "", "", "\u{E8D6}", false),
        row("终端", "", "", "", false),
    ];
    ui.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));

    ui.set_primary_label("打开".into());
    ui.set_actions_label("操作".into());
    ui.set_primary_shortcut("↵".into());
    ui.set_actions_shortcut("Ctrl K".into());

    ui.set_keycap_title("键帽".into());
    ui.set_keycap_outline("↵".into());
    ui.set_keycap_filled("Tab".into());
    ui.set_keycap_hero("Enter".into());

    ui.set_settings_title("设置分组".into());
    ui.set_toggle_label("启用笔记".into());
    ui.set_checkbox_label("显示在启动器".into());
    ui.set_sidebar_a("通用".into());
    ui.set_sidebar_b("外观".into());
    ui.set_sidebar_a_symbol("".into());
    ui.set_sidebar_b_symbol("".into());

    ui.set_menu_header("操作".into());
    let menu = vec![
        menu_item("打开", &["↵"], "\u{E8E5}", false, true),
        menu_item("在文件夹中显示", &["Ctrl", "O"], "", false, true),
        menu_item("复制路径", &[], "", false, false),
        menu_item("卸载应用", &[], "", true, true),
    ];
    ui.set_menu_items(ModelRc::from(Rc::new(VecModel::from(menu))));

    ui.set_scrollbar_title("滚动条".into());
    ui.set_dialog_title("重启电脑".into());
    ui.set_dialog_message("确定要立即重启吗？未保存的工作会丢失。".into());
    ui.set_dialog_accept("重启".into());
    ui.set_dialog_cancel("取消".into());
    ui.set_dialog_symbol("".into());
    ui.set_hud_message("已复制到剪贴板".into());
    ui.set_hud_detail("3 个项目".into());
    ui.set_hud_symbol("".into());

    // The recorder bubble's caption is i18n copy, never a Slint literal.
    ui.set_recorder_title("快捷键录音".into());
    ui.set_recorder_caption(listening_label(UiLang::ZhHans).into());
    let recorder_keys = vec![SharedString::from("Ctrl"), SharedString::from("K")];
    ui.set_recorder_keys(ModelRc::from(Rc::new(VecModel::from(recorder_keys))));
    ui.set_recorder_esc("esc".into());
    ui.set_recorder_reveal(true);

    // The Magpie settings vocabulary, one sample per primitive.
    ui.set_magpie_header("全局快捷键".into());
    ui.set_magpie_select("跟随系统".into());
    ui.set_magpie_chip("已接入".into());
    ui.set_magpie_badge_ok("已连接".into());
    ui.set_magpie_badge_warn("限流中".into());
    ui.set_magpie_action("打开设置…".into());
    ui.set_magpie_danger("移除".into());
    ui.set_magpie_input_hint("搜索应用…".into());
    ui.set_magpie_cell_title("Claude 订阅".into());
    ui.set_magpie_cell_subtitle("Pro · Max · Team".into());
    ui.set_magpie_cell_symbol("\u{E945}".into());
    ui.set_magpie_empty("没有匹配的应用".into());
    ui.set_magpie_empty_action("添加应用…".into());

    let weak = ui.as_weak();
    ui.on_toggle_appearance(move || {
        if let Some(ui) = weak.upgrade() {
            let next = if ui.get_appearance() == 0 { 1 } else { 0 };
            ui.set_appearance(next);
            tinycast_ui::apply_appearance(&ui, next as u8);
        }
    });

    ui.run()?;
    Ok(())
}

fn menu_item(
    label: &str,
    shortcut: &[&str],
    symbol: &str,
    destructive: bool,
    enabled: bool,
) -> MenuItem {
    let caps: Vec<SharedString> = shortcut
        .iter()
        .map(|cap| SharedString::from(*cap))
        .collect();
    MenuItem {
        label: label.into(),
        shortcut: ModelRc::new(VecModel::from(caps)),
        symbol: symbol.into(),
        destructive,
        enabled,
    }
}

fn row(title: &str, trailing: &str, keycap: &str, symbol: &str, selected: bool) -> GalleryRow {
    GalleryRow {
        title: title.into(),
        trailing: trailing.into(),
        keycap: keycap.into(),
        symbol: symbol.into(),
        selected,
    }
}
