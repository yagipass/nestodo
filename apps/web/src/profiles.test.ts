import { describe, expect, it } from "vite-plus/test";
import { avatarProblem, avatarTypeOf, isAvatarId, MAX_AVATAR_BYTES, MAX_NAME_LENGTH, nameProblem, normalizeName } from "./profiles";

const bytes = (...values: (number | string)[]) =>
  Uint8Array.from(values.flatMap((value) => (typeof value === "string" ? [...value].map((char) => char.charCodeAt(0)) : [value])));

const PNG = bytes(0x89, "PNG\r\n\x1a\n", 0, 0, 0, 13);
const JPEG = bytes(0xff, 0xd8, 0xff, 0xe0);
const WEBP = bytes("RIFF", 0, 0, 0, 0, "WEBPVP8 ");

describe("normalizeName", () => {
  it("drops the spaces around a name so a stray space does not end up in the account menu", () => {
    expect(normalizeName("  Kim \n")).toBe("Kim");
  });

  it("clears the name when only spaces are left, so the apps fall back to the email address instead of showing nothing", () => {
    expect(normalizeName("   ")).toBeNull();
    expect(normalizeName("")).toBeNull();
  });

  it("stores the composed form, so a name typed on a Mac and on Android is the same text", () => {
    expect(normalizeName("\u30AB\u3099")).toBe("\u30AC");
  });
});

describe("nameProblem", () => {
  it("accepts a name in any script", () => {
    expect(nameProblem("山田 太郎")).toBeNull();
    expect(nameProblem("Zoë")).toBeNull();
  });

  it("refuses characters that reverse or hide text, so a name cannot pose as another member's once names are shown to others", () => {
    expect(nameProblem("Kim\u202Eevil")).not.toBeNull();
    expect(nameProblem("Kim\u2066")).not.toBeNull();
    expect(nameProblem("Kim\u200F")).not.toBeNull();
  });

  it("refuses control characters such as a line break, which would break the one-line menu", () => {
    expect(nameProblem("Kim\nLee")).not.toBeNull();
    expect(nameProblem("Kim\u2028Lee")).not.toBeNull();
  });

  it("refuses invisible characters, so a name cannot look blank or pass for another that reads the same", () => {
    expect(nameProblem("\u200B")).not.toBeNull();
    expect(nameProblem("K\u200Bim")).not.toBeNull();
    expect(nameProblem("Kim\uFEFF")).not.toBeNull();
    expect(nameProblem("K\u2060im")).not.toBeNull();
  });

  it("keeps the joiners that emoji and some scripts need", () => {
    expect(nameProblem("Kim \u{1F468}\u200D\u{1F469}\u200D\u{1F467}")).toBeNull();
    expect(nameProblem("\u0915\u094D\u200C\u0937")).toBeNull();
  });

  it("refuses a name too long to fit the menu", () => {
    expect(nameProblem("a".repeat(MAX_NAME_LENGTH))).toBeNull();
    expect(nameProblem("a".repeat(MAX_NAME_LENGTH + 1))).not.toBeNull();
  });
});

describe("avatarTypeOf", () => {
  it("recognizes PNG, JPEG and WebP from their first bytes, so the content type served comes from the file rather than from the client", () => {
    expect(avatarTypeOf(PNG)).toBe("image/png");
    expect(avatarTypeOf(JPEG)).toBe("image/jpeg");
    expect(avatarTypeOf(WEBP)).toBe("image/webp");
  });

  it("refuses SVG and HTML, which could run script when opened from this site", () => {
    expect(avatarTypeOf(bytes('<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>'))).toBeNull();
    expect(avatarTypeOf(bytes("<!doctype html><script>alert(1)</script>"))).toBeNull();
  });

  it("refuses a RIFF file that is not WebP, such as a WAV recording", () => {
    expect(avatarTypeOf(bytes("RIFF", 0, 0, 0, 0, "WAVEfmt "))).toBeNull();
  });

  it("refuses a file cut off before its signature ends", () => {
    expect(avatarTypeOf(PNG.slice(0, 4))).toBeNull();
    expect(avatarTypeOf(new Uint8Array())).toBeNull();
  });
});

describe("avatarProblem", () => {
  it("refuses an image larger than the limit, so one upload cannot fill the storage or slow down every menu", () => {
    const large = new Uint8Array(MAX_AVATAR_BYTES + 1);
    large.set(PNG);

    expect(avatarProblem(large)).toMatch(/KB or smaller/);
    expect(avatarProblem(large.slice(0, MAX_AVATAR_BYTES))).toBeNull();
  });
});

describe("isAvatarId", () => {
  it("accepts only the ids the server makes, so a request cannot reach another object in the bucket", () => {
    expect(isAvatarId(crypto.randomUUID())).toBe(true);
    expect(isAvatarId("../todos")).toBe(false);
    expect(isAvatarId("")).toBe(false);
  });
});
