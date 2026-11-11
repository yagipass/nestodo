import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { exportJWK, generateKeyPair, type JWK } from "jose";
import type { TestProject } from "vite-plus/test/node";
import { startDevServer } from "./dev-server";

type Access = { teamDomain: string; audience: string; mcpAudience: string; signingKey: JWK };

declare module "vite-plus/test" {
  interface ProvidedContext {
    baseUrl: string;
    triggerToken: string;
    stateDir: string;
    access: Access;
    receiver: { url: string };
  }
}

const AUDIENCE = "e2e-access-app";
const MCP_AUDIENCE = "e2e-mcp-access-app";
const KEY_ID = "e2e";
const WEBHOOK_SECRET_KEY = Buffer.alloc(32, 1).toString("base64");
const ANSWERS: Record<string, { status: number; location?: string }> = {
  "/hook/refuses": { status: 401 },
  "/hook/redirects": { status: 302, location: "/hook/redirected" },
};

async function startAccessCerts() {
  const { publicKey, privateKey } = await generateKeyPair("RS256", { extractable: true });
  const certs = JSON.stringify({ keys: [{ ...(await exportJWK(publicKey)), kid: KEY_ID, alg: "RS256" }] });
  const server = createServer((req, res) => {
    if (req.url === "/cdn-cgi/access/certs") res.writeHead(200, { "Content-Type": "application/json" }).end(certs);
    else res.writeHead(404).end();
  });
  await new Promise<void>((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
  const { port } = server.address() as AddressInfo;
  return {
    teamDomain: `http://127.0.0.1:${port}`,
    signingKey: { ...(await exportJWK(privateKey)), kid: KEY_ID, alg: "RS256" },
    stop: () => new Promise<void>((resolveClose) => server.close(() => resolveClose())),
  };
}

async function startWebhookReceiver() {
  let received: { path: string; authorization?: string; version?: string; apiKey?: string; contentType?: string; body: unknown }[] = [];
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk;
    if (req.method === "POST" && req.url?.startsWith("/hook/")) {
      const { authorization, "x-api-version": version, "x-api-key": apiKey, "content-type": contentType } = req.headers as Record<string, string>;
      received.push({ path: req.url, authorization, version, apiKey, contentType, body: JSON.parse(body) });
      const { status, location } = ANSWERS[req.url] ?? { status: 200 };
      res.writeHead(status, location ? { Location: location } : {}).end();
    } else if (req.method === "GET" && req.url === "/received") {
      res.writeHead(200, { "Content-Type": "application/json" }).end(JSON.stringify(received));
    } else if (req.method === "DELETE" && req.url === "/received") {
      received = [];
      res.writeHead(204).end();
    } else {
      res.writeHead(404).end();
    }
  });
  await new Promise<void>((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
  const { port } = server.address() as AddressInfo;
  return {
    url: `http://127.0.0.1:${port}`,
    stop: () => new Promise<void>((resolveClose) => server.close(() => resolveClose())),
  };
}

export default async function setup(project: TestProject) {
  const certs = await startAccessCerts();
  const receiver = await startWebhookReceiver();
  const access: Access = {
    teamDomain: certs.teamDomain,
    audience: AUDIENCE,
    mcpAudience: MCP_AUDIENCE,
    signingKey: certs.signingKey,
  };
  const server = await startDevServer({
    ACCESS_TEAM_DOMAIN: access.teamDomain,
    ACCESS_AUD: access.audience,
    ACCESS_MCP_AUD: access.mcpAudience,
    WEBHOOK_SECRET_KEY,
    VITE_POSTHOG_PROJECT_TOKEN: "phc_e2e",
  }).catch(async (error) => {
    await certs.stop();
    await receiver.stop();
    throw error;
  });
  project.provide("baseUrl", server.baseUrl);
  project.provide("triggerToken", server.triggerToken);
  project.provide("stateDir", server.stateDir);
  project.provide("access", access);
  project.provide("receiver", { url: receiver.url });
  return async () => {
    await server.stop();
    await certs.stop();
    await receiver.stop();
  };
}
