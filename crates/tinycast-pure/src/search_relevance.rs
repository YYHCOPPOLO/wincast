pub mod fuzzy {
    pub struct FuzzyMatch;

    impl FuzzyMatch {
        pub const MAXIMUM_SCORE: i32 = 100_000;

        /// Returns None if no match. Higher is better. Exact > prefix > word-start/substring > subsequence.
        pub fn score(query: &str, target: &str) -> Option<i32> {
            let q = normalize(query);
            let t = normalize(target);
            if q.chars.is_empty() {
                return None;
            }
            if q.chars == t.chars {
                return Some(Self::MAXIMUM_SCORE);
            }
            if t.chars.len() < q.chars.len() {
                return None;
            }
            if t.chars.starts_with(&q.chars) {
                return Some(contiguous_score(
                    PREFIX_FLOOR,
                    q.chars.len(),
                    t.chars.len(),
                    0,
                ));
            }
            if let Some(at) = find_at_word_start(&q.chars, &t.chars, &t.word_start) {
                return Some(contiguous_score(
                    WORD_START_FLOOR,
                    q.chars.len(),
                    t.chars.len(),
                    at,
                ));
            }
            if let Some(at) = find_contiguous(&q.chars, &t.chars) {
                return Some(contiguous_score(
                    SUBSTRING_FLOOR,
                    q.chars.len(),
                    t.chars.len(),
                    at,
                ));
            }
            subsequence_score(&q.chars, &t.chars, &t.word_start)
        }
    }

    const PREFIX_FLOOR: i32 = 80_000;
    const WORD_START_FLOOR: i32 = 60_000;
    const SUBSTRING_FLOOR: i32 = 40_000;
    const SUBSEQUENCE_CEILING: i32 = 39_999;

    struct Normalized {
        chars: Vec<char>,
        word_start: Vec<bool>,
    }

    fn normalize(s: &str) -> Normalized {
        let mut chars = Vec::new();
        let mut word_start = Vec::new();
        let mut prev_visible: Option<char> = None;
        for ch in s.chars().filter(|c| !is_invisible_format(*c)) {
            let boundary = match prev_visible {
                None => true,
                Some(prev) => is_word_boundary(prev, ch),
            };
            let mut first = true;
            for lc in ch.to_lowercase() {
                chars.push(lc);
                word_start.push(first && boundary);
                first = false;
            }
            prev_visible = Some(ch);
        }
        Normalized { chars, word_start }
    }

    /// Unicode General Category Format (Cf): bidi marks, ZWSP, BOM, tags, …
    fn is_invisible_format(c: char) -> bool {
        matches!(
            c,
            '\u{00AD}'
                | '\u{0600}'..='\u{0605}'
                | '\u{061C}'
                | '\u{06DD}'
                | '\u{070F}'
                | '\u{0890}'..='\u{0891}'
                | '\u{08E2}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{110BD}'
                | '\u{110CD}'
                | '\u{13430}'..='\u{1343F}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0001}'
                | '\u{E0020}'..='\u{E007F}'
        )
    }

    fn is_word_boundary(prev: char, cur: char) -> bool {
        if !prev.is_alphanumeric() && cur.is_alphanumeric() {
            return true;
        }
        if prev.is_lowercase() && cur.is_uppercase() {
            return true;
        }
        prev.is_alphanumeric() && cur.is_alphanumeric() && prev.is_numeric() != cur.is_numeric()
    }

    fn contiguous_score(floor: i32, query_len: usize, target_len: usize, start: usize) -> i32 {
        let tightness = (query_len as i32).saturating_mul(9_999) / (target_len as i32).max(1);
        let pos_pen = (start as i32).min(1_999);
        floor
            .saturating_add(tightness)
            .saturating_sub(pos_pen)
            .max(floor)
            .min(FuzzyMatch::MAXIMUM_SCORE - 1)
    }

    fn find_at_word_start(query: &[char], target: &[char], word_start: &[bool]) -> Option<usize> {
        let n = query.len();
        if n == 0 || n > target.len() {
            return None;
        }
        (0..=target.len() - n).find(|&i| word_start[i] && target[i..i + n] == *query)
    }

    fn find_contiguous(query: &[char], target: &[char]) -> Option<usize> {
        target
            .windows(query.len())
            .position(|window| window == query)
    }

    fn subsequence_score(query: &[char], target: &[char], word_start: &[bool]) -> Option<i32> {
        let qn = query.len();
        if qn == 0 || qn > target.len() {
            return None;
        }

        let mut best_score = vec![i32::MIN; qn + 1];
        let mut best_last = vec![0usize; qn + 1];
        best_score[0] = 0;

        for ti in 0..target.len() {
            for j in (0..qn).rev() {
                if query[j] != target[ti] || best_score[j] == i32::MIN {
                    continue;
                }
                let mut s = best_score[j].saturating_add(16);
                if word_start[ti] {
                    s = s.saturating_add(12);
                }
                if j > 0 {
                    let prev_i = best_last[j];
                    if ti == prev_i + 1 {
                        s = s.saturating_add(8);
                    } else if ti > prev_i {
                        s = s.saturating_sub(((ti - prev_i - 1) as i32).min(8));
                    }
                }
                if s > best_score[j + 1] {
                    best_score[j + 1] = s;
                    best_last[j + 1] = ti;
                }
            }
        }

        if best_score[qn] == i32::MIN {
            None
        } else {
            Some(best_score[qn].clamp(1, SUBSEQUENCE_CEILING))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::FuzzyMatch;

        #[test]
        fn exact_beats_prefix_beats_subsequence() {
            let q = "code";
            let exact = FuzzyMatch::score(q, "code").unwrap();
            let prefix = FuzzyMatch::score(q, "codeberg").unwrap();
            let sub = FuzzyMatch::score(q, "xcxoxdxe").unwrap();
            assert!(exact > prefix && prefix > sub);
            assert!(exact <= FuzzyMatch::MAXIMUM_SCORE);
        }

        #[test]
        fn no_match_is_none() {
            assert_eq!(FuzzyMatch::score("zzz", "abc"), None);
        }

        #[test]
        fn non_exact_stays_below_maximum() {
            let prefix = FuzzyMatch::score("code", "codeberg").unwrap();
            let word = FuzzyMatch::score("code", "VS Code").unwrap();
            let sub_str = FuzzyMatch::score("code", "encode").unwrap();
            let sub_seq = FuzzyMatch::score("code", "xcxoxdxe").unwrap();
            assert!(prefix < FuzzyMatch::MAXIMUM_SCORE);
            assert!(word < FuzzyMatch::MAXIMUM_SCORE);
            assert!(sub_str < FuzzyMatch::MAXIMUM_SCORE);
            assert!(sub_seq < FuzzyMatch::MAXIMUM_SCORE);
            assert!(prefix > word && word > sub_str && sub_str > sub_seq);
        }

        #[test]
        fn strips_invisible_format_scalars() {
            let marked = "\u{200B}code\u{200E}";
            assert_eq!(
                FuzzyMatch::score("code", marked),
                Some(FuzzyMatch::MAXIMUM_SCORE)
            );
            assert_eq!(
                FuzzyMatch::score("\u{FEFF}CoDe", "code"),
                Some(FuzzyMatch::MAXIMUM_SCORE)
            );
        }

        #[test]
        fn unicode_lowercase_is_case_insensitive() {
            assert_eq!(
                FuzzyMatch::score("Code", "CODE"),
                Some(FuzzyMatch::MAXIMUM_SCORE)
            );
            assert!(
                FuzzyMatch::score("å", "Ångström").unwrap()
                    > FuzzyMatch::score("ng", "Ångström").unwrap()
            );
        }
    }
}
