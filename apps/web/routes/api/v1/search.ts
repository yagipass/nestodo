import { defineHandler } from "void";
import { requireUser } from "../../../src/access";
import { searchQuery } from "../../../src/todo-schemas";
import { searchComments } from "../../../src/todo-service";

export const GET = defineHandler.withValidator({ query: searchQuery })(async (c, { query }) => {
  const user = requireUser(c);
  return { todos: await searchComments(user.email, query.q) };
});
