use std::collections::HashMap;
use std::path::Path;

use tinycast_pure::app_entry::{AppEntry, AppKind};
use tinycast_pure::search_relevance::SearchFields;
use tinycast_pure::snippet::StoredSnippet;
use tinycast_pure::template::{
    expand_snippet, uses_selection, ArgumentSpec, ExpandContext, ExpandOutput, TemplateSnippet,
};

use crate::features::launcher::ui::list::PaintItem;

pub fn ensure_ui_automation() -> bool {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
    unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .ok()
            .is_some_and(|_: IUIAutomation| true)
    }
}

pub fn snippet_entry(record: &StoredSnippet) -> AppEntry {
    AppEntry {
        id: snippet_id(&record.path),
        kind: AppKind::Snippet,
        name: record.name.clone(),
        fields: SearchFields {
            display_name: record.name.clone(),
            snippet_keyword: record.keyword.clone(),
            ..Default::default()
        },
        hotkey: None,
    }
}

pub fn snippet_id(path: &Path) -> String {
    format!("snippet:{}", path.to_string_lossy())
}

pub fn path_from_entry_id(id: &str) -> Option<&str> {
    id.strip_prefix("snippet:")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentKind {
    Snippet,
    Quicklink,
}

#[derive(Clone, Debug)]
pub struct ArgumentSession {
    pub kind: ArgumentKind,
    pub snippet_path: String,
    pub snippet_name: String,
    pub show_confirmation: bool,
    pub specs: Vec<ArgumentSpec>,
    pub values: HashMap<String, String>,
    pub index: usize,
    pub clipboard: Vec<String>,
    pub selection: Option<String>,
    pub now: i64,
    pub locale: String,
    pub tz: String,
    pub keyword: Option<String>,
    pub target: isize,
}

impl ArgumentSession {
    pub fn current(&self) -> Option<&ArgumentSpec> {
        self.specs.get(self.index)
    }

    pub fn current_name(&self) -> &str {
        self.current()
            .map(|s| s.name.as_str())
            .unwrap_or("Argument")
    }

    pub fn current_options(&self) -> &[String] {
        self.current().map(|s| s.options.as_slice()).unwrap_or(&[])
    }

    pub fn filtered_options(&self, query: &str) -> Vec<String> {
        let q = query.to_lowercase();
        self.current_options()
            .iter()
            .filter(|opt| q.is_empty() || opt.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    pub fn commit(&mut self, value: String) -> bool {
        if let Some(spec) = self.specs.get(self.index) {
            self.values.insert(spec.name.clone(), value);
        }
        self.index += 1;
        self.index >= self.specs.len()
    }

    pub fn back(&mut self) -> Option<String> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        let name = self.specs.get(self.index)?.name.clone();
        self.values.remove(&name)
    }
}

pub fn paint_argument_items(
    session: &ArgumentSession,
    query: &str,
    selection: usize,
) -> Vec<PaintItem> {
    let mut items = Vec::new();
    if session.index > 0 {
        items.push(PaintItem::Header {
            title: "Arguments".into(),
        });
        for spec in session.specs.iter().take(session.index) {
            let value = session.values.get(&spec.name).cloned().unwrap_or_default();
            items.push(PaintItem::Row {
                title: spec.name.clone(),
                alias: None,
                trailing: value,
                keycap: None,
                icon_source: None,
                selected: false,
            });
        }
    }
    let options = session.filtered_options(query);
    if !options.is_empty() {
        items.push(PaintItem::Header {
            title: session.current_name().to_string(),
        });
        for (i, opt) in options.iter().enumerate() {
            items.push(PaintItem::Row {
                title: opt.clone(),
                alias: None,
                trailing: String::new(),
                keycap: None,
                icon_source: None,
                selected: i == selection,
            });
        }
    }
    items
}

pub fn expand_record(
    record: &StoredSnippet,
    library: &[StoredSnippet],
    ctx: &ExpandContext,
    args: &HashMap<String, String>,
) -> ExpandOutput {
    let snippets: Vec<TemplateSnippet> = library.iter().map(StoredSnippet::to_template).collect();
    expand_snippet(&record.body, ctx, args, &snippets, Some(&record.identity()))
}

pub fn expansion_context(
    clipboard: Vec<String>,
    selection: Option<String>,
    now: i64,
    locale: String,
) -> ExpandContext {
    ExpandContext {
        clipboard,
        selection,
        now,
        locale,
        tz: "UTC".into(),
        uuid: uuid_v4,
    }
}

/// UIA / synthetic Ctrl+C only when `{selection}` or `{selectedText}` is in the body.
pub fn capture_if_uses_selection(
    body: &str,
    capture: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if uses_selection(body) {
        capture()
    } else {
        None
    }
}

fn uuid_v4() -> String {
    match unsafe { windows::Win32::System::Com::CoCreateGuid() } {
        Ok(g) => format!(
            "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            g.data1,
            g.data2,
            g.data3,
            g.data4[0],
            g.data4[1],
            g.data4[2],
            g.data4[3],
            g.data4[4],
            g.data4[5],
            g.data4[6],
            g.data4[7]
        ),
        Err(_) => format!("{}", crate::platform::clock::unix_now()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tinycast_pure::search_relevance::{score, Band, BAND_STRIDE};
    use tinycast_pure::snippet::SnippetSourceRevision;

    fn record() -> StoredSnippet {
        StoredSnippet {
            path: PathBuf::from("Meeting Notes.md"),
            name: "Meeting Notes".into(),
            keyword: Some("!notes".into()),
            enabled: true,
            show_confirmation: false,
            body: "Hello {clipboard}".into(),
            source_revision: SnippetSourceRevision::new(""),
        }
    }

    #[test]
    fn snippet_keyword_is_display_literal_band() {
        let f = snippet_entry(&record()).fields;
        assert!(score("!notes", &f).unwrap() >= Band::DisplayLiteral as i32 * BAND_STRIDE);
    }

    #[test]
    fn argument_session_backspace_restores_previous() {
        let mut session = ArgumentSession {
            kind: ArgumentKind::Snippet,
            snippet_path: "a.md".into(),
            snippet_name: "A".into(),
            show_confirmation: false,
            specs: vec![
                ArgumentSpec {
                    name: "First".into(),
                    options: vec![],
                },
                ArgumentSpec {
                    name: "Second".into(),
                    options: vec![],
                },
            ],
            values: HashMap::new(),
            index: 0,
            clipboard: vec![],
            selection: None,
            now: 0,
            locale: "en".into(),
            tz: "UTC".into(),
            keyword: None,
            target: 0,
        };
        assert!(!session.commit("one".into()));
        assert_eq!(session.back().as_deref(), Some("one"));
        assert_eq!(session.index, 0);
    }

    #[test]
    fn expansion_context_forwards_selection() {
        let ctx = expansion_context(vec!["clip".into()], Some("sel".into()), 1, "en".into());
        assert_eq!(ctx.selection.as_deref(), Some("sel"));
        assert_eq!(ctx.clipboard, ["clip"]);
    }

    #[test]
    fn keyword_does_not_capture_when_template_omits_selection() {
        let mut captured = false;
        let got = capture_if_uses_selection("hello {clipboard}", || {
            captured = true;
            Some("would-ctrl-c".into())
        });
        assert!(!captured);
        assert_eq!(got, None);
    }

    #[test]
    fn keyword_captures_when_template_uses_selection_or_selected_text() {
        let mut captured = 0usize;
        let sel = capture_if_uses_selection("x {selection} y", || {
            captured += 1;
            Some("sel".into())
        });
        let text = capture_if_uses_selection("{selectedText}", || {
            captured += 1;
            Some("sel".into())
        });
        assert_eq!(captured, 2);
        assert_eq!(sel.as_deref(), Some("sel"));
        assert_eq!(text.as_deref(), Some("sel"));
    }

    #[test]
    fn keyword_argument_session_keeps_keyword_and_target() {
        let session = ArgumentSession {
            kind: ArgumentKind::Snippet,
            snippet_path: "a.md".into(),
            snippet_name: "A".into(),
            show_confirmation: false,
            specs: vec![],
            values: HashMap::new(),
            index: 0,
            clipboard: vec![],
            selection: Some("sel".into()),
            now: 0,
            locale: "en".into(),
            tz: "UTC".into(),
            keyword: Some("!notes".into()),
            target: 42,
        };
        assert_eq!(session.keyword.as_deref(), Some("!notes"));
        assert_eq!(session.target, 42);
        assert_eq!(session.selection.as_deref(), Some("sel"));
    }
}
