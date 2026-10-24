pub fn volume_readout(level: f32, muted: bool) -> String {
    if muted {
        "Muted".into()
    } else {
        tinycast_pure::volume::percentage(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::theme;

    #[test]
    fn muted_prints_word_not_percent() {
        assert_eq!(volume_readout(0.0, true), "Muted");
        assert_ne!(volume_readout(0.0, true), "0%");
        assert_eq!(volume_readout(0.4, false), "40%");
    }

    #[test]
    fn hud_timers_split() {
        assert_eq!(theme::duration::MESSAGE_HUD_SECS, 2.4);
        assert_eq!(theme::duration::VOLUME_HUD_SECS, 1.6);
    }
}
