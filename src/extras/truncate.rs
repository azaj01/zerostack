/// Truncate `s` to at most `max` bytes on a UTF-8 char boundary, appending
/// `marker` so callers know content was capped. Plain `String::truncate` panics
/// mid-character (e.g. on CJK); this walks back to the nearest boundary.
pub(crate) fn truncate_cjk(s: &str, max: usize, marker: &str) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut cut = max;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = s[..cut].to_string();
    out.push_str(marker);
    out
}
