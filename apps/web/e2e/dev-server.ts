import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer, type AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

export type DevServer = { baseUrl: string; triggerToken: string; stateDir: string; stop: () => Promise<void> };

const projectRoot = resolve(import.meta.dirname, "..");
const localDbPointer = join(projectRoot, ".void", "local-d1.json");
const triggerTokenFile = join(projectRoot, ".void", "dev-trigger-token");

function freePort() {
  return new Promise<number>((resolvePort, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, () => {
      const { port } = server.address() as AddressInfo;
      server.close(() => resolvePort(port));
    });
  });
}

async function restoreDefaultLocalDbPointer(stateDir: string) {
  const pointer = await readFile(localDbPointer, "utf-8").then(JSON.parse, () => null);
  if (pointer?.persistRoot === stateDir) await rm(localDbPointer);
}

export async function startDevServer(env: Record<string, string>): Promise<DevServer> {
  const stateDir = await mkdtemp(join(tmpdir(), "nestodo-e2e-"));
  const port = await freePort();
  const baseUrl = `http://localhost:${port}`;
  const child = spawn("pnpm", ["exec", "vp", "dev", "--port", String(port), "--strictPort"], {
    cwd: projectRoot,
    env: { ...process.env, ...env, E2E_PERSIST_TO: stateDir },
    stdio: ["ignore", "pipe", "pipe"],
    detached: true,
  });
  let output = "";
  child.stdout.on("data", (chunk) => (output += chunk));
  child.stderr.on("data", (chunk) => (output += chunk));
  const exited = new Promise<void>((resolveExit) => child.once("exit", () => resolveExit()));

  async function stop() {
    if (child.exitCode === null && child.pid !== undefined) {
      process.kill(-child.pid, "SIGTERM");
      await exited;
    }
    await restoreDefaultLocalDbPointer(stateDir);
    await rm(stateDir, { recursive: true, force: true });
  }

  const deadline = Date.now() + 90_000;
  while (true) {
    if (child.exitCode !== null) {
      await stop();
      throw new Error(`dev server exited before it was ready:\n${output}`);
    }
    if (Date.now() > deadline) {
      await stop();
      throw new Error(`dev server did not become ready in time:\n${output}`);
    }
    const ready = await fetch(baseUrl).then((res) => res.status < 500, () => false);
    if (ready) break;
    await new Promise((wait) => setTimeout(wait, 500));
  }
  try {
    await warmUp(baseUrl, () => output);
  } catch (error) {
    await stop();
    throw error;
  }
  return { baseUrl, triggerToken: (await readFile(triggerTokenFile, "utf-8")).trim(), stateDir, stop };
}

const warmUpRequests: { path: string; method?: string; body?: unknown }[] = [
  { path: "/" },
  { path: "/settings" },
  { path: "/api/v1/todos" },
  { path: "/api/v1/todos/1" },
  { path: "/api/v1/todos", method: "POST", body: {} },
  { path: "/mcp", method: "POST", body: {} },
];

async function warmUp(baseUrl: string, output: () => string) {
  for (const { path, method = "GET", body } of warmUpRequests) {
    for (let attempt = 0; ; attempt++) {
      const res = await fetch(baseUrl + path, {
        method,
        redirect: "manual",
        headers: { Origin: baseUrl, "Content-Type": "application/json", "X-VoidPages": "true", Accept: "application/json" },
        body: body === undefined ? undefined : JSON.stringify(body),
      }).catch(() => null);
      if (res && res.status < 500) break;
      if (attempt >= 20) throw new Error(`dev server kept failing ${method} ${path}:\n${output()}`);
      await new Promise((wait) => setTimeout(wait, 500));
    }
  }
  const reloads = () => output().match(/reload/g)?.length ?? 0;
  let seen = reloads();
  let quietSince = Date.now();
  while (Date.now() - quietSince < 2_000) {
    await new Promise((wait) => setTimeout(wait, 250));
    if (reloads() !== seen) {
      seen = reloads();
      quietSince = Date.now();
    }
  }
}
