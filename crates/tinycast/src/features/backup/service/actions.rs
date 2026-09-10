use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};

use crate::app_settings::AppSettings;

pub struct ImportSummary {
    pub settings_fields: usize,
}

pub fn export_json(settings: &AppSettings) -> serde_json::Value {
    let raw = serde_json::to_value(settings).unwrap_or_else(|_| serde_json::json!({}));
    tinycast_pure::settings_backup::filter_import(&raw)
}

pub fn apply_import(settings: &mut AppSettings, json: serde_json::Value) -> ImportSummary {
    let before = serde_json::to_value(&*settings).ok();
    settings.apply_backup(json);
    let after = serde_json::to_value(&*settings).ok();
    let mut fields = 0;
    if let (Some(serde_json::Value::Object(a)), Some(serde_json::Value::Object(b))) =
        (before, after)
    {
        for (k, v) in b {
            if a.get(&k) != Some(&v) {
                fields += 1;
            }
        }
    }
    ImportSummary {
        settings_fields: fields,
    }
}

pub fn pick_open_json(owner: HWND) -> Option<std::path::PathBuf> {
    pick(owner, false)
}

pub fn pick_save_json(owner: HWND) -> Option<std::path::PathBuf> {
    pick(owner, true)
}

fn pick(owner: HWND, save: bool) -> Option<std::path::PathBuf> {
    let mut file = vec![0u16; 1024];
    let mut filter: Vec<u16> = "JSON\0*.json;*.tinycast.json\0Raycast\0*.rayconfig\0All\0*.*\0\0"
        .encode_utf16()
        .collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: PCWSTR(filter.as_mut_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        Flags: OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                Default::default()
            },
        lpstrDefExt: w!("json"),
        ..Default::default()
    };
    let ok = unsafe {
        if save {
            GetSaveFileNameW(&mut ofn).as_bool()
        } else {
            GetOpenFileNameW(&mut ofn).as_bool()
        }
    };
    if !ok {
        return None;
    }
    let len = file.iter().position(|c| *c == 0).unwrap_or(file.len());
    let path = String::from_utf16_lossy(&file[..len]);
    if path.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_cannot_enable_snippets() {
        let mut s = AppSettings::default();
        apply_import(&mut s, serde_json::json!({"snippetsEnabled": true}));
        assert!(!s.snippets_enabled);
    }

    #[test]
    fn export_omits_excluded_keys() {
        let mut s = AppSettings::default();
        s.snippets_enabled = true;
        s.ai_enabled = true;
        s.calendar_enabled = true;
        let json = export_json(&s);
        assert!(json.get("snippetsEnabled").is_none());
        assert!(json.get("aiEnabled").is_none());
        assert!(json.get("calendarEnabled").is_none());
        assert!(json.get("autoJoinMeetings").is_none());
        assert!(json.get("cameraPreview").is_none());
    }
}
