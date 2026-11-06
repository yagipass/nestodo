import { describe, expect, it } from "vite-plus/test";
import {
  nextWebhookTarget,
  openWebhookTarget,
  sealWebhookTarget,
  visibleWebhookTarget,
  webhookRequest,
  webhookTargetProblem,
  webhookUrlProblem,
  type WebhookEdit,
  type WebhookTarget,
} from "./webhooks";

const SECRET_KEY = btoa(String.fromCharCode(...new Uint8Array(32).fill(7)));
const OTHER_KEY = btoa(String.fromCharCode(...new Uint8Array(32).fill(8)));

function secret(name: string, value: string) {
  return { name, value, secret: true };
}

function plain(name: string, value: string) {
  return { name, value, secret: false };
}

describe("webhookUrlProblem", () => {
  const production = { allowHttp: false };

  it("accepts an https URL, the only kind a token may travel over", () => {
    expect(webhookUrlProblem("https://example.com/hook?a=b", production)).toBeNull();
  });

  it("refuses http in production, since the todos and the token would cross the network in the clear", () => {
    expect(webhookUrlProblem("http://example.com/hook", production)).toMatch(/https/);
  });

  it("allows http only where a local server receives it, so the app can be tried without a certificate", () => {
    expect(webhookUrlProblem("http://127.0.0.1:8787/hook", { allowHttp: true })).toBeNull();
    expect(webhookUrlProblem("ftp://example.com/hook", { allowHttp: true })).toMatch(/https/);
  });

  it("refuses credentials in the URL, which fetch would reject on every send", () => {
    expect(webhookUrlProblem("https://user:secret@example.com/hook", production)).toMatch(/token/);
  });

  it("refuses text that is not a full URL, so a typo is caught on save and not on the first send", () => {
    expect(webhookUrlProblem("example.com/hook", production)).toMatch(/full URL/);
  });
});

describe("webhookRequest", () => {
  const todo = { id: 12, title: "read the changelog", note: "https://example.com", position: 3, createdAt: "2026-11-05 00:00:00" };
  const hook = { url: "https://example.com/hook", urlSecret: true, token: "", headers: [] };

  it("gives no server special treatment, so every receiver gets only the headers the member listed", () => {
    const receiver = { ...hook, url: "https://api.example.com/v1/jobs/1/run" };
    expect(webhookRequest(receiver, "p", [todo]).headers.has("X-Api-Version")).toBe(false);
    const listed = webhookRequest({ ...receiver, headers: [plain("X-Api-Version", "2")] }, "p", [todo]);
    expect(listed.headers.get("X-Api-Version")).toBe("2");
  });

  it("sends a secret header exactly like any other, since Secret only changes what the screen shows", () => {
    expect(webhookRequest({ ...hook, headers: [secret("X-Api-Key", "k")] }, "p", [todo]).headers.get("X-Api-Key")).toBe("k");
  });

  it("sends the token as a bearer token, the scheme most webhook receivers expect", () => {
    expect(webhookRequest({ ...hook, token: "t" }, "p", [todo]).headers.get("Authorization")).toBe("Bearer t");
  });

  it("sends no Authorization header without a token, so a server whose URL is its secret does not see an empty bearer", () => {
    expect(webhookRequest(hook, "p", [todo]).headers.has("Authorization")).toBe(false);
  });

  it("always labels the body as JSON, since the receiver parses it by that type", () => {
    expect(webhookRequest({ ...hook, headers: [secret("X-Api-Key", "k")] }, "p", [todo]).headers.get("Content-Type")).toBe("application/json");
  });

  it("sends the id, title and note a receiver acts on, and not the app's internal fields", () => {
    const { body } = webhookRequest(hook, "Side projects", [todo]);
    expect(JSON.parse(body)).toEqual({
      event: "todos.awaiting_note",
      project: "Side projects",
      todos: [{ id: 12, title: "read the changelog", note: "https://example.com" }],
    });
  });

  it("does not follow redirects, so a redirect cannot hand the token and the todos to another server", () => {
    expect(webhookRequest({ ...hook, token: "t" }, "p", [todo]).redirect).toBe("manual");
  });
});

describe("nextWebhookTarget", () => {
  const production = { allowHttp: false };
  const URL_A = "https://a.example.com/hook";
  const URL_B = "https://b.example.com/hook";
  const blank: WebhookEdit = { url: "", urlSecret: true, token: "", headers: [] };
  const saved: WebhookTarget = {
    url: URL_A,
    urlSecret: true,
    token: "saved-token",
    headers: [secret("X-Api-Key", "saved-key"), plain("X-Api-Version", "2")],
  };

  function next(edit: Partial<WebhookEdit>, from: WebhookTarget | null = saved) {
    return nextWebhookTarget({ ...blank, ...edit }, from, production);
  }

  function headerProblem(...headers: WebhookEdit["headers"]) {
    return next({ url: URL_A, headers }, null).problem;
  }

  describe("the URL", () => {
    it("keeps a saved secret URL when left blank, since the screen cannot show it to send back", () => {
      expect(next({}).target?.url).toBe(URL_A);
    });

    it("refuses to turn off Secret on a blank URL, so saving cannot reveal a value the screen has hidden", () => {
      expect(next({ urlSecret: false })).toMatchObject({ target: null, field: "url", problem: expect.stringContaining("again") });
    });

    it("shows a secret URL again once it is entered with Secret off, since the member has just typed it", () => {
      expect(next({ url: URL_A, urlSecret: false }).target).toMatchObject({ url: URL_A, urlSecret: false });
    });

    it("refuses a blank URL when the saved one is not secret, since the form sends it back and blank means it was cleared", () => {
      expect(next({}, { ...saved, urlSecret: false })).toMatchObject({ target: null, field: "url" });
    });

    it("asks for a URL on the first save, since there is nothing to send to yet", () => {
      expect(next({}, null)).toMatchObject({ target: null, field: "url" });
    });

    it("checks a typed URL like any other, so a typo is caught when editing too", () => {
      expect(next({ url: "http://example.com/hook" })).toMatchObject({ target: null, field: "url" });
    });
  });

  describe("the token", () => {
    it("keeps the saved token when left blank on the same URL, so renaming the project does not mean generating a new one", () => {
      expect(next({}).target?.token).toBe("saved-token");
      expect(next({ url: URL_A }).target?.token).toBe("saved-token");
    });

    it("replaces the saved token with a typed one, so a regenerated token can be saved", () => {
      expect(next({ token: "new-token" }).target?.token).toBe("new-token");
    });

    it("removes the token when asked, since blank means keep and a server may stop wanting one", () => {
      expect(next({ token: null }).target?.token).toBe("");
    });

    it("drops the saved token on a new URL, so one service's credential never reaches another", () => {
      expect(next({ url: URL_B }).target?.token).toBe("");
    });

    it("sends a typed token to a new URL, since the member chose to give it to that server", () => {
      expect(next({ url: URL_B, token: "new-token" }).target?.token).toBe("new-token");
    });
  });

  describe("saved headers", () => {
    it("keeps a blank secret value on the same URL under the name's new letter case, since header names ignore case", () => {
      expect(next({ headers: [secret("x-api-key", "")] }).target?.headers).toEqual([secret("x-api-key", "saved-key")]);
    });

    it("replaces a secret value with a typed one", () => {
      expect(next({ headers: [secret("X-Api-Key", "new-key")] }).target?.headers).toEqual([secret("X-Api-Key", "new-key")]);
    });

    it("drops a header left out of the list, so a removed header stops being sent", () => {
      expect(next({ headers: [] }).target?.headers).toEqual([]);
    });

    it("refuses to turn off Secret on a blank value, so saving cannot reveal a value the screen has hidden", () => {
      expect(next({ headers: [plain("X-Api-Key", "")] })).toMatchObject({ target: null, field: "headers", problem: expect.stringContaining("again") });
    });

    it("hides a value once Secret is turned on, even one that was shown before", () => {
      expect(next({ headers: [secret("X-Api-Version", "2")] }).target?.headers).toEqual([secret("X-Api-Version", "2")]);
    });

    it("does not keep a value for a renamed header, so a secret cannot move to a name the member did not enter it under", () => {
      expect(next({ headers: [secret("X-Other", "")] })).toMatchObject({ target: null, field: "headers" });
    });

    it("does not fill a cleared visible value from the saved one, since blank there means it was cleared", () => {
      expect(next({ headers: [plain("X-Api-Version", "")] })).toMatchObject({ target: null, field: "headers" });
      expect(next({ headers: [secret("X-Api-Version", "")] })).toMatchObject({ target: null, field: "headers" });
    });

    it("refuses a blank secret value on a new URL, so one service's key never reaches another", () => {
      expect(next({ url: URL_B, headers: [secret("X-Api-Key", "")] })).toMatchObject({ target: null, field: "headers", problem: expect.stringContaining("new URL") });
    });

    it("carries a visible value to a new URL, since the form sends it back and it is not a secret", () => {
      expect(next({ url: URL_B, headers: [plain("X-Api-Version", "2")] }).target?.headers).toEqual([plain("X-Api-Version", "2")]);
    });

    it("cannot keep a secret from an unreadable or missing target, so nothing is sent with a value that does not exist", () => {
      expect(next({ url: URL_A, headers: [secret("X-Api-Key", "")] }, null)).toMatchObject({ target: null, field: "headers" });
    });
  });

  describe("header rows", () => {
    it("keeps the order, the Secret mark and spaces inside a value, since schemes such as Basic use them", () => {
      expect(next({ headers: [plain("X-Origin", "https://example.com:8443"), secret("Authorization", "Basic abc")], token: null }).target?.headers).toEqual([
        plain("X-Origin", "https://example.com:8443"),
        secret("Authorization", "Basic abc"),
      ]);
    });

    it("ignores an empty row and surrounding spaces, so an unused row or a pasted space does not block the save", () => {
      expect(next({ headers: [secret("", ""), secret(" X-Api-Key ", "  abc  ")] }).target?.headers).toEqual([secret("X-Api-Key", "abc")]);
    });

    it("refuses a value without a name, and says which row, including an HTTP/2 pseudo-header", () => {
      expect(headerProblem(plain("X-Good", "a"), plain("", "b"))).toMatch(/^Header 2:/);
      expect(headerProblem(plain(":authority", "example.com"))).toMatch(/^Header 1:/);
    });

    it("refuses characters a header name cannot use, which fetch would reject on every send", () => {
      for (const name of ["X Api Key", "ヘッダー", "En-tête", "헤더", "标头", "X(Api)", "X-Good\r\nHost"]) expect(headerProblem(plain(name, "a"))).toMatch(/name/);
    });

    it("refuses an empty value or one fetch cannot send, so the failure shows on save and not at every send", () => {
      expect(headerProblem(plain("X-Api-Key", ""))).toMatch(/value/);
      for (const value of ["日本語", "한국어", "中文", "café", "a\tb", "a\r\nHost: evil.example"]) expect(headerProblem(plain("X-Api-Key", value))).toMatch(/value/);
    });

    it("refuses headers that change where or how the request is delivered, whatever their letter case", () => {
      for (const name of ["Host", "content-length", "Content-Type", "TRANSFER-ENCODING", "Connection", "Upgrade", "Proxy-Authorization", "CF-Connecting-IP", "cf-access-jwt-assertion"]) {
        expect(headerProblem(plain(name, "a"))).toContain("cannot be set");
      }
    });

    it("allows names that only begin like a reserved one, so an ordinary header is not blocked by accident", () => {
      expect(headerProblem(plain("Hostname", "a"), plain("X-Host", "b"), plain("TE-Custom", "c"))).toBeNull();
    });

    it("refuses the same header twice in any letter case, since the receiver would get both values joined", () => {
      expect(headerProblem(plain("X-Api-Key", "a"), plain("x-api-key", "b"))).toMatch(/^Header 2:.*twice/);
    });

    it("never repeats a value in its message, since a refused row may hold a secret", () => {
      for (const header of [plain("", "sk-pasted-secret"), plain("X-Api-Key", "sk-pasted-secret\tx"), plain("bad name", "sk-pasted-secret")]) {
        expect(headerProblem(header)).not.toContain("sk-pasted-secret");
      }
    });
  });
});

describe("visibleWebhookTarget", () => {
  const target: WebhookTarget = {
    url: "https://hooks.example.com/services/SECRET-PATH",
    urlSecret: true,
    token: "sk-secret-token",
    headers: [secret("X-Api-Key", "secret-header-value"), plain("X-Api-Version", "2")],
  };

  it("gives the browser nothing marked secret, and never the token, since a page's data can be read by anyone at the screen", () => {
    const visible = JSON.stringify(visibleWebhookTarget(target));
    expect(visible).not.toContain("SECRET-PATH");
    expect(visible).not.toContain("sk-secret-token");
    expect(visible).not.toContain("secret-header-value");
  });

  it("gives the browser what is not secret, so it can be read and edited without typing it again", () => {
    expect(visibleWebhookTarget({ ...target, urlSecret: false })).toEqual({
      url: target.url,
      headers: [secret("X-Api-Key", ""), plain("X-Api-Version", "2")],
    });
  });
});

describe("webhookTargetProblem", () => {
  const base = { url: "https://example.com/hook", urlSecret: true };

  it("refuses a token with an Authorization header in any letter case, since only one of them could be sent", () => {
    expect(webhookTargetProblem({ ...base, token: "t", headers: [secret("authorization", "Basic a")] })).toMatch(/Authorization/);
  });

  it("allows either one alone, so Bearer and other schemes both work", () => {
    expect(webhookTargetProblem({ ...base, token: "t", headers: [secret("X-Api-Key", "k")] })).toBeNull();
    expect(webhookTargetProblem({ ...base, token: "", headers: [secret("Authorization", "Basic a")] })).toBeNull();
  });
});

describe("sealed webhook targets", () => {
  const target = {
    url: "https://hooks.example.com/services/SECRET-PATH",
    urlSecret: true,
    token: "sk-secret-token",
    headers: [secret("X-Api-Key", "secret-header-value")],
  };

  it("cannot be opened without the Secret marks, so a value saved before they existed is not treated as safe to show", async () => {
    const old = { url: target.url, token: target.token, headers: [["X-Api-Key", "secret-header-value"]] };
    const sealed = await sealWebhookTarget(old as unknown as WebhookTarget, SECRET_KEY, "a@example.com");
    await expect(openWebhookTarget(sealed, SECRET_KEY, "a@example.com")).rejects.toThrow();
  });

  it("gives the URL, token and headers back to the cron that sends them", async () => {
    const sealed = await sealWebhookTarget(target, SECRET_KEY, "a@example.com");
    expect(await openWebhookTarget(sealed, SECRET_KEY, "a@example.com")).toEqual(target);
  });

  it("reveals none of the URL, token or header values to someone reading the database", async () => {
    const stored = atob(await sealWebhookTarget(target, SECRET_KEY, "a@example.com"));
    expect(stored).not.toContain("SECRET-PATH");
    expect(stored).not.toContain("sk-secret-token");
    expect(stored).not.toContain("secret-header-value");
  });

  it("cannot be opened for another member, so a row copied to another user does not send with its token", async () => {
    const sealed = await sealWebhookTarget(target, SECRET_KEY, "a@example.com");
    await expect(openWebhookTarget(sealed, SECRET_KEY, "b@example.com")).rejects.toThrow();
  });

  it("cannot be opened with another key, so a leaked database is useless without the Worker secret", async () => {
    const sealed = await sealWebhookTarget(target, SECRET_KEY, "a@example.com");
    await expect(openWebhookTarget(sealed, OTHER_KEY, "a@example.com")).rejects.toThrow();
  });
});
