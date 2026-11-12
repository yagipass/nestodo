import { describe, expect, it } from "vite-plus/test";
import { base64Of, centerSquare } from "./avatar-image";

describe("centerSquare", () => {
  it("keeps the middle of a wide or tall photo, where the face usually is", () => {
    expect(centerSquare(400, 300)).toEqual({ x: 50, y: 0, side: 300 });
    expect(centerSquare(300, 400)).toEqual({ x: 0, y: 50, side: 300 });
  });

  it("keeps all of a square photo", () => {
    expect(centerSquare(256, 256)).toEqual({ x: 0, y: 0, side: 256 });
  });
});

describe("base64Of", () => {
  it("encodes an image larger than one call can spread, so a full-size upload does not overflow the stack", async () => {
    const bytes = Uint8Array.from({ length: 300_000 }, (_, index) => index % 251);

    expect(await base64Of(new Blob([bytes]))).toBe(Buffer.from(bytes).toString("base64"));
  });
});
