export const MAX_ICON_LENGTH = 32;

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

const STARTS_AS_EMOJI = /^\p{Emoji}/u;

const SHOWN_AS_EMOJI = /\p{Emoji_Presentation}|\uFE0F/u;

export function isIcon(value: string) {
  if (value === "" || value.length > MAX_ICON_LENGTH) return false;
  if (!STARTS_AS_EMOJI.test(value) || !SHOWN_AS_EMOJI.test(value)) return false;
  return [...graphemes.segment(value)].length === 1;
}
