import { HTTPException } from "hono/http-exception";
import { createRemoteJWKSet, errors, jwtVerify } from "jose";
import type { CloudContext } from "void";
import { env } from "void/env";

export type AccessUser = { email: string };

declare module "void" {
  interface CloudContextVariables {
    accessUser: AccessUser | null;
    shared: { auth: { user: AccessUser | null } };
  }
}

const DEV_USER: AccessUser = { email: "dev@localhost" };

const INVALID_ASSERTION_ERRORS = [
  errors.JWTExpired,
  errors.JWTClaimValidationFailed,
  errors.JWSSignatureVerificationFailed,
  errors.JWSInvalid,
  errors.JWTInvalid,
  errors.JWKSNoMatchingKey,
  errors.JOSEAlgNotAllowed,
  errors.JOSENotSupported,
];

const keySets = new Map<string, ReturnType<typeof createRemoteJWKSet>>();

function keySetFor(issuer: string) {
  let keySet = keySets.get(issuer);
  if (!keySet) {
    keySet = createRemoteJWKSet(new URL("/cdn-cgi/access/certs", issuer));
    keySets.set(issuer, keySet);
  }
  return keySet;
}

export function userIdFor(email: string) {
  return email.toLowerCase();
}

async function verifyAssertion(assertion: string, teamDomain: string, audience: string[]): Promise<AccessUser | null> {
  const issuer = new URL(teamDomain).origin;
  try {
    const { payload } = await jwtVerify(assertion, keySetFor(issuer), { issuer, audience, algorithms: ["RS256"] });
    return typeof payload.email === "string" ? { email: userIdFor(payload.email) } : null;
  } catch (error) {
    if (INVALID_ASSERTION_ERRORS.some((type) => error instanceof type)) return null;
    throw error;
  }
}

export async function resolveAccessUser(assertion: string | undefined, { mcp }: { mcp: boolean }): Promise<AccessUser | null> {
  if (!env.ACCESS_TEAM_DOMAIN || !env.ACCESS_AUD) {
    if (import.meta.env.DEV) return DEV_USER;
    throw new Error("ACCESS_TEAM_DOMAIN and ACCESS_AUD must be set");
  }
  const audience = mcp ? [env.ACCESS_AUD, env.ACCESS_MCP_AUD].filter((aud) => aud !== undefined) : [env.ACCESS_AUD];
  return assertion ? verifyAssertion(assertion, env.ACCESS_TEAM_DOMAIN, audience) : null;
}

export function requireUser(c: CloudContext) {
  const user = c.get("accessUser");
  if (!user) throw new HTTPException(401, { message: "Unauthorized" });
  return user;
}
