import { randomUUID } from "node:crypto";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { deflateSync } from "node:zlib";
import { importJWK, SignJWT, type JWK } from "jose";
import { expect, inject } from "vite-plus/test";
import { PAGE_HEADERS, pageData } from "./pages-client";

export type Todo = {
  id: number;
  parentId: number | null;
  position: number;
  title: string;
  icon: string | null;
  completedAt: string | null;
  collapsed: boolean;
  createdAt: string;
  commentCount: number;
};

export type Comment = { id: number; body: string; via: "app" | "mcp"; at: string; editedAt: string | null };

export type TimelineItem = { type: "event"; kind: "created" | "completed" | "reopened"; at: string } | ({ type: "comment" } & Comment);

export type Day = { date: string; added: number; completed: number; open: number };

export type ArchivedTodo = { path: string[]; title: string; createdAt: string; completedAt: string };

export type WebhookHeader = { name: string; value: string; secret: boolean };

export type Profile = { email: string; name: string | null; avatarUrl: string | null };

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
        "SELECT path, title, created_at AS createdAt, completed_at AS completedAt FROM archived_todos WHERE user_id = ? ORDER BY id",
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

export function reopenDirectly(id: number) {
  const database = localDatabase({ readOnly: false });
  try {
    database.prepare("UPDATE todos SET completed_at = NULL WHERE id = ?").run(id);
  } finally {
    database.close();
  }
}

export function archiveDirectly(id: number, email: string) {
  const database = localDatabase({ readOnly: false });
  try {
    database
      .prepare("INSERT INTO archived_todos (id, user_id, path, title, created_at, completed_at) VALUES (?, ?, '[]', '', '', '')")
      .run(id, email);
  } finally {
    database.close();
  }
}

export function storedComments(email: string) {
  const database = localDatabase({ readOnly: true });
  try {
    return database.prepare("SELECT todo_id AS todoId, body FROM todo_comments WHERE user_id = ? ORDER BY id").all(email) as { todoId: number; body: string }[];
  } finally {
    database.close();
  }
}

export function trashedTodoIds(email: string) {
  const database = localDatabase({ readOnly: true });
  try {
    return (database.prepare("SELECT id FROM trashed_todos WHERE user_id = ? ORDER BY id").all(email) as { id: number }[]).map((row) => row.id);
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

function crc32(data: Buffer) {
  let crc = ~0;
  for (const byte of data) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return ~crc >>> 0;
}

function chunk(type: string, data: Buffer) {
  const typed = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(typed));
  return Buffer.concat([length, typed, crc]);
}

export function png([red, green, blue]: [number, number, number]) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(1, 0);
  header.writeUInt32BE(1, 4);
  header.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(Buffer.from([0, red, green, blue]))),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

export type ImageIcon = { id: string; name: string; url: string };

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
    if (init.page) Object.assign(headers, PAGE_HEADERS);
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
    return pageData<{ props: { todos: Todo[]; icons: ImageIcon[]; showDone: boolean } }>(await res.text()).props;
  }

  async todos(): Promise<Todo[]> {
    const res = await this.request("/api/v1/todos");
    expect(res.status).toBe(200);
    return (await json<{ todos: Todo[] }>(res)).todos;
  }

  async createTodo(title: string, fields: { comment?: string; icon?: string; parentId?: number | null; index?: number } = {}): Promise<Todo> {
    const res = await this.request("/api/v1/todos", { method: "POST", body: { title, ...fields } });
    expect(res.status).toBe(201);
    return (await json<{ todo: Todo }>(res)).todo;
  }

  async patch(id: number, body: Record<string, unknown>) {
    return this.request(`/api/v1/todos/${id}`, { method: "PATCH", body });
  }

  async timeline(id: number): Promise<TimelineItem[]> {
    const res = await this.request(`/api/v1/todos/${id}/timeline`);
    expect(res.status).toBe(200);
    return (await json<{ items: TimelineItem[] }>(res)).items;
  }

  async comment(id: number, body: string): Promise<Comment> {
    const res = await this.request(`/api/v1/todos/${id}/comments`, { method: "POST", body: { body } });
    expect(res.status).toBe(201);
    return (await json<{ comment: Comment }>(res)).comment;
  }

  editComment(id: number, body: unknown) {
    return this.request(`/api/v1/comments/${id}`, { method: "PATCH", body: { body } });
  }

  deleteComment(id: number) {
    return this.request(`/api/v1/comments/${id}`, { method: "DELETE" });
  }

  restore(id: number, body: { parentId?: number | null; index?: number } = {}) {
    return this.request(`/api/v1/todos/${id}/restore`, { method: "POST", body });
  }

  async search(query: string) {
    const res = await this.request(`/api/v1/search?q=${encodeURIComponent(query)}`);
    expect(res.status).toBe(200);
    return (await json<{ todos: { id: number; comment: string }[] }>(res)).todos;
  }

  async stats(timeZone: string) {
    const res = await this.request(`/stats?tz=${encodeURIComponent(timeZone)}`, { page: true });
    expect(res.status).toBe(200);
    return pageData<{ props: { timeZone: string; days: Day[] } }>(await res.text()).props;
  }

  async settings(cookie?: string) {
    const res = await this.request("/settings", { page: true, cookie });
    expect(res.status).toBe(200);
    return pageData<{ props: { available: boolean; webhook: WebhookSettings | null; analytics: boolean; appearance: string } }>(await res.text()).props;
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

  apiWebhook() {
    return this.request("/api/v1/webhook", { origin: null });
  }

  apiSaveWebhook(body: { url: string; urlSecret?: boolean; token: string | null; headers?: WebhookHeader[]; project: string }) {
    return this.request("/api/v1/webhook", { method: "PUT", origin: null, body: { urlSecret: true, headers: [], ...body } });
  }

  apiRemoveWebhook() {
    return this.request("/api/v1/webhook", { method: "DELETE", origin: null });
  }

  async profile() {
    const res = await this.request("/api/v1/profile", { origin: null });
    expect(res.status).toBe(200);
    return json<Profile>(res);
  }

  async pageProfile() {
    const res = await this.request("/settings", { page: true });
    expect(res.status).toBe(200);
    return pageData<{ shared: { auth: { profile: Profile | null } } }>(await res.text()).shared.auth.profile;
  }

  saveName(name: unknown) {
    return this.request("/api/v1/profile", { method: "PUT", origin: null, body: { name } });
  }

  uploadAvatar(image: Uint8Array | string) {
    return this.request("/api/v1/profile/avatar", {
      method: "PUT",
      origin: null,
      body: { image: typeof image === "string" ? image : Buffer.from(image).toString("base64") },
    });
  }

  removeAvatar() {
    return this.request("/api/v1/profile/avatar", { method: "DELETE", origin: null });
  }

  avatar(url: string) {
    return this.request(url, { origin: null });
  }

  async images() {
    const res = await this.request("/api/v1/icons");
    expect(res.status).toBe(200);
    return (await json<{ icons: ImageIcon[] }>(res)).icons;
  }

  addImage(name: unknown, image: Uint8Array | string = png([200, 80, 160])) {
    return this.request("/api/v1/icons", {
      method: "POST",
      body: { name, image: typeof image === "string" ? image : Buffer.from(image).toString("base64") },
    });
  }

  async addedImage(name: string, image?: Uint8Array) {
    const res = await this.addImage(name, image);
    expect(res.status).toBe(201);
    return (await json<{ icon: ImageIcon }>(res)).icon;
  }

  renameImage(id: string, name: unknown) {
    return this.request(`/api/v1/icons/${id}`, { method: "PATCH", body: { name } });
  }

  deleteImage(id: string) {
    return this.request(`/api/v1/icons/${id}`, { method: "DELETE" });
  }

  image(url: string) {
    return this.request(url);
  }
}

export async function signedInUser(email = `${randomUUID()}@e2e.test`) {
  return new Session(await accessAssertion({ email }));
}
