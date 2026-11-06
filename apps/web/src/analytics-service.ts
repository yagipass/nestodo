import { env } from "cloudflare:workers";
import { db, eq } from "void/db";
import { analyticsConsents } from "@schema";

export async function analyticsConsentOf(userId: string) {
  const row = await env.DB.withSession("first-primary").prepare("SELECT 1 FROM analytics_consents WHERE user_id = ?").bind(userId).first();
  return row !== null;
}

export async function setAnalyticsConsent(userId: string, enabled: boolean) {
  if (enabled) await db.insert(analyticsConsents).values({ userId }).onConflictDoNothing();
  else await db.delete(analyticsConsents).where(eq(analyticsConsents.userId, userId));
}
