import { defineHandler } from "void";
import { ValidationError } from "void/handler";
import { requireUser } from "../../../src/access";
import { saveWebhookBody } from "../../../src/webhook-schemas";
import { deleteWebhook, saveWebhook, webhooksAvailable, webhookSettingsOf } from "../../../src/webhook-service";

async function settingsOf(email: string) {
  return { available: webhooksAvailable(), webhook: await webhookSettingsOf(email) };
}

function bodyIssues({ errors }: ValidationError) {
  const issues = Object.entries(errors).map(([path, message]) => ({ message, path }));
  return { error: "Validation failed", issues: [{ slot: "body", issues }] };
}

export const GET = defineHandler((c) => settingsOf(requireUser(c).email));

export const PUT = defineHandler.withValidator({ body: saveWebhookBody })(async (c, { body }) => {
  const { email } = requireUser(c);
  try {
    await saveWebhook(email, body);
  } catch (error) {
    if (error instanceof ValidationError) return c.json(bodyIssues(error), 400);
    throw error;
  }
  return c.json(await settingsOf(email));
});

export const DELETE = defineHandler(async (c) => {
  await deleteWebhook(requireUser(c).email);
  return c.body(null, 204);
});
