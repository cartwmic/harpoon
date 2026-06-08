//! Match algorithms for filter mode.
//!
//! Two backends, switched at config parse time:
//! - [`FuzzyMatcher`] wraps `nucleo-matcher` for fuzzy + char-index extraction.
//! - [`SubstringMatcher`] is a case-insensitive ASCII-fold substring scan.
//!
//! Both return `(score, char_indices)` where indices are **character (Unicode
//! scalar) positions**, never byte offsets. The render layer is responsible
//! for any char→byte conversion at the FFI boundary if Phase 0.5 verification
//! discovers the host indexes by byte.
//!
//! `MatcherImpl` is a static-dispatch enum (not `Box<dyn Matcher>`) — see
//! `design.md` "Decision: Static-dispatch matcher".

use nucleo_matcher::{Config, Matcher as NucleoMatcher, Utf32Str};

use crate::config::{Config as HarpoonConfig, MatcherKind};

/// Output of a match attempt: score (higher = better) plus the **character
/// indices** in the haystack that contributed to the match. Indices are
/// sorted ascending. Empty `Vec` for empty needle (any haystack matches with
/// no contributing characters).
pub type MatchResult = (i32, Vec<usize>);

/// Matcher trait. Kept for documentation/clarity; storage is via
/// [`MatcherImpl`] (static dispatch, not `Box<dyn>`).
pub trait Matcher {
    /// Match `needle` against `haystack`. Returns `Some((score, char_indices))`
    /// on match, `None` on miss. Empty needle matches any haystack with score
    /// `i32::MAX` and an empty index list.
    ///
    /// `&mut self` is required to accommodate `nucleo::Matcher`'s internal
    /// scratch buffers; the substring matcher is stateless but uses the same
    /// signature for uniformity.
    fn match_indices(&mut self, haystack: &str, needle: &str) -> Option<MatchResult>;
}

/// Fuzzy matcher backed by `nucleo-matcher`.
///
/// Holds the matcher's internal scratch state across calls for amortized
/// allocation cost.
pub struct FuzzyMatcher {
    inner: NucleoMatcher,
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self {
            inner: NucleoMatcher::new(Config::DEFAULT.match_paths()),
        }
    }
}

impl FuzzyMatcher {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Matcher for FuzzyMatcher {
    fn match_indices(&mut self, haystack: &str, needle: &str) -> Option<MatchResult> {
        if needle.is_empty() {
            return Some((i32::MAX, Vec::new()));
        }

        let mut hay_buf = Vec::new();
        let mut ndl_buf = Vec::new();
        let hay = Utf32Str::new(haystack, &mut hay_buf);
        let ndl = Utf32Str::new(needle, &mut ndl_buf);

        let mut indices: Vec<u32> = Vec::new();
        let score = self.inner.fuzzy_indices(hay, ndl, &mut indices)?;
        // nucleo returns u32 char indices already; convert and sort (nucleo
        // emits them in match order, which may not be ascending).
        let mut idx: Vec<usize> = indices.into_iter().map(|i| i as usize).collect();
        idx.sort_unstable();
        Some((score as i32, idx))
    }
}

/// Case-insensitive ASCII-fold substring matcher.
///
/// The score is the negative of the match start position so earlier matches
/// rank higher. Indices are the contiguous range `[start, start + needle.chars().count())`
/// in **character** positions.
#[derive(Debug, Default)]
pub struct SubstringMatcher;

impl SubstringMatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Matcher for SubstringMatcher {
    fn match_indices(&mut self, haystack: &str, needle: &str) -> Option<MatchResult> {
        if needle.is_empty() {
            return Some((i32::MAX, Vec::new()));
        }

        // Walk haystack chars; at each position check whether needle.chars()
        // matches starting here, case-insensitively (ASCII fold).
        let hay_chars: Vec<char> = haystack.chars().collect();
        let ndl_chars: Vec<char> = needle.chars().collect();
        if ndl_chars.len() > hay_chars.len() {
            return None;
        }

        for start in 0..=(hay_chars.len() - ndl_chars.len()) {
            let mut all_match = true;
            for (offset, &nc) in ndl_chars.iter().enumerate() {
                let hc = hay_chars[start + offset];
                if !chars_eq_ascii_ci(hc, nc) {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                // Score: negate start so earlier matches rank higher; cap at i32.
                let score = -(start as i32);
                let indices: Vec<usize> = (start..start + ndl_chars.len()).collect();
                return Some((score, indices));
            }
        }
        None
    }
}

/// Case-insensitive char comparison with ASCII fold. Non-ASCII chars compare
/// strictly (case-folding non-ASCII Unicode would require the full
/// `unicode-case` tables and isn't worth the dep for substring fallback).
fn chars_eq_ascii_ci(a: char, b: char) -> bool {
    if a.is_ascii() || b.is_ascii() {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}

/// Static-dispatch matcher implementation. Stored on `State`; constructed at
/// load-time from `Config`. Avoids the `Default` headache and heap allocation
/// of `Box<dyn Matcher>`.
pub enum MatcherImpl {
    Fuzzy(FuzzyMatcher),
    Substring(SubstringMatcher),
}

impl Default for MatcherImpl {
    fn default() -> Self {
        MatcherImpl::Fuzzy(FuzzyMatcher::default())
    }
}

impl MatcherImpl {
    /// Construct from plugin config. Mirrors `Config::matcher`.
    pub fn from_config(config: &HarpoonConfig) -> Self {
        match config.matcher {
            MatcherKind::Fuzzy => MatcherImpl::Fuzzy(FuzzyMatcher::default()),
            MatcherKind::Substring => MatcherImpl::Substring(SubstringMatcher::default()),
        }
    }

    /// Identify the variant. Used by the render layer to decide between
    /// `Text::color_indices` (fuzzy, non-contiguous) and `Text::color_range`
    /// (substring, contiguous) per `specs/filter-mode/spec.md`.
    pub fn kind(&self) -> MatcherKind {
        match self {
            MatcherImpl::Fuzzy(_) => MatcherKind::Fuzzy,
            MatcherImpl::Substring(_) => MatcherKind::Substring,
        }
    }
}

impl Matcher for MatcherImpl {
    fn match_indices(&mut self, haystack: &str, needle: &str) -> Option<MatchResult> {
        match self {
            MatcherImpl::Fuzzy(m) => m.match_indices(haystack, needle),
            MatcherImpl::Substring(m) => m.match_indices(haystack, needle),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Trait-level contract tests (run on both backends) ----------

    fn assert_empty_needle_matches<M: Matcher>(mut m: M) {
        let (score, idx) = m.match_indices("anything", "").expect("empty needle should match");
        assert_eq!(score, i32::MAX);
        assert!(idx.is_empty());
    }

    fn assert_no_match_returns_none<M: Matcher>(mut m: M) {
        assert!(m.match_indices("abc", "xyz").is_none());
    }

    // ---------- FuzzyMatcher ----------

    #[test]
    fn fuzzy_empty_needle() {
        assert_empty_needle_matches(FuzzyMatcher::new());
    }

    #[test]
    fn fuzzy_no_match() {
        assert_no_match_returns_none(FuzzyMatcher::new());
    }

    #[test]
    fn fuzzy_ascii_returns_char_indices() {
        let mut m = FuzzyMatcher::new();
        let (_, idx) = m.match_indices("shell | edit log", "ed").unwrap();
        assert_eq!(idx, vec![8, 9]);
    }

    #[test]
    fn fuzzy_multibyte_returns_char_position_not_byte_offset() {
        let mut m = FuzzyMatcher::new();
        let (_, idx) = m.match_indices("📦 build", "b").unwrap();
        assert_eq!(idx, vec![2], "expected char index 2, not byte offset 5");
    }

    #[test]
    fn fuzzy_case_insensitive() {
        let mut m = FuzzyMatcher::new();
        // "Work | Edit log" with "edit"
        let result = m.match_indices("Work | Edit log", "edit");
        assert!(result.is_some(), "case-insensitive fuzzy should match");
    }

    #[test]
    fn fuzzy_contiguous_scores_higher_than_scattered() {
        let mut m = FuzzyMatcher::new();
        let (s_contig, _) = m.match_indices("edit", "ed").unwrap();
        let (s_scattered, _) = m.match_indices("e_d_x", "ed").unwrap();
        assert!(
            s_contig > s_scattered,
            "contiguous {} should score above scattered {}",
            s_contig,
            s_scattered
        );
    }

    #[test]
    fn fuzzy_indices_sorted_ascending() {
        let mut m = FuzzyMatcher::new();
        let (_, idx) = m.match_indices("a-b-c-d-e", "abcd").unwrap();
        let mut sorted = idx.clone();
        sorted.sort_unstable();
        assert_eq!(idx, sorted, "indices should be sorted ascending");
    }

    // ---------- SubstringMatcher ----------

    #[test]
    fn substring_empty_needle() {
        assert_empty_needle_matches(SubstringMatcher::new());
    }

    #[test]
    fn substring_no_match() {
        assert_no_match_returns_none(SubstringMatcher::new());
    }

    #[test]
    fn substring_ascii_match_returns_char_range() {
        let mut m = SubstringMatcher::new();
        let (_, idx) = m.match_indices("shell | edit log", "edit").unwrap();
        assert_eq!(idx, vec![8, 9, 10, 11]);
    }

    #[test]
    fn substring_case_insensitive() {
        let mut m = SubstringMatcher::new();
        let (_, idx) = m.match_indices("shell | EDIT log", "edit").unwrap();
        assert_eq!(idx, vec![8, 9, 10, 11]);
    }

    #[test]
    fn substring_multibyte_haystack_chars_not_bytes() {
        // "📦 build | tail log" — char positions of "log" are 16,17,18 if
        // we count: 📦(0) (1) b(2) u(3) i(4) l(5) d(6) (7) |(8) (9) t(10)
        // a(11) i(12) l(13) (14) l(15) o(16) g(17). So "log" → [15,16,17].
        let mut m = SubstringMatcher::new();
        let (_, idx) = m.match_indices("📦 build | tail log", "log").unwrap();
        assert_eq!(idx, vec![15, 16, 17]);
    }

    #[test]
    fn substring_no_partial_match_into_haystack_end() {
        // needle longer than haystack from any start
        let mut m = SubstringMatcher::new();
        assert!(m.match_indices("ab", "abc").is_none());
    }

    #[test]
    fn substring_does_not_match_non_contiguous() {
        let mut m = SubstringMatcher::new();
        // "edit" should NOT match "e_d_i_t" (substring requires contiguous).
        assert!(m.match_indices("e_d_i_t", "edit").is_none());
    }

    #[test]
    fn substring_earlier_match_scores_higher() {
        let mut m = SubstringMatcher::new();
        let (s_early, _) = m.match_indices("ab cd ab", "ab").unwrap();
        let (s_late, _) = m.match_indices("xx ab", "ab").unwrap();
        assert!(
            s_early > s_late,
            "earlier match {} should score above later match {}",
            s_early,
            s_late
        );
    }

    // ---------- MatcherImpl static dispatch ----------

    #[test]
    fn matcher_impl_default_is_fuzzy() {
        let m = MatcherImpl::default();
        assert_eq!(m.kind(), MatcherKind::Fuzzy);
    }

    #[test]
    fn matcher_impl_from_config_fuzzy() {
        let cfg = HarpoonConfig {
            matcher: MatcherKind::Fuzzy,
            ..Default::default()
        };
        let m = MatcherImpl::from_config(&cfg);
        assert_eq!(m.kind(), MatcherKind::Fuzzy);
    }

    #[test]
    fn matcher_impl_from_config_substring() {
        let cfg = HarpoonConfig {
            matcher: MatcherKind::Substring,
            ..Default::default()
        };
        let m = MatcherImpl::from_config(&cfg);
        assert_eq!(m.kind(), MatcherKind::Substring);
    }

    #[test]
    fn matcher_impl_dispatches_to_inner() {
        let mut m = MatcherImpl::Substring(SubstringMatcher::new());
        let (_, idx) = m.match_indices("shell | edit log", "edit").unwrap();
        assert_eq!(idx, vec![8, 9, 10, 11]);
    }
}
