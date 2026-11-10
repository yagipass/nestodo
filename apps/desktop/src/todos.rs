pub const MAX_TITLE_LENGTH: usize = 2000;

pub const MAX_NOTE_LENGTH: usize = 10000;

pub const MAX_DEPTH: usize = 64;

#[must_use]
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}
