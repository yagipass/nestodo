import { defineScheduled } from "void";
import { purgeUnattachedCommentImages } from "../src/comment-image-service";

export const cron = "0 18 * * *";

export default defineScheduled(async (controller) => {
  await purgeUnattachedCommentImages(new Date(controller.scheduledTime));
});
