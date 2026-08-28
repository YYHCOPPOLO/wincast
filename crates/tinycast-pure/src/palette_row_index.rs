/// Flat selection: headers consume no index. `card` occupies index 0 when present.
pub fn selectable_count(card: bool, rows: usize) -> usize {
    rows + usize::from(card)
}

pub fn clamp_selection(selection: usize, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        selection.min(count - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_occupies_index_zero() {
        assert_eq!(selectable_count(true, 3), 4);
        assert_eq!(clamp_selection(10, 4), 3);
    }
}
