import { randomUUID } from "node:crypto";
import { exportJWK, generateKeyPair } from "jose";
import { describe, expect, inject, it } from "vite-plus/test";
import { accessAssertion, Session, signedInUser } from "./helpers";

const access = inject("access");

async function forgedSigningKey() {
  const { privateKey } = await generateKeyPair("RS256", { extractable: true });
  return { ...(await exportJWK(privateKey)), kid: "e2e", alg: "RS256" };
}

describe("only identities that Cloudflare Access vouched for reach a user's todos", () => {
  it.each([
    { name: "signed by someone other than Access", options: async () => ({ signingKey: await forgedSigningKey() }) },
    { name: "issued for another Access application", options: async () => ({ audience: "another-access-app" }) },
    { name: "issued by another Access team", options: async () => ({ issuer: "https://other-team.cloudflareaccess.com" }) },
    { name: "expired", options: async () => ({ expiresAt: Math.floor(Date.now() / 1000) - 60 }) },
  ])("refuses an assertion $name even when it names the victim's email", async ({ options }) => {
    const email = `${randomUUID()}@e2e.test`;
    const victim = await signedInUser(email);
    await victim.createTodo("victim's secret");

    const attacker = new Session(await accessAssertion({ email, ...(await options()) }));

    expect((await attacker.request("/api/v1/todos")).status).toBe(401);
    expect((await attacker.request("/", { page: true })).status).toBe(401);
  });

  it("refuses an Access service token because it belongs to no user", async () => {
    const serviceToken = new Session(await accessAssertion({}));
    expect((await serviceToken.request("/api/v1/todos")).status).toBe(401);
  });

  it("keeps the MCP connector's Access login to the MCP endpoint, so Claude cannot delete todos or redirect the member's webhook", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const member = await signedInUser(email);
    const list = await member.createTodo("Reading list");
    await member.createTodo("chapter one", { parentId: list.id });

    const connector = new Session(await accessAssertion({ email, audience: access.mcpAudience }));

    expect((await connector.request(`/api/v1/todos/${list.id}?recursive=true`, { method: "DELETE" })).status).toBe(401);
    expect((await connector.saveWebhook({ url: "https://hooks.example/elsewhere", token: null, project: "Reading list" })).status).toBe(401);
    expect((await connector.setAnalytics(true)).status).toBe(401);
    expect((await connector.request("/settings", { page: true })).status).toBe(401);

    expect((await member.todos()).map((t) => t.title)).toEqual(["Reading list", "chapter one"]);
    expect((await member.settings()).webhook).toBeNull();
    expect(await member.analytics()).toBe(false);
  });

  it("keeps a user's todos when Access sends their email in different letter case", async () => {
    const id = randomUUID();
    const first = await signedInUser(`Family.${id}@E2E.test`);
    await first.createTodo("buy milk");

    const later = await signedInUser(`family.${id}@e2e.test`);
    expect((await later.todos()).map((t) => t.title)).toEqual(["buy milk"]);
  });

  it("rejects changes sent from another site even when the browser carries the user's Access login", async () => {
    const user = await signedInUser();

    const crossSite = await user.request("/api/v1/todos", {
      method: "POST",
      body: { title: "from another site" },
      origin: "https://evil.example",
    });
    expect(crossSite.status).toBe(403);
    expect(await user.todos()).toEqual([]);

    const sameSite = await user.request("/api/v1/todos", { method: "POST", body: { title: "from the app" } });
    expect(sameSite.status).toBe(201);
  });

  it("rejects a change from another site even when the browser leaves out the Origin header, while the CLI and Android app, which send neither header, still work", async () => {
    const user = await signedInUser();

    const crossSite = await user.request("/api/v1/todos", {
      method: "POST",
      body: { title: "from another site" },
      origin: null,
      secFetchSite: "cross-site",
    });
    expect(crossSite.status).toBe(403);
    expect(await user.todos()).toEqual([]);

    const nativeClient = await user.request("/api/v1/todos", { method: "POST", body: { title: "from the CLI" }, origin: null });
    expect(nativeClient.status).toBe(201);
  });

  it("accepts changes only as JSON, which a page on another site cannot send without the browser asking first, so even an old browser that sends neither Origin nor Sec-Fetch-Site cannot be used to add todos", async () => {
    const user = await signedInUser();

    for (const contentType of ["text/plain;charset=UTF-8", "application/x-www-form-urlencoded", "multipart/form-data; boundary=x"]) {
      const res = await user.request("/api/v1/todos", { method: "POST", body: { title: "from another site" }, contentType, origin: null });
      expect(res.status).toBe(415);
    }
    expect(await user.todos()).toEqual([]);

    const android = await user.request("/api/v1/todos", {
      method: "POST",
      body: { title: "from Android" },
      contentType: "application/json; charset=utf-8",
      origin: null,
    });
    expect(android.status).toBe(201);
  });

  it("keeps signed-in pages out of caches so a shared browser cannot show them to the next person", async () => {
    const user = await signedInUser();
    const res = await user.request("/", { page: true });
    expect(res.status).toBe(200);
    expect(res.headers.get("cache-control")).toBe("no-store");
  });
});
