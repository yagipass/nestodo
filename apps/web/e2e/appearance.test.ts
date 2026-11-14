import { describe, expect, it } from "vite-plus/test";
import { signedInUser, type Session } from "./helpers";

const PAGES = ["/", "/stats?tz=UTC", "/settings"];

async function htmlTag(client: Session, path: string, cookie?: string) {
  const res = await client.request(path, { cookie });
  expect(res.status, path).toBe(200);
  return (await res.text()).match(/<html[^>]*>/)?.[0];
}

describe("the appearance chosen in Settings", () => {
  it("is in every page as it arrives, so a browser set to dark never shows a light page before the scripts run", async () => {
    const client = await signedInUser();

    for (const path of PAGES) {
      expect(await htmlTag(client, path, "appearance=dark"), path).toContain('data-appearance="dark"');
      expect(await htmlTag(client, path, "appearance=light"), path).toContain('data-appearance="light"');
    }
  });

  it("leaves the pages to the system until one is chosen, so a new browser matches the rest of the device", async () => {
    const client = await signedInUser();

    for (const path of PAGES) {
      expect(await htmlTag(client, path), path).not.toContain("data-appearance");
      expect(await htmlTag(client, path, "appearance=sepia"), path).not.toContain("data-appearance");
    }
  });

  it("is shown as selected in Settings, so the browser's own choice is what the page offers to change", async () => {
    const client = await signedInUser();

    expect((await client.settings("appearance=dark")).appearance).toBe("dark");
    expect((await client.settings()).appearance).toBe("system");
  });
});
