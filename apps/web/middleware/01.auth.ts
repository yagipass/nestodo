import { defineMiddleware } from "void";
import { resolveAccessUser } from "../src/access";

const SAFE_METHODS = ["GET", "HEAD", "OPTIONS"];
const SAME_ORIGIN_FETCH_SITES = ["same-origin", "none"];
const BODY_METHODS = ["POST", "PUT", "PATCH"];

function isJson(contentType: string | undefined) {
  return contentType?.split(";", 1)[0].trim().toLowerCase() === "application/json";
}

function isCrossOrigin(origin: string | undefined, fetchSite: string | undefined, url: string) {
  if (origin) return origin !== new URL(url).origin;
  return fetchSite !== undefined && !SAME_ORIGIN_FETCH_SITES.includes(fetchSite);
}

export default defineMiddleware(async (c, next) => {
  if (!SAFE_METHODS.includes(c.req.method) && isCrossOrigin(c.req.header("origin"), c.req.header("sec-fetch-site"), c.req.url)) {
    await c.req.raw.arrayBuffer();
    return c.json({ error: "forbidden", message: "Cross-origin request rejected" }, 403);
  }
  if (c.req.path.startsWith("/api/") && BODY_METHODS.includes(c.req.method) && !isJson(c.req.header("content-type"))) {
    await c.req.raw.arrayBuffer();
    return c.json({ error: "unsupported_media_type", message: "Send the body as JSON with Content-Type: application/json" }, 415);
  }

  const user = await resolveAccessUser(c.req.header("cf-access-jwt-assertion"), { mcp: c.req.path === "/mcp" });
  c.set("accessUser", user);
  c.set("shared", { auth: { user } });
  await next();
  if (user) c.header("Cache-Control", "no-store");
});
