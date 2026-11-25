import { isImageIconId, type ImageIcon } from "./image-icons";

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

const IMAGE = "image:";

export type ShownIcon = { emoji: string } | { image: ImageIcon };

export function imageIcon(id: string) {
  return `${IMAGE}${id}`;
}

export function imageIdOf(icon: string | null) {
  if (icon === null || !icon.startsWith(IMAGE)) return null;
  const id = icon.slice(IMAGE.length);
  return isImageIconId(id) ? id : null;
}

export function isTodoIcon(value: string) {
  return isIcon(value) || imageIdOf(value) !== null;
}

export function shownIcon(icon: string | null, images: readonly ImageIcon[]): ShownIcon | null {
  if (icon === null) return null;
  if (!TYPED.test(icon)) return { emoji: icon };
  const id = imageIdOf(icon);
  const image = id === null ? undefined : images.find((candidate) => candidate.id === id);
  return image ? { image } : null;
}
