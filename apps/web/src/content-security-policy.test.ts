import { createHash } from "node:crypto";
import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vite-plus/test";
import { CONTENT_SECURITY_POLICY } from "./content-security-policy";

function directive(name: string) {
  const found = CONTENT_SECURITY_POLICY.split("; ").find((entry) => entry.startsWith(`${name} `));
  return found?.split(" ").slice(1) ?? [];
}

function inlineScripts(html: string) {
  return [...html.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g)].map(([, body]) => body);
}

describe("CONTENT_SECURITY_POLICY", () => {
  it("allows the script React writes beside a server-rendered form, so a Save clicked before the page loads is not lost", () => {
    const scripts = inlineScripts(renderToString(createElement("form", { action: () => {} })));

    expect(scripts.length).toBeGreaterThan(0);
    for (const script of scripts) {
      expect(directive("script-src")).toContain(`'sha256-${createHash("sha256").update(script).digest("base64")}'`);
    }
  });
});
