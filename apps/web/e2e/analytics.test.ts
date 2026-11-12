import { randomUUID } from "node:crypto";
import { describe, expect, it } from "vite-plus/test";
import { signedInUser } from "./helpers";

describe("consent to usage analytics", () => {
  it("is off for a member who has never turned it on, because nothing may be sent without a yes", async () => {
    const member = await signedInUser();

    expect(await member.analytics()).toBe(false);
    expect((await member.settings()).analytics).toBe(false);
  });

  it("follows the member to another device, because the choice belongs to the account", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const laptop = await signedInUser(email);
    expect((await laptop.setAnalytics(true)).status).toBe(200);

    const phone = await signedInUser(email);

    expect(await phone.analytics()).toBe(true);
    expect((await phone.settings()).analytics).toBe(true);
  });

  it("turns off everywhere when the member withdraws it", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const laptop = await signedInUser(email);
    await laptop.setAnalytics(true);
    await laptop.setAnalytics(false);

    expect(await (await signedInUser(email)).analytics()).toBe(false);
  });

  it("stays on when the member turns it on twice", async () => {
    const member = await signedInUser();
    await member.setAnalytics(true);

    expect((await member.setAnalytics(true)).status).toBe(200);
    expect(await member.analytics()).toBe(true);
  });

  it("does not turn on for another member of the family", async () => {
    await (await signedInUser()).setAnalytics(true);

    expect(await (await signedInUser()).analytics()).toBe(false);
  });

  it("tells the apps that this server sends usage data, so they show the switch only where turning it on does something", async () => {
    const member = await signedInUser();

    const res = await member.request("/api/v1/analytics", { origin: null });

    expect(await res.json()).toEqual(expect.objectContaining({ available: true, enabled: false }));
  });

  it("hands the apps this server's PostHog project, so their usage data goes to the project the server's owner chose", async () => {
    const member = await signedInUser();

    const res = await member.request("/api/v1/analytics", { origin: null });

    expect(await res.json()).toEqual(expect.objectContaining({ token: "phc_e2e" }));
  });

  it("rejects anything but a yes or no, so a malformed request cannot record consent", async () => {
    const member = await signedInUser();

    expect((await member.setAnalytics("yes")).status).toBe(400);
    expect(await member.analytics()).toBe(false);
  });
});
