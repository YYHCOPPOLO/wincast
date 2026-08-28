use std::collections::HashSet;

use crate::app_entry::{AppEntry, AppKind};
use crate::favorites::FavoritesStore;
use crate::launcher_ranking::LauncherRankingStore;
use crate::search_relevance::score;
use crate::visibility::VisibilityStore;

/// Empty-query publication order. Favorites is a pin, not a kind.
const KIND_ORDER: [AppKind; 8] = [
    AppKind::Application,
    AppKind::SystemSettings,
    AppKind::Quicklink,
    AppKind::Snippet,
    AppKind::SystemAction,
    AppKind::WindowCommand,
    AppKind::CustomCommand,
    AppKind::Command,
];

/// Section identity. Favorites / Results are not `AppKind` cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LauncherSectionKind {
    Favorites,
    Results,
    Kind(AppKind),
}

#[derive(Clone, Debug)]
pub struct LauncherSection {
    pub kind: LauncherSectionKind,
    pub rows: Vec<AppEntry>,
}

impl LauncherSectionKind {
    pub fn title(self) -> &'static str {
        match self {
            LauncherSectionKind::Favorites => "Favorites",
            LauncherSectionKind::Results => "Results",
            LauncherSectionKind::Kind(kind) => kind.section_title(),
        }
    }
}

/// Flattened list: section headers then rows. `Results` has no header.
#[derive(Clone, Debug)]
pub enum LauncherListItem<'a> {
    Header(LauncherSectionKind),
    Row(&'a AppEntry),
}

pub fn list_items(sections: &[LauncherSection]) -> Vec<LauncherListItem<'_>> {
    let mut out = Vec::new();
    for section in sections {
        if section.kind != LauncherSectionKind::Results {
            out.push(LauncherListItem::Header(section.kind));
        }
        for row in &section.rows {
            out.push(LauncherListItem::Row(row));
        }
    }
    out
}

/// Headers consume no selection index.
pub fn selectable_rows(sections: &[LauncherSection]) -> Vec<&AppEntry> {
    sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .collect()
}

pub fn is_category_listing(query: &str) -> bool {
    AppKind::named_by(query).is_some()
}

pub fn ordered_results(
    entries: &[AppEntry],
    query: &str,
    now: i64,
    ranking: &LauncherRankingStore,
    visibility: &VisibilityStore,
    favorites: &FavoritesStore,
    show_sections: bool,
) -> Vec<LauncherSection> {
    let visible: Vec<&AppEntry> = entries
        .iter()
        .filter(|entry| is_shown(entry, visibility))
        .collect();

    if query.is_empty() {
        return empty_query(&visible, favorites, show_sections);
    }
    if let Some(kind) = AppKind::named_by(query) {
        return category_listing(&visible, kind, query, show_sections);
    }
    ranked(&visible, query, now, ranking)
}

fn is_shown(entry: &AppEntry, visibility: &VisibilityStore) -> bool {
    visibility.is_kind_enabled(entry.kind) && visibility.is_item_visible(&entry.id)
}

fn empty_query(
    visible: &[&AppEntry],
    favorites: &FavoritesStore,
    show_sections: bool,
) -> Vec<LauncherSection> {
    let favorite_rows = pinned_favorites(visible, favorites);
    let pinned: HashSet<&str> = favorite_rows.iter().map(|e| e.id.as_str()).collect();
    let rest: Vec<&AppEntry> = visible
        .iter()
        .copied()
        .filter(|entry| !pinned.contains(entry.id.as_str()))
        .collect();

    if show_sections {
        let mut sections = Vec::new();
        if !favorite_rows.is_empty() {
            sections.push(LauncherSection {
                kind: LauncherSectionKind::Favorites,
                rows: favorite_rows,
            });
        }
        sections.extend(kind_sections(&rest));
        sections
    } else {
        let mut rows = favorite_rows;
        for kind in KIND_ORDER {
            rows.extend(
                rest.iter()
                    .filter(|entry| entry.kind == kind)
                    .map(|entry| (*entry).clone()),
            );
        }
        results_section(rows)
    }
}

fn category_listing(
    visible: &[&AppEntry],
    named_kind: AppKind,
    query: &str,
    show_sections: bool,
) -> Vec<LauncherSection> {
    let included: Vec<&AppEntry> = visible
        .iter()
        .copied()
        .filter(|entry| entry.kind == named_kind || entry.fields.display_name == query)
        .collect();
    if show_sections {
        kind_sections(&included)
    } else {
        let mut rows = Vec::new();
        for kind in KIND_ORDER {
            rows.extend(
                included
                    .iter()
                    .filter(|entry| entry.kind == kind)
                    .map(|entry| (*entry).clone()),
            );
        }
        results_section(rows)
    }
}

fn ranked(
    visible: &[&AppEntry],
    query: &str,
    now: i64,
    ranking: &LauncherRankingStore,
) -> Vec<LauncherSection> {
    let mut hits: Vec<(i32, &AppEntry)> = visible
        .iter()
        .copied()
        .filter_map(|entry| {
            let base = score(query, &entry.fields)?;
            let boosted = base.saturating_add(ranking.boost(query, &entry.id, now));
            Some((boosted, entry))
        })
        .collect();
    hits.sort_by(|(score_a, a), (score_b, b)| {
        score_b
            .cmp(score_a)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    results_section(hits.into_iter().map(|(_, entry)| entry.clone()).collect())
}

fn pinned_favorites(visible: &[&AppEntry], favorites: &FavoritesStore) -> Vec<AppEntry> {
    favorites
        .ids
        .iter()
        .filter_map(|id| {
            visible
                .iter()
                .copied()
                .find(|entry| entry.id == *id)
                .cloned()
        })
        .collect()
}

fn kind_sections(entries: &[&AppEntry]) -> Vec<LauncherSection> {
    KIND_ORDER
        .into_iter()
        .filter_map(|kind| {
            let rows: Vec<AppEntry> = entries
                .iter()
                .filter(|entry| entry.kind == kind)
                .map(|entry| (*entry).clone())
                .collect();
            if rows.is_empty() {
                None
            } else {
                Some(LauncherSection {
                    kind: LauncherSectionKind::Kind(kind),
                    rows,
                })
            }
        })
        .collect()
}

fn results_section(rows: Vec<AppEntry>) -> Vec<LauncherSection> {
    if rows.is_empty() {
        Vec::new()
    } else {
        vec![LauncherSection {
            kind: LauncherSectionKind::Results,
            rows,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_category_listing, list_items, ordered_results, selectable_rows, LauncherListItem,
        LauncherSection, LauncherSectionKind,
    };
    use crate::app_entry::{AppEntry, AppKind};
    use crate::favorites::FavoritesStore;
    use crate::launcher_ranking::LauncherRankingStore;
    use crate::search_relevance::{score, SearchFields};
    use crate::visibility::VisibilityStore;
    use std::path::PathBuf;

    fn entry(id: &str, kind: AppKind, name: &str) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            kind,
            name: name.to_string(),
            fields: SearchFields {
                display_name: name.to_string(),
                ..Default::default()
            },
            hotkey: None,
        }
    }

    fn empty_rank() -> LauncherRankingStore {
        LauncherRankingStore::load(PathBuf::from(
            "Z:\\tinycast-does-not-exist\\launcher-ranking.json",
        ))
    }

    fn kinds(sections: &[LauncherSection]) -> Vec<LauncherSectionKind> {
        sections.iter().map(|s| s.kind).collect()
    }

    fn ids(section: &LauncherSection) -> Vec<&str> {
        section.rows.iter().map(|r| r.id.as_str()).collect()
    }

    fn all_ids(sections: &[LauncherSection]) -> Vec<&str> {
        sections
            .iter()
            .flat_map(|s| s.rows.iter().map(|r| r.id.as_str()))
            .collect()
    }

    #[test]
    fn empty_query_section_order() {
        let entries = [
            entry("app:notepad", AppKind::Application, "Notepad"),
            entry("app:calc", AppKind::Application, "Calculator"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&entries, "", 0, &rank, &vis, &fav, true);
        let kinds: Vec<_> = sections.iter().map(|s| s.kind).collect();
        assert_eq!(
            &kinds[..2],
            &[
                LauncherSectionKind::Kind(AppKind::Application),
                LauncherSectionKind::Kind(AppKind::Command),
            ]
        );
        assert_eq!(ids(&sections[0]), ["app:notepad", "app:calc"]);
        assert_eq!(ids(&sections[1]), ["command:quit"]);
    }

    #[test]
    fn weaker_band_cannot_outrank_with_boost() {
        let display_exact = score(
            "Code",
            &SearchFields {
                display_name: "Code".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let alt = SearchFields {
            display_name: "Other".into(),
            alternate_names: vec!["codeberg helper".into()],
            ..Default::default()
        };
        let alt_score = score("code", &alt).unwrap() + LauncherRankingStore::MAXIMUM_BOOST;
        assert!(display_exact > alt_score);
    }

    #[test]
    fn empty_query_kind_order_not_input_order() {
        let entries = [
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
            entry("settings:display", AppKind::SystemSettings, "Display"),
            entry("app:notepad", AppKind::Application, "Notepad"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&entries, "", 0, &rank, &vis, &fav, true);
        assert_eq!(
            kinds(&sections),
            [
                LauncherSectionKind::Kind(AppKind::Application),
                LauncherSectionKind::Kind(AppKind::SystemSettings),
                LauncherSectionKind::Kind(AppKind::Command),
            ]
        );
        assert_eq!(ids(&sections[0]), ["app:notepad"]);
        assert_eq!(ids(&sections[1]), ["settings:display"]);
        assert_eq!(ids(&sections[2]), ["command:quit"]);
    }

    #[test]
    fn empty_query_pins_favorites_prefix_without_duplicating() {
        let entries = [
            entry("app:notepad", AppKind::Application, "Notepad"),
            entry("app:calc", AppKind::Application, "Calculator"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
            entry("command:about", AppKind::Command, "About Tinycast"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let mut fav = FavoritesStore::default();
        fav.toggle("command:quit".into());
        fav.toggle("app:calc".into());
        let sections = ordered_results(&entries, "", 0, &rank, &vis, &fav, true);
        assert_eq!(
            kinds(&sections),
            [
                LauncherSectionKind::Favorites,
                LauncherSectionKind::Kind(AppKind::Application),
                LauncherSectionKind::Kind(AppKind::Command),
            ]
        );
        assert_eq!(ids(&sections[0]), ["command:quit", "app:calc"]);
        assert_eq!(ids(&sections[1]), ["app:notepad"]);
        assert_eq!(ids(&sections[2]), ["command:about"]);
    }

    #[test]
    fn hidden_item_and_disabled_kind_are_dropped() {
        let entries = [
            entry("app:notepad", AppKind::Application, "Notepad"),
            entry("app:calc", AppKind::Application, "Calculator"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
            entry("settings:display", AppKind::SystemSettings, "Display"),
        ];
        let rank = empty_rank();
        let mut vis = VisibilityStore::default();
        vis.hide_item("app:calc");
        vis.set_kind_enabled(AppKind::Command, false);
        let mut fav = FavoritesStore::default();
        fav.toggle("app:calc".into());
        fav.toggle("command:quit".into());
        let sections = ordered_results(&entries, "", 0, &rank, &vis, &fav, true);
        assert_eq!(
            kinds(&sections),
            [
                LauncherSectionKind::Kind(AppKind::Application),
                LauncherSectionKind::Kind(AppKind::SystemSettings),
            ]
        );
        assert_eq!(all_ids(&sections), ["app:notepad", "settings:display"]);
    }

    #[test]
    fn non_empty_query_ranks_and_drops_misses() {
        let entries = [
            entry("app:codeberg", AppKind::Application, "Codeberg"),
            entry("app:code", AppKind::Application, "Code"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&entries, "code", 0, &rank, &vis, &fav, false);
        assert_eq!(kinds(&sections), [LauncherSectionKind::Results]);
        assert_eq!(ids(&sections[0]), ["app:code", "app:codeberg"]);
    }

    #[test]
    fn non_empty_query_does_not_pin_favorites() {
        let entries = [
            entry("app:codeberg", AppKind::Application, "Codeberg"),
            entry("app:code", AppKind::Application, "Code"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let mut fav = FavoritesStore::default();
        fav.toggle("app:codeberg".into());
        let sections = ordered_results(&entries, "code", 0, &rank, &vis, &fav, false);
        assert_eq!(ids(&sections[0]), ["app:code", "app:codeberg"]);
        assert_ne!(sections[0].kind, LauncherSectionKind::Favorites);
    }

    #[test]
    fn category_listing_is_unsorted_index_order() {
        let entries = [
            entry("app:zeta", AppKind::Application, "Zeta"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
            entry("app:alpha", AppKind::Application, "Alpha"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let mut fav = FavoritesStore::default();
        fav.toggle("app:alpha".into());
        assert!(is_category_listing("Applications"));
        let sections = ordered_results(&entries, "Applications", 0, &rank, &vis, &fav, true);
        assert_eq!(
            kinds(&sections),
            [LauncherSectionKind::Kind(AppKind::Application)]
        );
        assert_eq!(ids(&sections[0]), ["app:zeta", "app:alpha"]);
    }

    #[test]
    fn category_listing_includes_display_name_collision() {
        let entries = [
            entry("settings:display", AppKind::SystemSettings, "Display"),
            entry(
                "app:system-settings",
                AppKind::Application,
                "System Settings",
            ),
            entry("settings:bluetooth", AppKind::SystemSettings, "Bluetooth"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&entries, "System Settings", 0, &rank, &vis, &fav, true);
        assert_eq!(
            kinds(&sections),
            [
                LauncherSectionKind::Kind(AppKind::Application),
                LauncherSectionKind::Kind(AppKind::SystemSettings),
            ]
        );
        assert_eq!(ids(&sections[0]), ["app:system-settings"]);
        assert_eq!(
            ids(&sections[1]),
            ["settings:display", "settings:bluetooth"]
        );
    }

    #[test]
    fn weaker_band_stays_behind_in_ordered_results() {
        let code = entry("app:code", AppKind::Application, "Code");
        let other = AppEntry {
            id: "app:other".into(),
            kind: AppKind::Application,
            name: "Other".into(),
            fields: SearchFields {
                display_name: "Other".into(),
                alternate_names: vec!["codeberg helper".into()],
                ..Default::default()
            },
            hotkey: None,
        };
        let mut rank = empty_rank();
        let now = 2_000_000_000;
        for _ in 0..200 {
            rank.record("code", "app:other", now);
        }
        assert_eq!(
            rank.boost("code", "app:other", now),
            LauncherRankingStore::MAXIMUM_BOOST
        );
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&[code, other], "code", now, &rank, &vis, &fav, false);
        assert_eq!(ids(&sections[0]), ["app:code", "app:other"]);
    }

    #[test]
    fn headers_are_not_selectable() {
        let entries = [
            entry("app:notepad", AppKind::Application, "Notepad"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
        ];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&entries, "", 0, &rank, &vis, &fav, true);
        let items = list_items(&sections);
        assert!(matches!(
            items[0],
            LauncherListItem::Header(LauncherSectionKind::Kind(AppKind::Application))
        ));
        let rows = selectable_rows(&sections);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "app:notepad");
        assert_eq!(rows[1].id, "command:quit");
        assert_eq!(LauncherSectionKind::Favorites.title(), "Favorites");
        assert_eq!(
            LauncherSectionKind::Kind(AppKind::SystemSettings).title(),
            "System Settings"
        );
    }

    #[test]
    fn ranked_results_have_no_header() {
        let entries = [entry("app:notepad", AppKind::Application, "Notepad")];
        let rank = empty_rank();
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&entries, "not", 0, &rank, &vis, &fav, false);
        let items = list_items(&sections);
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], LauncherListItem::Row(_)));
    }
}
