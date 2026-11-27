import { describe, expect, it } from "vite-plus/test";
import { fitWithin } from "./comment-images";

describe("fitWithin", () => {
  it("shrinks a large photo until its longer side fits, keeping its shape, so an upload stays small without cropping anything", () => {
    expect(fitWithin(4032, 3024, 2048)).toEqual({ width: 2048, height: 1536 });
    expect(fitWithin(1170, 2532, 2048)).toEqual({ width: 946, height: 2048 });
  });

  it("never enlarges a small screenshot, which would only blur it and make it bigger", () => {
    expect(fitWithin(800, 600, 2048)).toEqual({ width: 800, height: 600 });
  });

  it("keeps a very thin image at least one pixel wide, so it can still be drawn", () => {
    expect(fitWithin(10000, 1, 2048)).toEqual({ width: 2048, height: 1 });
  });
});
