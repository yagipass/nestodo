import { z } from "zod";

export const analyticsConsentBody = z.object({ enabled: z.boolean() });
