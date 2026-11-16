import { defineHandler } from "void";
import { requireUser } from "../../../../../src/access";
import { todoIdParams } from "../../../../../src/todo-schemas";
import { timelineOf } from "../../../../../src/todo-service";

export const GET = defineHandler.withValidator({ params: todoIdParams })(async (c, { params }) => {
  const user = requireUser(c);
  const items = await timelineOf(user.email, params.id);
  return items ? c.json({ items }) : c.json({ error: "not_found", message: "Todo not found" }, 404);
});
