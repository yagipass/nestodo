import { env as bindings } from "cloudflare:workers";
import { HTTPException } from "hono/http-exception";
import { db, eq } from "void/db";
import { env } from "void/env";
import { ValidationError } from "void/handler";
import { webhooks } from "@schema";
import { sqliteTime } from "./time";
import { webhookCandidatesOf } from "./todo-service";
import { todosAwaitingNote } from "./todos";
import { nextWebhookTarget, openWebhookTarget, sealWebhookTarget, visibleWebhookTarget, webhookRequest, webhookTargetProblem, type WebhookEdit, type WebhookTarget } from "./webhooks";

const SEND_TIMEOUT = 10_000;

type Webhook = typeof webhooks.$inferSelect;

export type WebhookInput = WebhookEdit & { project: string };

const settingsSql =
  "SELECT target, has_token AS hasToken, header_names AS headerNames, project, last_sent_at AS lastSentAt, last_error AS lastError FROM webhooks WHERE user_id = ?";

type SettingsRow = Pick<Webhook, "target" | "project" | "lastSentAt" | "lastError"> & { hasToken: number; headerNames: string };

function savedRowOf(userId: string) {
  return bindings.DB.withSession("first-primary").prepare(settingsSql).bind(userId).first<SettingsRow>();
}

export function webhooksAvailable() {
  return Boolean(env.WEBHOOK_SECRET_KEY);
}

function savedTargetOf(row: SettingsRow | null, userId: string) {
  if (!row || !env.WEBHOOK_SECRET_KEY) return null;
  return openWebhookTarget(row.target, env.WEBHOOK_SECRET_KEY, userId).catch(() => null);
}

export async function webhookSettingsOf(userId: string) {
  const row = await savedRowOf(userId);
  if (!row) return null;
  const { project, lastSentAt, lastError } = row;
  const target = await savedTargetOf(row, userId);
  const visible = target
    ? visibleWebhookTarget(target)
    : { url: null, headers: (JSON.parse(row.headerNames) as string[]).map((name) => ({ name, secret: true, value: "" })) };
  return { hasToken: row.hasToken !== 0, unreadable: target === null, ...visible, project, lastSentAt, lastError };
}

function secretKey() {
  if (env.WEBHOOK_SECRET_KEY) return env.WEBHOOK_SECRET_KEY;
  const status = 409;
  throw new HTTPException(status, {
    res: Response.json({ error: "webhooks_unavailable", message: "WEBHOOK_SECRET_KEY is not set on this server" }, { status }),
  });
}

export async function saveWebhook(userId: string, { project, ...edit }: WebhookInput) {
  const key = secretKey();
  const row = await savedRowOf(userId);
  const saved = await savedTargetOf(row, userId);
  if (row && !saved && edit.url === "") throw new ValidationError({ url: "The saved URL can no longer be read. Enter it again" });
  const { target, field, problem } = nextWebhookTarget(edit, saved, { allowHttp: import.meta.env.DEV });
  if (target === null) throw new ValidationError({ [field]: problem });
  const conflict = webhookTargetProblem(target);
  if (conflict) throw new ValidationError({ headers: conflict });
  const fields = {
    target: await sealWebhookTarget(target, key, userId),
    hasToken: target.token !== "",
    headerNames: target.headers.map(({ name }) => name),
    project,
    lastSentAt: null,
    lastError: null,
  };
  await db
    .insert(webhooks)
    .values({ userId, ...fields })
    .onConflictDoUpdate({ target: webhooks.userId, set: fields });
}

export async function deleteWebhook(userId: string) {
  await db.delete(webhooks).where(eq(webhooks.userId, userId));
}

async function attempt(webhook: Webhook, waiting: Parameters<typeof webhookRequest>[2]) {
  let target: WebhookTarget;
  try {
    target = await openWebhookTarget(webhook.target, secretKey(), webhook.userId);
  } catch {
    return "The saved URL and token could not be read. Save them again";
  }
  try {
    const res = await fetch(target.url, { ...webhookRequest(target, webhook.project, waiting), signal: AbortSignal.timeout(SEND_TIMEOUT) });
    return res.ok ? null : `The server answered ${res.status}`;
  } catch (error) {
    return error instanceof Error && error.name === "TimeoutError" ? "The server did not answer in time" : "The server could not be reached";
  }
}

async function sendTo(webhook: Webhook, now: Date) {
  const waiting = todosAwaitingNote(await webhookCandidatesOf(webhook.userId, now), webhook.project, now);
  if (waiting.length === 0) return null;
  const lastError = await attempt(webhook, waiting);
  await db
    .update(webhooks)
    .set({ lastSentAt: sqliteTime(now.getTime()), lastError })
    .where(eq(webhooks.userId, webhook.userId));
  return lastError;
}

export async function sendWaitingTodos(now: Date) {
  const rows = await db.select().from(webhooks);
  const results = await Promise.allSettled(rows.map((webhook) => sendTo(webhook, now)));
  const failures = results.flatMap((result) => {
    if (result.status === "rejected") return [String(result.reason)];
    return result.value === null ? [] : [result.value];
  });
  if (failures.length > 0) throw new Error(`Webhook delivery failed for ${failures.length} of ${rows.length} members: ${failures.join("; ")}`);
}
