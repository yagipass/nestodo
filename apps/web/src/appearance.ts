export const APPEARANCE_COOKIE = "appearance";

export const APPEARANCES = ["system", "light", "dark"] as const;

export type Appearance = (typeof APPEARANCES)[number];

export function appearanceOf(value: string | undefined): Appearance {
  return value === "light" || value === "dark" ? value : "system";
}
