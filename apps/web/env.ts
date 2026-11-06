import { defineEnv, string, url } from "void/env";

export default defineEnv({
  ACCESS_TEAM_DOMAIN: url().optional(),
  ACCESS_AUD: string().optional(),
  ACCESS_MCP_AUD: string().optional(),
  WEBHOOK_SECRET_KEY: string().optional(),
  VITE_POSTHOG_PROJECT_TOKEN: string().optional(),
});
