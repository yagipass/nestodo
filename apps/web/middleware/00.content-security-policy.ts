import { defineMiddleware } from "void";
import { CONTENT_SECURITY_POLICY } from "../src/content-security-policy";

export default defineMiddleware(async (c, next) => {
  await next();
  if (!import.meta.env.DEV) c.header("Content-Security-Policy", CONTENT_SECURITY_POLICY);
});
