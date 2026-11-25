use unicode_properties::{EmojiStatus, UnicodeEmoji};
use unicode_segmentation::UnicodeSegmentation;

use crate::todos::utf16_len;

pub const MAX_ICON_LENGTH: usize = 32;

pub const ICON_CHOICES: [&str; 40] = [
    "🏠",
    "🧹",
    "🧺",
    "🧽",
    "🗑\u{FE0F}",
    "🪴",
    "🔧",
    "💡",
    "🛒",
    "🥛",
    "🍳",
    "🧻",
    "🛍\u{FE0F}",
    "🎁",
    "👟",
    "🎂",
    "📅",
    "🏥",
    "🦷",
    "💊",
    "✂\u{FE0F}",
    "📞",
    "🚗",
    "✈\u{FE0F}",
    "📚",
    "✏\u{FE0F}",
    "📝",
    "🎒",
    "💼",
    "💻",
    "📮",
    "🧾",
    "💰",
    "🏛\u{FE0F}",
    "🐈",
    "🐕",
    "⭐",
    "❤\u{FE0F}",
    "🎉",
    "⚠\u{FE0F}",
];

fn has_emoji_presentation(c: char) -> bool {
    matches!(
        c.emoji_status(),
        EmojiStatus::EmojiPresentation
            | EmojiStatus::EmojiPresentationAndModifierBase
            | EmojiStatus::EmojiPresentationAndEmojiComponent
            | EmojiStatus::EmojiPresentationAndModifierAndEmojiComponent
    )
}

#[must_use]
pub fn is_icon(value: &str) -> bool {
    if value.is_empty() || utf16_len(value) > MAX_ICON_LENGTH {
        return false;
    }
    let starts_as_emoji = value
        .chars()
        .next()
        .is_some_and(UnicodeEmoji::is_emoji_char);
    let shown_as_emoji = value
        .chars()
        .any(|c| c == '\u{FE0F}' || has_emoji_presentation(c));
    starts_as_emoji && shown_as_emoji && value.graphemes(true).count() == 1
}

fn is_typed(icon: &str) -> bool {
    icon.split_once(':').is_some_and(|(kind, _)| {
        kind.starts_with(|c: char| c.is_ascii_lowercase())
            && kind
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    })
}

#[must_use]
pub fn shown_icon(icon: Option<&str>) -> Option<&str> {
    icon.filter(|icon| !is_typed(icon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_every_emoji_the_picker_offers_so_choosing_one_is_never_refused_by_the_server() {
        for choice in ICON_CHOICES {
            assert!(is_icon(choice), "{choice:?} is refused");
        }
    }

    #[test]
    fn accepts_one_emoji_of_any_shape_the_os_palette_can_type_so_flags_keycaps_skin_tones_and_families_all_work()
     {
        for emoji in [
            "\u{1F6D2}",
            "\u{1F5D1}\u{FE0F}",
            "\u{2764}\u{FE0F}",
            "1\u{FE0F}\u{20E3}",
            "\u{261D}\u{1F3FB}",
            "\u{1F1EF}\u{1F1F5}",
            "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}",
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
        ] {
            assert!(is_icon(emoji), "{emoji:?} is refused");
        }
    }

    #[test]
    fn refuses_text_symbols_drawn_as_text_and_anything_but_exactly_one_emoji_as_the_server_does() {
        for value in [
            "",
            "a",
            "1",
            "\u{3042}",
            "\u{2605}",
            "\u{2764}",
            "\u{1F6D2}\u{1F6D2}",
            "\u{1F6D2} ",
            "line:cart",
        ] {
            assert!(!is_icon(value), "{value:?} is accepted");
        }
    }

    #[test]
    fn counts_the_length_in_utf16_units_as_the_server_does_so_one_long_emoji_sequence_is_refused_only_past_32()
     {
        let family = |people: usize| vec!["\u{1F469}"; people].join("\u{200D}");
        assert_eq!(utf16_len(&family(11)), 32);
        assert!(is_icon(&family(11)));
        assert!(!is_icon(&family(12)));
    }

    #[test]
    fn hides_a_typed_icon_from_a_later_release_instead_of_drawing_its_raw_text() {
        for typed in ["line:cart:green", "image:0f8e2c", "a:", "x1_y-z:anything"] {
            assert_eq!(shown_icon(Some(typed)), None, "{typed:?} is drawn");
        }
    }

    #[test]
    fn draws_any_other_value_as_it_is_so_an_emoji_the_client_does_not_know_yet_still_appears() {
        for value in ["\u{1F6D2}", "Cart:x", "1line:cart", ":cart", "line cart:"] {
            assert_eq!(shown_icon(Some(value)), Some(value));
        }
        assert_eq!(shown_icon(None), None);
    }
}
