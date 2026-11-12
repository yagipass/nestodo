import { describe, expect, it } from "vite-plus/test";
import { isPagePath } from "./page-path";

describe("isPagePath", () => {
  it("is true for the pages, whose header shows the name and picture", () => {
    expect(isPagePath("/")).toBe(true);
    expect(isPagePath("/stats")).toBe(true);
    expect(isPagePath("/settings")).toBe(true);
  });

  it("is false for files every page load fetches, such as scripts, fonts and icons, so they do not each look up the profile", () => {
    expect(isPagePath("/assets/layout-BVJmEIl4.css")).toBe(false);
    expect(isPagePath("/assets/Figtree-Bold-DRmn1l9e.woff2")).toBe(false);
    expect(isPagePath("/icon.svg")).toBe(false);
    expect(isPagePath("/manifest.webmanifest")).toBe(false);
  });

  it("is false for the API and MCP, which answer without a header", () => {
    expect(isPagePath("/api/v1/profile")).toBe(false);
    expect(isPagePath("/api/v1/avatars/0b0f3f5e-8a35-4c1b-9d0e-2f6c1a7b8c9d")).toBe(false);
    expect(isPagePath("/mcp")).toBe(false);
    expect(isPagePath("/__void/scheduled")).toBe(false);
  });
});
