pub fn guarded(enabled: bool) -> bool {
    enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_commands_noop_when_disabled() {
        assert!(!guarded(false));
        assert!(guarded(true));
    }
}
