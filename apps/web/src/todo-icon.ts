export const MAX_ICON_LENGTH = 32;

export const ICON_CHOICES = [
  "🏠", "🧹", "🧺", "🧽", "🗑️", "🪴", "🔧", "💡",
  "🛒", "🥛", "🍳", "🧻", "🛍️", "🎁", "👟", "🎂",
  "📅", "🏥", "🦷", "💊", "✂️", "📞", "🚗", "✈️",
  "📚", "✏️", "📝", "🎒", "💼", "💻", "📮", "🧾",
  "💰", "🏛️", "🐈", "🐕", "⭐", "❤️", "🎉", "⚠️",
] as const;

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

const STARTS_AS_EMOJI = /^\p{Emoji}/u;

const SHOWN_AS_EMOJI = /\p{Emoji_Presentation}|\uFE0F/u;

const TYPED = /^[a-z][a-z0-9_-]*:/;

export function isIcon(value: string) {
  if (value === "" || value.length > MAX_ICON_LENGTH) return false;
  if (!STARTS_AS_EMOJI.test(value) || !SHOWN_AS_EMOJI.test(value)) return false;
  return [...graphemes.segment(value)].length === 1;
}

export function shownIcon(icon: string | null) {
  return icon !== null && !TYPED.test(icon) ? icon : null;
}
