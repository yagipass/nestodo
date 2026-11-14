import { getCookie } from "hono/cookie";
import { defineMiddleware } from "void";
import { APPEARANCE_COOKIE, appearanceOf } from "../src/appearance";

export default defineMiddleware(async (c, next) => {
  const appearance = appearanceOf(getCookie(c, APPEARANCE_COOKIE));
  if (appearance !== "system") c.set("headDefaults", { htmlAttrs: { "data-appearance": appearance } });
  await next();
});
