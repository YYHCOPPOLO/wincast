/// SF Symbol name → Segoe Fluent / MDL2 codepoint.
pub fn fluent_for_sf(sf: &str) -> Option<&'static str> {
    Some(match sf {
        "magnifyingglass" => "\u{E721}",
        "chevron.left" => "\u{E76B}",
        "arrow.right" => "\u{E72A}",
        "exclamationmark.triangle" => "\u{E7BA}",
        "switch.2" => "\u{E8BF}",
        "square.grid.2x2" => "\u{E80A}",
        "gearshape" => "\u{E713}",
        "bolt" => "\u{E945}",
        "terminal" => "\u{E756}",
        "link" => "\u{E71B}",
        "sparkles" => "\u{EBD4}",
        "wand.and.sparkles" => "\u{E90F}",
        "doc.text.magnifyingglass" => "\u{EA51}",
        "text.page" => "\u{E8A5}",
        "curlybraces" => "\u{E943}",
        "macwindow" => "\u{E737}",
        "doc.on.clipboard" => "\u{E8C8}",
        "face.smiling" => "\u{E76E}",
        "calendar" => "\u{E787}",
        "puzzlepiece.extension" => "\u{EA86}",
        "lock.shield" => "\u{EA18}",
        "arrow.up.arrow.down.circle" => "\u{E895}",
        "info.circle" => "\u{E946}",
        "info.circle.fill" => "\u{EA1F}",
        "speaker.wave.2" => "\u{E767}",
        "speaker.slash.fill" => "\u{E74F}",
        "checkmark.circle.fill" => "\u{E73E}",
        "exclamationmark.circle.fill" => "\u{EA39}",
        "plus" => "\u{E710}",
        "folder" => "\u{E8B7}",
        "lock.fill" => "\u{E72E}",
        "rectangle.stack" => "\u{E8B1}",
        "heart" => "\u{EB51}",
        "checkmark" => "\u{E73E}",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnifying_glass_maps() {
        assert!(fluent_for_sf("magnifyingglass").is_some());
        assert!(fluent_for_sf("chevron.left").is_some());
        assert!(fluent_for_sf("arrow.right").is_some());
        assert!(fluent_for_sf("exclamationmark.triangle").is_some());
    }
}
