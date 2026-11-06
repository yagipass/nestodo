import { describe, expect, it } from "vite-plus/test";
import { json, signedInUser } from "./helpers";

describe("installing the site as a desktop app", () => {
  it("fetches the manifest with cookies because Cloudflare Access answers a request without them with its login page", async () => {
    const html = await (await (await signedInUser()).request("/")).text();
    const link = html.match(/<link[^>]*rel="manifest"[^>]*>/)?.[0];

    expect(link).toContain('href="/manifest.webmanifest"');
    expect(link).toContain('crossorigin="use-credentials"');
  });

  it("serves every icon the manifest names so the app does not install without one", async () => {
    const user = await signedInUser();
    const manifest = await json<{ icons: { src: string }[] }>(await user.request("/manifest.webmanifest"));

    expect(manifest.icons.length).toBeGreaterThan(0);
    for (const icon of manifest.icons) {
      const res = await user.request(icon.src);
      expect(res.status, icon.src).toBe(200);
      expect(res.headers.get("content-type"), icon.src).toBe("image/png");
    }
  });
});
