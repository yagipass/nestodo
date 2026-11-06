import { createMcpHandler } from "@modelcontextprotocol/server";
import { defineHandler } from "void";
import { requireUser } from "../src/access";
import { nestodoMcpServer } from "../src/mcp-server";

const handle = defineHandler((c) => {
  const user = requireUser(c);
  return createMcpHandler(() => nestodoMcpServer(user.email)).fetch(c.req.raw);
});

export const GET = handle;
export const POST = handle;
export const DELETE = handle;
