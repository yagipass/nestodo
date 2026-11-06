import { randomUUID } from "node:crypto";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { importJWK, SignJWT, type JWK } from "jose";
import { expect, inject } from "vite-plus/test";

export type Todo = {
  id: number;
  parentId: number | null;
  position: number;
  title: string;
  note: string;
  completedAt: string | null;
  collapsed: boolean;
  createdAt: string;
};

export type Day = { date: string; added: number; completed: number; open: number };

export type ArchivedTodo = { path: string[]; title: string; note: string; createdAt: string; completedAt: string };

export type WebhookHeader = { name: string; value: string; secret: boolean };

export type WebhookSettings = {
  url: string | null;
  hasToken: boolean;
  unreadable: boolean;
  headers: WebhookHeader[];
  project: string;
  lastSentAt: string | null;
  lastError: string | null;
};

const baseUrl = inject("baseUrl");
const triggerToken = inject("triggerToken");
const stateDir = inject("stateDir");
const access = inject("access");

export async function json<T>(res: Response) {
  return (await res.json()) as T;
}

export function triggerCron(cron: string, at: Date) {
  return fetch(`${baseUrl}/__void/scheduled`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "x-void-dev-trigger": triggerToken },
    body: JSON.stringify({ cron, scheduledTime: at.getTime() }),
  });
}

export async function runCron(cron: string, at: Date) {
  expect(await (await triggerCron(cron, at)).json()).toEqual({ ok: true });
}

function localDatabase(options: { readOnly: boolean }) {
  const dir = join(stateDir, "v3", "d1", "miniflare-D1DatabaseObject");
  const [file] = readdirSync(dir).filter((name) => name.endsWith(".sqlite") && name !== "metadata.sqlite");
  return new DatabaseSync(join(dir, file), options);
}

export function archivedTodos(email: string): ArchivedTodo[] {
  const database = localDatabase({ readOnly: true });
  try {
    const rows = database
      .prepare(
        "SELECT path, title, note, created_at AS createdAt, completed_at AS completedAt FROM archived_todos WHERE user_id = ? ORDER BY id",
      )
      .all(email) as (Omit<ArchivedTodo, "path"> & { path: string })[];
    return rows.map((row) => ({ ...row, path: JSON.parse(row.path) }));
  } finally {
    database.close();
  }
}

export function setParentDirectly(id: number, parentId: number) {
  const database = localDatabase({ readOnly: false });
  try {
    database.prepare("UPDATE todos SET parent_id = ? WHERE id = ?").run(parentId, id);
  } finally {
    database.close();
  }
}

export function archiveDirectly(id: number, email: string) {
  const database = localDatabase({ readOnly: false });
  try {
    database
      .prepare("INSERT INTO archived_todos (id, user_id, path, title, note, created_at, completed_at) VALUES (?, ?, '[]', '', '', '', '')")
      .run(id, email);
  } finally {
    database.close();
  }
}

export function deleteAllWebhooks() {
  const database = localDatabase({ readOnly: false });
  try {
    database.prepare("DELETE FROM webhooks").run();
  } finally {
    database.close();
  }
}

export function corruptStoredWebhook(email: string) {
  const database = localDatabase({ readOnly: false });
  try {
    database.prepare("UPDATE webhooks SET target = ? WHERE user_id = ?").run(btoa("not what was sealed"), email);
  } finally {
    database.close();
  }
}

export function storedWebhook(email: string) {
  const database = localDatabase({ readOnly: true });
  try {
    return JSON.stringify(database.prepare("SELECT * FROM webhooks WHERE user_id = ?").get(email));
  } finally {
    database.close();
  }
}

type AssertionOptions = {
  email?: string;
  audience?: string;
  issuer?: string;
  expiresAt?: number | string;
  signingKey?: JWK;
};

export async function accessAssertion({
  email,
  audience = access.audience,
  issuer = access.teamDomain,
  expiresAt = "5m",
  signingKey = access.signingKey,
}: AssertionOptions) {
  return new SignJWT(email === undefined ? {} : { email })
    .setProtectedHeader({ alg: "RS256", kid: signingKey.kid })
    .setIssuer(issuer)
    .setAudience(audience)
    .setIssuedAt()
    .setExpirationTime(expiresAt)
    .sign(await importJWK(signingKey, "RS256"));
}

export class Session {
  constructor(private assertion?: string) {}

  async request(
    pathOrUrl: string,
    init: { method?: string; body?: unknown; contentType?: string; page?: boolean; origin?: string | null; secFetchSite?: string; cookie?: string } = {},
  ) {
    const headers: Record<string, string> = {};
    if (init.origin !== null) headers.Origin = init.origin ?? baseUrl;
    if (init.secFetchSite) headers["Sec-Fetch-Site"] = init.secFetchSite;
    if (this.assertion) headers["Cf-Access-Jwt-Assertion"] = this.assertion;
    if (init.cookie) headers.Cookie = init.cookie;
    if (init.page) {
      headers["X-VoidPages"] = "true";
      headers.Accept = "application/json";
    }
    if (init.body !== undefined) headers["Content-Type"] = init.contentType ?? "application/json";
    const url = pathOrUrl.startsWith("http") ? pathOrUrl : baseUrl + pathOrUrl;
    return fetch(url, {
      method: init.method ?? "GET",
      headers,
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      redirect: "manual",
    });
  }

  async page(cookie?: string) {
    const res = await this.request("/", { page: true, cookie });
    expect(res.status).toBe(200);
    return (await json<{ props: { todos: Todo[]; showDone: boolean } }>(res)).props;
  }

  async todos(): Promise<Todo[]> {
    const res = await this.request("/api/v1/todos");
    expect(res.status).toBe(200);
    return (await json<{ todos: Todo[] }>(res)).todos;
  }

  async createTodo(title: string, fields: { note?: string; parentId?: number | null; index?: number } = {}): Promise<Todo> {
    const res = await this.request("/api/v1/todos", { method: "POST", body: { title, ...fields } });
    expect(res.status).toBe(201);
    return (await json<{ todo: Todo }>(res)).todo;
  }

  async patch(id: number, body: Record<string, unknown>) {
    return this.request(`/api/v1/todos/${id}`, { method: "PATCH", body });
  }

  async stats(timeZone: string) {
    const res = await this.request(`/stats?tz=${encodeURIComponent(timeZone)}`, { page: true });
    expect(res.status).toBe(200);
    return (await json<{ props: { timeZone: string; days: Day[] } }>(res)).props;
  }

  async settings() {
    const res = await this.request("/settings", { page: true });
    expect(res.status).toBe(200);
    return (await json<{ props: { available: boolean; webhook: WebhookSettings | null; analytics: boolean } }>(res)).props;
  }

  async analytics() {
    const res = await this.request("/api/v1/analytics");
    expect(res.status).toBe(200);
    return (await json<{ enabled: boolean }>(res)).enabled;
  }

  setAnalytics(enabled: unknown) {
    return this.request("/api/v1/analytics", { method: "PUT", body: { enabled } });
  }

  saveWebhook(body: { url: string; urlSecret?: boolean; token: string | null; headers?: WebhookHeader[]; project: string }) {
    return this.request("/settings?save", { method: "POST", page: true, body: { urlSecret: true, headers: [], ...body } });
  }

  removeWebhook() {
    return this.request("/settings?remove", { method: "DELETE", page: true });
  }
}

export async function signedInUser(email = `${randomUUID()}@e2e.test`) {
  return new Session(await accessAssertion({ email }));
}
