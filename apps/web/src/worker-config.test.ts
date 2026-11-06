import { describe, expect, it } from "vite-plus/test";
import config from "../void.config";

describe("Worker configuration", () => {
  it("sends webhooks to hosts on the Worker's own zone through Cloudflare's front door, so a member cannot use one to reach an origin without passing Access", () => {
    expect(config.worker?.compatibility_flags).toContain("global_fetch_strictly_public");
  });
});
