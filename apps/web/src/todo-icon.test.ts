import { describe, expect, it } from "vite-plus/test";
import { isIcon } from "./todo-icon";

describe("isIcon", () => {
  it("accepts emoji built from several code points, which the OS palette inserts as one symbol", () => {
    for (const emoji of ["🗑️", "❤️", "1️⃣", "☝🏻", "🇯🇵", "👩‍👩‍👧", "🏴󠁧󠁢󠁥󠁮󠁧󠁿"]) expect(isIcon(emoji), emoji).toBe(true);
  });

  it("refuses letters, kana and symbols, so the slot before a title only ever holds an emoji", () => {
    for (const text of ["a", "1", "あ", "★", "#"]) expect(isIcon(text), text).toBe(false);
  });

  it("refuses a heart typed without the emoji selector, which most systems draw as a plain black glyph", () => {
    expect(isIcon("❤")).toBe(false);
  });

  it("refuses two emoji or an emoji with a trailing space, so the icon stays one fixed-size symbol", () => {
    for (const text of ["🛒🛒", "🛒 ", " 🛒", ""]) expect(isIcon(text), JSON.stringify(text)).toBe(false);
  });

  it("refuses the typed form reserved for later icon kinds, so no client can store one before every app understands it", () => {
    expect(isIcon("line:cart:green")).toBe(false);
    expect(isIcon("image:3f2a9c1e")).toBe(false);
  });
});
