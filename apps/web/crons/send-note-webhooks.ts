import { defineScheduled } from "void";
import { sendWaitingTodos } from "../src/webhook-service";

export const cron = "*/10 * * * *";

export default defineScheduled(async (controller) => {
  await sendWaitingTodos(new Date(controller.scheduledTime));
});
