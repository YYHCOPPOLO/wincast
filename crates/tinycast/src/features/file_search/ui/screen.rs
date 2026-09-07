use tinycast_pure::file_search::FileSearchHit;

use crate::features::file_search::service::session::State;
use crate::features::launcher::ui::list::PaintItem;

pub fn empty_message(state: State, query: &str) -> &'static str {
    empty_message_lang(state, query, tinycast_pure::i18n::UiLang::En)
}

pub fn empty_message_lang(
    state: State,
    query: &str,
    lang: tinycast_pure::i18n::UiLang,
) -> &'static str {
    if query.trim().is_empty() {
        return tinycast_pure::i18n::file_search_empty("type", lang);
    }
    let key = match state {
        State::Failed => "unavailable",
        State::Ready => "none",
        State::Searching | State::Idle => "searching",
    };
    tinycast_pure::i18n::file_search_empty(key, lang)
}

pub fn paint_items(
    state: State,
    query: &str,
    results: &[FileSearchHit],
    selection: usize,
) -> Vec<PaintItem> {
    if query.trim().is_empty() || results.is_empty() || state == State::Failed {
        return Vec::new();
    }
    match state {
        State::Searching | State::Ready => results
            .iter()
            .enumerate()
            .map(|(i, hit)| PaintItem::Row {
                title: hit.name.clone(),
                alias: None,
                trailing: hit.parent.clone(),
                keycap: None,
                icon_source: Some(hit.path.replace('/', "\\")),
                selected: i == selection,
            })
            .collect(),
        State::Idle | State::Failed => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_paints_nothing() {
        assert!(paint_items(State::Idle, "", &[], 0).is_empty());
        assert!(paint_items(State::Ready, "  ", &[], 0).is_empty());
    }

    #[test]
    fn empty_states_paint_no_items() {
        assert!(paint_items(State::Searching, "a", &[], 0).is_empty());
        assert!(paint_items(State::Ready, "a", &[], 0).is_empty());
        assert!(paint_items(State::Failed, "a", &[], 0).is_empty());
        assert!(paint_items(State::Idle, "a", &[], 0).is_empty());
    }

    #[test]
    fn empty_message_matches_oracle() {
        assert_eq!(
            empty_message(State::Idle, ""),
            "Type to search files and folders"
        );
        assert_eq!(empty_message(State::Searching, "a"), "Searching files…");
        assert_eq!(empty_message(State::Ready, "a"), "No files found");
        assert_eq!(
            empty_message(State::Failed, "a"),
            "File search is unavailable"
        );
    }
}
