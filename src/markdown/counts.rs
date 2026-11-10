//! Cached counts without allocating token lists.
pub fn text_counts(markdown: &str) -> (usize, usize) {
    let mut words = 0usize;
    let mut in_word = false;
    if markdown.is_ascii() {
        for &b in markdown.as_bytes() {
            if b.is_ascii_whitespace() || b == 0x0b {
                in_word = false;
            } else if !in_word {
                in_word = true;
                words += 1;
            }
        }
        return (words, markdown.len());
    }
    let mut chars = 0usize;
    for ch in markdown.chars() {
        chars += 1;
        if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            in_word = true;
            words += 1;
        }
    }
    (words, chars)
}
