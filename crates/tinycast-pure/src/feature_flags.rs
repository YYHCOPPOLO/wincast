/// Feature master switches. Absent JSON keys deserialize as `false`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FeatureFlags {
    pub file_search_enabled: bool,
    pub notes_enabled: bool,
    pub snippets_enabled: bool,
    pub window_management_enabled: bool,
    pub calendar_enabled: bool,
    pub ai_enabled: bool,
    pub quick_actions_enabled: bool,
    pub extensions_enabled: bool,
    pub quicklinks_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::FeatureFlags;

    #[test]
    fn feature_flags_default_off() {
        let f = FeatureFlags::default();
        assert!(!f.file_search_enabled);
        assert!(!f.notes_enabled);
        assert!(!f.snippets_enabled);
        assert!(!f.window_management_enabled);
        assert!(!f.calendar_enabled);
        assert!(!f.ai_enabled);
        assert!(!f.quick_actions_enabled);
        assert!(!f.extensions_enabled);
        assert!(!f.quicklinks_enabled);
    }
}
