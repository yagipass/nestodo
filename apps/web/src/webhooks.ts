import { z } from "zod";

export const WEBHOOK_EVENT = "todos.awaiting_note";

export type WebhookHeader = { name: string; value: string; secret: boolean };

export type WebhookTarget = { url: string; urlSecret: boolean; token: string; headers: WebhookHeader[] };

export type WebhookEdit = Omit<WebhookTarget, "token"> & { token: string | null };

type WebhookField = "url" | "headers";

type NextTarget = { target: WebhookTarget; field: null; problem: null } | { target: null; field: WebhookField; problem: string };

type Waiting = { id: number; title: string; note: string };

const HEADER_NAME = /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/;
const HEADER_VALUE = /^[\x20-\x7e]+$/;
const RESERVED_HEADER = /^(connection|content-length|content-type|host|keep-alive|te|trailer|transfer-encoding|upgrade|proxy-.+|cf-.+)$/;

const storedTarget = z.object({
  url: z.string(),
  urlSecret: z.boolean(),
  token: z.string(),
  headers: z.array(z.object({ name: z.string(), value: z.string(), secret: z.boolean() })),
});

export function webhookUrlProblem(value: string, { allowHttp }: { allowHttp: boolean }) {
  if (!URL.canParse(value)) return "Enter a full URL, such as https://example.com/hook";
  const { protocol, username, password } = new URL(value);
  if (protocol !== "https:" && !(allowHttp && protocol === "http:")) return "The URL must start with https://";
  if (username || password) return "Put the credentials in the token field, not in the URL";
  return null;
}

function nextHeaders(rows: readonly WebhookHeader[], saved: WebhookTarget | null, sameUrl: boolean) {
  const headers: WebhookHeader[] = [];
  const seen = new Set<string>();
  for (const [index, row] of rows.entries()) {
    const name = row.name.trim();
    const value = row.value.trim();
    if (name === "" && value === "") continue;
    const refuse = (reason: string) => ({ headers: null, problem: `Header ${index + 1}: ${reason}` });
    const lower = name.toLowerCase();
    if (!HEADER_NAME.test(name)) return refuse("the name is empty or has a character a header cannot use");
    if (RESERVED_HEADER.test(lower)) return refuse(`${name} cannot be set here`);
    if (seen.has(lower)) return refuse(`${name} is listed twice`);
    seen.add(lower);
    if (value === "") {
      const kept = saved?.headers.find((header) => header.secret && header.name.toLowerCase() === lower);
      if (!kept) return refuse("enter a value");
      if (!row.secret) return refuse("enter the value again to turn off Secret, since a saved secret cannot be shown");
      if (!sameUrl) return refuse("enter the value again or remove the header, since a saved secret is not sent to a new URL");
      headers.push({ name, value: kept.value, secret: true });
      continue;
    }
    if (!HEADER_VALUE.test(value)) return refuse("the value must be visible ASCII characters");
    headers.push({ name, value, secret: row.secret });
  }
  return { headers, problem: null };
}

export function nextWebhookTarget(edit: WebhookEdit, saved: WebhookTarget | null, { allowHttp }: { allowHttp: boolean }): NextTarget {
  const refuse = (field: WebhookField, problem: string) => ({ target: null, field, problem });
  if (edit.url === "") {
    if (!saved?.urlSecret) return refuse("url", "Enter the URL to send to");
    if (!edit.urlSecret) return refuse("url", "Enter the URL again to turn off Secret, since a saved secret cannot be shown");
  } else {
    const problem = webhookUrlProblem(edit.url, { allowHttp });
    if (problem) return refuse("url", problem);
  }
  const url = edit.url || (saved?.url ?? "");
  const sameUrl = url === saved?.url;
  const { headers, problem } = nextHeaders(edit.headers, saved, sameUrl);
  if (headers === null) return refuse("headers", problem);
  const keptToken = sameUrl ? saved.token : "";
  const token = edit.token === null ? "" : edit.token || keptToken;
  return { target: { url, urlSecret: edit.urlSecret, token, headers }, field: null, problem: null };
}

export function webhookTargetProblem({ token, headers }: WebhookTarget) {
  const both = token !== "" && headers.some(({ name }) => name.toLowerCase() === "authorization");
  return both ? "Use either the token or an Authorization header, not both" : null;
}

export function visibleWebhookTarget({ url, urlSecret, headers }: WebhookTarget) {
  return {
    url: urlSecret ? null : url,
    headers: headers.map(({ name, value, secret }) => ({ name, secret, value: secret ? "" : value })),
  };
}

export function webhookRequest({ token, headers: listed }: WebhookTarget, project: string, waiting: readonly Waiting[]) {
  const headers = new Headers(listed.map(({ name, value }): [string, string] => [name, value]));
  headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);
  return {
    method: "POST",
    redirect: "manual",
    headers,
    body: JSON.stringify({
      event: WEBHOOK_EVENT,
      project,
      todos: waiting.map(({ id, title, note }) => ({ id, title, note })),
    }),
  } satisfies RequestInit;
}

const encoder = new TextEncoder();
const IV_LENGTH = 12;

function secretKeyFrom(base64: string) {
  const raw = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
  return crypto.subtle.importKey("raw", raw, "AES-GCM", false, ["encrypt", "decrypt"]);
}

export async function sealWebhookTarget(target: WebhookTarget, secretKey: string, userId: string) {
  const iv = crypto.getRandomValues(new Uint8Array(IV_LENGTH));
  const sealed = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv, additionalData: encoder.encode(userId) },
    await secretKeyFrom(secretKey),
    encoder.encode(JSON.stringify(target)),
  );
  return btoa(String.fromCharCode(...iv, ...new Uint8Array(sealed)));
}

export async function openWebhookTarget(sealed: string, secretKey: string, userId: string): Promise<WebhookTarget> {
  const bytes = Uint8Array.from(atob(sealed), (char) => char.charCodeAt(0));
  const opened = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: bytes.slice(0, IV_LENGTH), additionalData: encoder.encode(userId) },
    await secretKeyFrom(secretKey),
    bytes.slice(IV_LENGTH),
  );
  return storedTarget.parse(JSON.parse(new TextDecoder().decode(opened)));
}
