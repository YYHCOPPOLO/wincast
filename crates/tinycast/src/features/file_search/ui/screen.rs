use tinycast_pure::file_search::FileSearchHit;

use crate::features::file_search::service::session::State;
use crate::features::launcher::ui::list::PaintItem;

pub fn paint_items(
    state: State,
    query: &str,
    results: &[FileSearchHit],
    selection: usize,
) -> Vec<PaintItem> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    match state {
        State::Idle => Vec::new(),
        State::Searching if results.is_empty() => vec![PaintItem::Header {
            title: "Searching files…".into(),
        }],
        State::Failed => vec![PaintItem::Header {
            title: "File search is unavailable".into(),
        }],
        State::Ready if results.is_empty() => vec![PaintItem::Header {
            title: "No files found".into(),
        }],
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
    fn in_flight_and_empty_copy() {
        match &paint_items(State::Searching, "a", &[], 0)[0] {
            PaintItem::Header { title } => assert_eq!(title, "Searching files…"),
            _ => panic!("expected header"),
        }
        match &paint_items(State::Ready, "a", &[], 0)[0] {
            PaintItem::Header { title } => assert_eq!(title, "No files found"),
            _ => panic!("expected header"),
        }
        match &paint_items(State::Failed, "a", &[], 0)[0] {
            PaintItem::Header { title } => assert_eq!(title, "File search is unavailable"),
            _ => panic!("expected header"),
        }
    }
}
