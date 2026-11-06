import { beforeEach, describe, expect, inject, it } from "vite-plus/test";
import { corruptStoredWebhook, deleteAllWebhooks, json, runCron, Session, signedInUser, storedWebhook, triggerCron, type WebhookHeader } from "./helpers";

const EVERY_TEN_MINUTES = "*/10 * * * *";
const MINUTE = 60_000;
const PROJECT = "Side projects";

const receiver = inject("receiver");

type Received = { path: string; authorization?: string; version?: string; apiKey?: string; contentType?: string; body: unknown };

function later(ms: number) {
  return new Date(Date.now() + ms);
}

function hook(name: string) {
  return `${receiver.url}/hook/${name}`;
}

async function received() {
  return json<Received[]>(await fetch(`${receiver.url}/received`));
}

async function addUnderProject(client: Session, title: string, note = "") {
  const root = await client.createTodo(PROJECT);
  return client.createTodo(title, { parentId: root.id, note });
}

function secret(name: string, value: string): WebhookHeader {
  return { name, value, secret: true };
}

function plain(name: string, value: string): WebhookHeader {
  return { name, value, secret: false };
}

async function memberWithWebhook(url: string, token = "", headers: WebhookHeader[] = []) {
  const client = await signedInUser();
  await client.saveWebhook({ url, token, headers, project: PROJECT });
  expect((await client.settings()).webhook).not.toBeNull();
  return client;
}

beforeEach(async () => {
  deleteAllWebhooks();
  await fetch(`${receiver.url}/received`, { method: "DELETE" });
});

describe("sending waiting todos to a webhook", () => {
  it("sends a todo's title and note with the token once it has sat 3 minutes without a note, so the receiver can act without reading the app", async () => {
    const client = await memberWithWebhook(hook("mine"), "e2e-token");
    const todo = await addUnderProject(client, "read the Workers changelog", "https://developers.cloudflare.com/workers/");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toEqual([
      {
        path: "/hook/mine",
        authorization: "Bearer e2e-token",
        contentType: "application/json",
        body: {
          event: "todos.awaiting_note",
          project: PROJECT,
          todos: [{ id: todo.id, title: "read the Workers changelog", note: "https://developers.cloudflare.com/workers/" }],
        },
      },
    ]);
    expect((await client.settings()).webhook).toMatchObject({ lastError: null, lastSentAt: expect.any(String) });
  });

  it("sends nothing while every todo under the project has a note, so the receiver is not called for nothing", async () => {
    const client = await memberWithWebhook(hook("mine"));
    await addUnderProject(client, "read the Workers changelog", "check what changed in cron triggers");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toEqual([]);
  });

  it("sends each member's todos only to that member's URL, since a todo is private to its owner", async () => {
    const alice = await memberWithWebhook(hook("alice"));
    const bob = await memberWithWebhook(hook("bob"));
    const alicesTodo = await addUnderProject(alice, "alice's task");
    const bobsTodo = await addUnderProject(bob, "bob's task");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    const titlesFor = async (path: string) =>
      (await received()).filter((call) => call.path === path).flatMap((call) => (call.body as { todos: { id: number }[] }).todos.map((todo) => todo.id));
    expect(await titlesFor("/hook/alice")).toEqual([alicesTodo.id]);
    expect(await titlesFor("/hook/bob")).toEqual([bobsTodo.id]);
  });

  it("matches the project when it was typed with spaces around it, so a stray space does not silently stop the sends", async () => {
    const client = await signedInUser();
    await client.saveWebhook({ url: hook("mine"), token: "", project: `  ${PROJECT}\u3000` });
    await addUnderProject(client, "read the Workers changelog");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toHaveLength(1);
  });

  it("stops sending once the member removes the webhook", async () => {
    const client = await memberWithWebhook(hook("mine"));
    await addUnderProject(client, "read the Workers changelog");
    expect((await client.removeWebhook()).ok).toBe(true);

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toEqual([]);
    expect((await client.settings()).webhook).toBeNull();
  });
});

describe("a webhook that fails", () => {
  it("fails the run and tells the member, so a revoked token or a used-up limit does not go unnoticed", async () => {
    const client = await memberWithWebhook(hook("refuses"));
    await addUnderProject(client, "read the Workers changelog");

    const res = await triggerCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(res.ok).toBe(false);
    expect((await client.settings()).webhook?.lastError).toBe("The server answered 401");
  });

  it("clears the failure when the member saves a fix, so an old error does not read as a current one", async () => {
    const client = await memberWithWebhook(hook("refuses"));
    await addUnderProject(client, "read the Workers changelog");
    await triggerCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    await client.saveWebhook({ url: hook("mine"), token: "", project: PROJECT });

    expect((await client.settings()).webhook?.lastError).toBeNull();
  });

  it("still sends to other members, so one member's broken URL does not cost anyone else their webhook", async () => {
    const broken = await memberWithWebhook(hook("refuses"));
    const healthy = await memberWithWebhook(hook("mine"));
    await addUnderProject(broken, "broken member's task");
    await addUnderProject(healthy, "healthy member's task");

    await triggerCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect((await received()).map((call) => call.path)).toContain("/hook/mine");
    expect((await healthy.settings()).webhook?.lastError).toBeNull();
  });

  it("does not follow a redirect, so the token and the todos are not handed to a server the member did not choose", async () => {
    const client = await memberWithWebhook(hook("redirects"), "e2e-token");
    await addUnderProject(client, "read the Workers changelog");

    const res = await triggerCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(res.ok).toBe(false);
    expect((await received()).map((call) => call.path)).toEqual(["/hook/redirects"]);
    expect((await client.settings()).webhook?.lastError).toBe("The server answered 302");
  });
});

describe("saving webhook settings", () => {
  it("never gives a secret URL, not even its host, or the token back to the browser, since a token is shown once at its source and a URL can itself be a secret, sometimes in its host name", async () => {
    const client = await memberWithWebhook(hook("secret-path"), "e2e-secret-token");

    const res = await client.request("/settings", { page: true });
    const page = await res.text();

    expect(page).not.toContain("secret-path");
    expect(page).not.toContain(new URL(receiver.url).host);
    expect(page).not.toContain("e2e-secret-token");
    expect((await client.settings()).webhook).toMatchObject({ url: null, hasToken: true, unreadable: false, project: PROJECT });
  });

  it("shows a URL the member did not mark secret, so it can be read and edited without pasting it again", async () => {
    const client = await signedInUser();
    await client.saveWebhook({ url: hook("visible-path"), urlSecret: false, token: "", project: PROJECT });

    expect((await client.settings()).webhook?.url).toBe(hook("visible-path"));
  });

  it("stores a URL and token encrypted even when the URL is not secret, so reading the database does not reveal them", async () => {
    const client = await signedInUser("stored@e2e.test");
    await client.saveWebhook({ url: hook("secret-path"), urlSecret: false, token: "e2e-secret-token", project: PROJECT });

    const stored = storedWebhook("stored@e2e.test");

    expect(stored).not.toContain("secret-path");
    expect(stored).not.toContain(new URL(receiver.url).host);
    expect(stored).not.toContain("e2e-secret-token");
  });

  it("removes the token when asked, so a server that stopped wanting one does not keep receiving it", async () => {
    const client = await memberWithWebhook(hook("mine"), "e2e-token");
    await client.saveWebhook({ url: "", token: null, project: PROJECT });
    await addUnderProject(client, "read the Workers changelog");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    const [call] = await received();
    expect(call.authorization).toBeUndefined();
    expect((await client.settings()).webhook?.hasToken).toBe(false);
  });

  it("drops the saved token when the URL changes, so one service's credential never reaches another", async () => {
    const client = await memberWithWebhook(hook("first"), "e2e-token");
    await client.saveWebhook({ url: hook("second"), token: "", project: PROJECT });
    await addUnderProject(client, "read the Workers changelog");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    const [call] = await received();
    expect(call.path).toBe("/hook/second");
    expect(call.authorization).toBeUndefined();
    expect((await client.settings()).webhook?.hasToken).toBe(false);
  });

  it("keeps the saved URL and token when both are left blank, so renaming the project does not mean generating a new token", async () => {
    const client = await memberWithWebhook(hook("mine"), "e2e-token");
    await client.saveWebhook({ url: "", token: "", project: "Work" });
    const root = await client.createTodo("Work");
    await client.createTodo("prepare the review", { parentId: root.id });

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toMatchObject([{ path: "/hook/mine", authorization: "Bearer e2e-token", body: { project: "Work" } }]);
  });

  it("replaces only the token when the URL is left blank, so a regenerated token can be saved without pasting the URL again", async () => {
    const client = await memberWithWebhook(hook("mine"), "old-token");
    await client.saveWebhook({ url: "", token: "new-token", project: PROJECT });
    await addUnderProject(client, "read the Workers changelog");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toMatchObject([{ path: "/hook/mine", authorization: "Bearer new-token" }]);
  });

  it("asks for a URL on the first save, since there is nothing to send to yet", async () => {
    const client = await signedInUser();

    const res = await client.saveWebhook({ url: "", token: "", project: PROJECT });

    expect(await res.json()).toMatchObject({ errors: { url: expect.any(String) } });
    expect((await client.settings()).webhook).toBeNull();
  });

  it("refuses a URL that is not http or https, so a typo shows on save instead of failing every send", async () => {
    const client = await signedInUser();

    const res = await client.saveWebhook({ url: "ftp://example.com/hook", token: "", project: PROJECT });

    expect(await res.json()).toMatchObject({ errors: { url: expect.any(String) } });
    expect((await client.settings()).webhook).toBeNull();
  });

  it("refuses a token with a line break, which would make every send fail", async () => {
    const client = await signedInUser();

    const res = await client.saveWebhook({ url: hook("mine"), token: "a\nb", project: PROJECT });

    expect(await res.json()).toMatchObject({ errors: { token: expect.any(String) } });
    expect((await client.settings()).webhook).toBeNull();
  });

  it("asks for everything again when the saved target can no longer be read, so a rotated key does not fail silently at every send", async () => {
    const client = await signedInUser("unreadable@e2e.test");
    await client.saveWebhook({ url: hook("mine"), urlSecret: false, token: "e2e-token", headers: [plain("X-Api-Version", "2")], project: PROJECT });
    corruptStoredWebhook("unreadable@e2e.test");

    expect((await client.settings()).webhook).toMatchObject({ unreadable: true, url: null, headers: [secret("X-Api-Version", "")] });

    const blank = await client.saveWebhook({ url: "", token: "", project: PROJECT });
    expect(await blank.json()).toMatchObject({ errors: { url: expect.stringContaining("no longer be read") } });

    await client.saveWebhook({ url: hook("mine"), token: "", project: PROJECT });
    expect((await client.settings()).webhook).toMatchObject({ unreadable: false, hasToken: false });
  });

  it("keeps one member's save away from another member's webhook", async () => {
    const alice = await memberWithWebhook(hook("alice"), "alice-token");
    const bob = await signedInUser();

    await bob.saveWebhook({ url: hook("bob"), token: "", project: PROJECT });

    expect((await alice.settings()).webhook?.hasToken).toBe(true);
    expect((await bob.settings()).webhook?.hasToken).toBe(false);
  });

  it("refuses someone who is not signed in, since these settings send a member's todos elsewhere", async () => {
    const stranger = new Session();

    expect((await stranger.request("/settings", { page: true })).status).toBe(401);
    expect((await stranger.saveWebhook({ url: hook("mine"), token: "", project: PROJECT })).status).toBe(401);
    expect((await stranger.removeWebhook()).status).toBe(401);
  });

  it("refuses a save from another site, so a page the member visits cannot redirect their todos", async () => {
    const client = await signedInUser();

    const res = await client.request("/settings?save", {
      method: "POST",
      page: true,
      origin: "https://evil.example",
      body: { url: hook("evil"), urlSecret: true, token: "", headers: [], project: PROJECT },
    });

    expect(res.status).toBe(403);
    expect((await client.settings()).webhook).toBeNull();
  });
});

describe("a webhook's custom headers", () => {
  it("sends them with the todos, so a server that wants X-Api-Key or X-Api-Version can be reached without changing the app", async () => {
    const client = await memberWithWebhook(hook("mine"), "", [secret("X-Api-Key", "e2e-header-value"), plain("X-Api-Version", "2")]);
    await addUnderProject(client, "read the Workers changelog");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    const [call] = await received();
    expect(call).toMatchObject({ path: "/hook/mine", apiKey: "e2e-header-value", version: "2", contentType: "application/json" });
    expect(call.authorization).toBeUndefined();
  });

  it("asks for a secret value again when the URL changes, so one service's key never reaches another", async () => {
    const client = await memberWithWebhook(hook("first"), "", [secret("X-Api-Key", "e2e-header-value")]);

    const res = await client.saveWebhook({ url: hook("second"), token: "", headers: [secret("X-Api-Key", "")], project: PROJECT });
    expect(await res.json()).toMatchObject({ errors: { headers: expect.stringContaining("new URL") } });

    await client.saveWebhook({ url: hook("second"), token: "", headers: [plain("X-Api-Version", "2")], project: PROJECT });
    await addUnderProject(client, "read the Workers changelog");
    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    const [call] = await received();
    expect(call).toMatchObject({ path: "/hook/second", version: "2" });
    expect(call.apiKey).toBeUndefined();
  });

  it("keeps a secret value left blank, so renaming the project does not mean entering every key again", async () => {
    const client = await memberWithWebhook(hook("mine"), "", [secret("X-Api-Key", "e2e-header-value")]);
    await client.saveWebhook({ url: "", token: "", headers: [secret("X-Api-Key", "")], project: "Work" });
    const root = await client.createTodo("Work");
    await client.createTodo("prepare the review", { parentId: root.id });

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toMatchObject([{ apiKey: "e2e-header-value", body: { project: "Work" } }]);
  });

  it("stops sending a header left out of the list, so a removed header does not keep reaching the server", async () => {
    const client = await memberWithWebhook(hook("mine"), "", [secret("X-Api-Key", "e2e-header-value"), plain("X-Api-Version", "2")]);
    await client.saveWebhook({ url: "", token: "", headers: [secret("X-Api-Key", "e2e-new-value")], project: PROJECT });
    await addUnderProject(client, "read the Workers changelog");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    const [call] = await received();
    expect(call.apiKey).toBe("e2e-new-value");
    expect(call.version).toBeUndefined();
  });

  it("shows only values not marked secret, and stores them all encrypted, since a header can hold a secret", async () => {
    const client = await signedInUser("headers@e2e.test");
    await client.saveWebhook({ url: hook("mine"), token: "", headers: [secret("X-Api-Key", "e2e-header-value"), plain("X-Visible", "e2e-visible-value")], project: PROJECT });

    const page = await (await client.request("/settings", { page: true })).text();

    expect((await client.settings()).webhook?.headers).toEqual([secret("X-Api-Key", ""), plain("X-Visible", "e2e-visible-value")]);
    expect(page).not.toContain("e2e-header-value");
    expect(storedWebhook("headers@e2e.test")).not.toContain("e2e-header-value");
    expect(storedWebhook("headers@e2e.test")).not.toContain("e2e-visible-value");
  });

  it("does not reveal a secret value when Secret is turned off without typing it, so a borrowed session cannot read a saved key", async () => {
    const client = await memberWithWebhook(hook("mine"), "", [secret("X-Api-Key", "e2e-header-value")]);

    const res = await client.saveWebhook({ url: "", token: "", headers: [plain("X-Api-Key", "")], project: PROJECT });
    const body = await res.text();

    expect(JSON.parse(body)).toMatchObject({ errors: { headers: expect.stringContaining("again") } });
    expect(body).not.toContain("e2e-header-value");
    expect((await client.settings()).webhook?.headers).toEqual([secret("X-Api-Key", "")]);
  });

  it("refuses a header that changes how the request is delivered, so a setting cannot break or redirect the send", async () => {
    const client = await signedInUser();

    const res = await client.saveWebhook({ url: hook("mine"), token: "", headers: [secret("Host", "evil.example")], project: PROJECT });

    expect(await res.json()).toMatchObject({ errors: { headers: expect.stringContaining("Host") } });
    expect((await client.settings()).webhook).toBeNull();
  });

  it("does not repeat a refused value in the error, since it may be a secret pasted by mistake", async () => {
    const client = await signedInUser();

    const res = await client.saveWebhook({ url: hook("mine"), token: "", headers: [secret("", "e2e-pasted-secret-without-a-name")], project: PROJECT });
    const body = await res.text();

    expect(JSON.parse(body)).toMatchObject({ errors: { headers: expect.any(String) } });
    expect(body).not.toContain("e2e-pasted-secret-without-a-name");
  });

  it("refuses a token together with an Authorization header, since the receiver could only be sent one of them", async () => {
    const client = await signedInUser();

    const res = await client.saveWebhook({ url: hook("mine"), token: "e2e-token", headers: [secret("Authorization", "Basic e2e")], project: PROJECT });

    expect(await res.json()).toMatchObject({ errors: { headers: expect.stringContaining("Authorization") } });
    expect((await client.settings()).webhook).toBeNull();
  });

  it("refuses a new token when an Authorization header is already saved, so the saved scheme is not silently overwritten", async () => {
    const client = await memberWithWebhook(hook("mine"), "", [secret("Authorization", "Basic e2e")]);

    const res = await client.saveWebhook({ url: "", token: "e2e-token", headers: [secret("Authorization", "")], project: PROJECT });

    expect(await res.json()).toMatchObject({ errors: { headers: expect.stringContaining("Authorization") } });
  });

  it("sends an Authorization header when there is no token, so servers using a scheme other than Bearer can be reached", async () => {
    const client = await memberWithWebhook(hook("mine"), "", [secret("Authorization", "Basic e2e")]);
    await addUnderProject(client, "read the Workers changelog");

    await runCron(EVERY_TEN_MINUTES, later(5 * MINUTE));

    expect(await received()).toMatchObject([{ authorization: "Basic e2e" }]);
  });
});
