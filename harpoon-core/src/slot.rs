//! Slot character ↔ index conversion. 35 slots total: digits `1-9` map to
//! `0..=8`, letters `a-z` map to `9..=34`. Slot `0` (digit) is intentionally
//! unused — keeping the digit set non-zero matches user expectation that
//! "slot 1 is the first one".
//!
//! See `specs/jump-mode/spec.md` "Slot mapping".

/// Resolve a slot character to a `panes` index.
///
/// - `'1'..='9'` → `Some(0..=8)` (9 digit slots, slot `0` unused)
/// - `'a'..='z'` → `Some(9..=34)` (26 letter slots)
/// - All other chars → `None`
pub fn slot_index_from_char(c: char) -> Option<usize> {
    if c.is_ascii_digit() && c != '0' {
        Some((c as usize) - ('1' as usize))
    } else if c.is_ascii_lowercase() {
        Some(9 + (c as usize) - ('a' as usize))
    } else {
        None
    }
}

/// Inverse of [`slot_index_from_char`]. Returns the slot character for a
/// given panes index, or `None` if the index has no slot (≥ 35).
pub fn slot_char_from_index(i: usize) -> Option<char> {
    if i < 9 {
        // 0 → '1', 8 → '9'
        char::from_u32(('1' as u32) + (i as u32))
    } else if i < 35 {
        // 9 → 'a', 34 → 'z'
        char::from_u32(('a' as u32) + (i as u32 - 9))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_one_through_nine() {
        assert_eq!(slot_index_from_char('1'), Some(0));
        assert_eq!(slot_index_from_char('5'), Some(4));
        assert_eq!(slot_index_from_char('9'), Some(8));
    }

    #[test]
    fn digit_zero_unused() {
        assert_eq!(slot_index_from_char('0'), None);
    }

    #[test]
    fn lowercase_letters() {
        assert_eq!(slot_index_from_char('a'), Some(9));
        assert_eq!(slot_index_from_char('b'), Some(10));
        assert_eq!(slot_index_from_char('z'), Some(34));
    }

    #[test]
    fn uppercase_letters_unmapped() {
        assert_eq!(slot_index_from_char('A'), None);
        assert_eq!(slot_index_from_char('Z'), None);
    }

    #[test]
    fn non_alphanumeric_unmapped() {
        assert_eq!(slot_index_from_char('!'), None);
        assert_eq!(slot_index_from_char(' '), None);
        assert_eq!(slot_index_from_char('-'), None);
    }

    #[test]
    fn slot_char_inverse_digits() {
        assert_eq!(slot_char_from_index(0), Some('1'));
        assert_eq!(slot_char_from_index(4), Some('5'));
        assert_eq!(slot_char_from_index(8), Some('9'));
    }

    #[test]
    fn slot_char_inverse_letters() {
        assert_eq!(slot_char_from_index(9), Some('a'));
        assert_eq!(slot_char_from_index(10), Some('b'));
        assert_eq!(slot_char_from_index(34), Some('z'));
    }

    #[test]
    fn slot_char_beyond_35_unmapped() {
        assert_eq!(slot_char_from_index(35), None);
        assert_eq!(slot_char_from_index(100), None);
    }

    #[test]
    fn round_trip_all_valid() {
        for i in 0..35 {
            let c = slot_char_from_index(i).expect("valid index has char");
            assert_eq!(
                slot_index_from_char(c),
                Some(i),
                "round trip should yield original index for i={}",
                i
            );
        }
    }
}
