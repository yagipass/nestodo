import { defineHandler } from "void";
import { requireUser } from "../../../src/access";
import { analyticsConsentBody } from "../../../src/analytics-schemas";
import { analyticsConsentOf, setAnalyticsConsent } from "../../../src/analytics-service";

export const GET = defineHandler(async (c) => ({ enabled: await analyticsConsentOf(requireUser(c).email) }));

export const PUT = defineHandler.withValidator({ body: analyticsConsentBody })(async (c, { body }) => {
  await setAnalyticsConsent(requireUser(c).email, body.enabled);
  return { enabled: body.enabled };
});
