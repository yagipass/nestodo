import { getCookie } from "hono/cookie";
import { defineHandler, defineHead, type InferProps } from "void";
import { requireUser } from "../src/access";
import { analyticsConsentOf } from "../src/analytics-service";
import { APPEARANCE_COOKIE, appearanceOf } from "../src/appearance";
import { saveWebhookBody } from "../src/webhook-schemas";
import { deleteWebhook, saveWebhook, webhooksAvailable, webhookSettingsOf } from "../src/webhook-service";

export type Props = InferProps<typeof loader>;

export const loader = defineHandler(async (c) => {
  const user = requireUser(c);
  const [webhook, analytics] = await Promise.all([webhookSettingsOf(user.email), analyticsConsentOf(user.email)]);
  return { available: webhooksAvailable(), webhook, analytics, appearance: appearanceOf(getCookie(c, APPEARANCE_COOKIE)) };
});

export const actions = {
  save: defineHandler.withValidator({ body: saveWebhookBody })(async (c, { body }) => {
    await saveWebhook(requireUser(c).email, body);
  }),
  remove: defineHandler(async (c) => {
    await deleteWebhook(requireUser(c).email);
  }),
};

export const head = defineHead(() => ({ title: "Settings" }));
