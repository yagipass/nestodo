import { defineScheduled } from "void";
import { purgeCompletedTodos } from "../src/todo-service";

export const cron = "0 18 * * *";

export default defineScheduled(async (controller) => {
  await purgeCompletedTodos(new Date(controller.scheduledTime));
});
