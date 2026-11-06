import type { CloudContext } from "void";
import { and, count, db, eq, gte, isNull, sql } from "void/db";
import { todoEvents, todos } from "@schema";
import { requireUser } from "./access";
import { activityWindow, dailyActivity, isTimeZone } from "./stats";

export async function statsFor(c: CloudContext) {
  const user = requireUser(c);
  const cf = c.req.raw.cf as { timezone?: unknown } | undefined;
  const timeZone = [c.req.query("tz"), cf?.timezone].find(isTimeZone) ?? "UTC";
  return { timeZone, days: await activityOf(user.email, timeZone, new Date()) };
}

export async function activityOf(userId: string, timeZone: string, now: Date) {
  const { today, since, modifier } = activityWindow(timeZone, now);
  const date = sql<string>`date(${todoEvents.at}, ${modifier})`;
  const events = (kind: (typeof todoEvents.kind.enumValues)[number]) => sql<number>`sum(${todoEvents.kind} = ${kind})`.as(kind);
  const [counts, [{ open }]] = await db.batch([
    db
      .select({
        date: date.as("date"),
        created: events("created"),
        completed: events("completed"),
        reopened: events("reopened"),
        deleted: events("deleted"),
      })
      .from(todoEvents)
      .where(and(eq(todoEvents.userId, userId), gte(todoEvents.at, since)))
      .groupBy(date),
    db
      .select({ open: count() })
      .from(todos)
      .where(and(eq(todos.userId, userId), isNull(todos.completedAt))),
  ]);
  return dailyActivity(counts, today, open);
}
