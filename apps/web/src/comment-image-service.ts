import { env as bindings } from "cloudflare:workers";
import { storage } from "void/storage";
import { KEEP_UNATTACHED_IMAGE_HOURS, presentCommentImage } from "./comment-images";
import { avatarTypeOf } from "./profiles";
import { MINUTE, sqliteTime } from "./time";

const KEYS_PER_DELETE = 1000;

const objectKey = (id: string) => `comment-images/${id}`;

function primary() {
  return bindings.DB.withSession("first-primary");
}

export async function addCommentImage(userId: string, image: Uint8Array) {
  const contentType = avatarTypeOf(image);
  if (contentType === null) throw new Error("The comment image was saved without checking its format");
  const id = crypto.randomUUID();
  await storage.put(objectKey(id), image, { httpMetadata: { contentType } });
  try {
    await bindings.DB.prepare("INSERT INTO comment_images (id, user_id) VALUES (?, ?)").bind(id, userId).run();
  } catch (error) {
    await storage.delete(objectKey(id)).catch((cleanup) => console.error("Could not delete a comment image that was never saved", cleanup));
    throw error;
  }
  return presentCommentImage(id);
}

export async function commentImageOf(userId: string, id: string) {
  const found = await primary().prepare("SELECT 1 FROM comment_images WHERE user_id = ? AND id = ?").bind(userId, id).first();
  return found === null ? null : storage.get(objectKey(id));
}

export async function deleteStoredCommentImages(ids: readonly string[]) {
  for (let start = 0; start < ids.length; start += KEYS_PER_DELETE) {
    await storage.delete(ids.slice(start, start + KEYS_PER_DELETE).map(objectKey));
  }
}

export async function purgeUnattachedCommentImages(now: Date) {
  const cutoff = sqliteTime(now.getTime() - KEEP_UNATTACHED_IMAGE_HOURS * 60 * MINUTE);
  const { results } = await bindings.DB.prepare("DELETE FROM comment_images WHERE comment_id IS NULL AND created_at <= ? RETURNING id")
    .bind(cutoff)
    .all<{ id: string }>();
  await deleteStoredCommentImages(results.map((row) => row.id));
}
