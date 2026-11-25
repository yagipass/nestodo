import { describe, expect, it } from "vite-plus/test";
import { ICON_CHOICES, imageIcon, imageIdOf, isIcon, isTodoIcon, shownIcon } from "./todo-icon";

const CAT = { id: "3f2a9c1e-5b7d-4e8f-9a0b-1c2d3e4f5a6b" };

describe("isIcon", () => {
  it("accepts every emoji the picker offers, so choosing one can never be refused by the server", () => {
    expect(ICON_CHOICES).toHaveLength(40);
    expect(new Set(ICON_CHOICES).size).toBe(40);
    for (const choice of ICON_CHOICES) expect(isIcon(choice), choice).toBe(true);
  });

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

describe("isTodoIcon", () => {
  it("accepts an emoji or one of the member's images, the two kinds a todo can show", () => {
    expect(isTodoIcon("🛒")).toBe(true);
    expect(isTodoIcon(imageIcon(CAT.id))).toBe(true);
  });

  it("refuses an image reference without a well-formed id, so a typo is refused before any lookup", () => {
    for (const text of ["image:", "image:cat", `image:${CAT.id.toUpperCase()}`, `image:${CAT.id} `, `IMAGE:${CAT.id}`]) {
      expect(isTodoIcon(text), text).toBe(false);
    }
  });

  it("still refuses the other typed kinds, so no client stores one before every app can draw it", () => {
    expect(isTodoIcon("line:cart:green")).toBe(false);
  });
});

describe("imageIdOf", () => {
  it("reads the id back from what imageIcon stores, so the apps find the image a todo points at", () => {
    expect(imageIdOf(imageIcon(CAT.id))).toBe(CAT.id);
    expect(imageIdOf("🛒")).toBeNull();
    expect(imageIdOf(null)).toBeNull();
  });
});

describe("shownIcon", () => {
  it("draws nothing for a typed icon from a newer release instead of printing its raw text before the title", () => {
    expect(shownIcon("line:cart:green")).toBeNull();
    expect(shownIcon("image:3f2a9c1e-5b7d")).toBeNull();
  });

  it("draws an emoji icon as it is stored", () => {
    expect(shownIcon("🛒")).toBe("🛒");
    expect(shownIcon(null)).toBeNull();
  });
});
