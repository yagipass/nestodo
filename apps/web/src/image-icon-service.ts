import { env as bindings } from "cloudflare:workers";
import { HTTPException } from "hono/http-exception";
import { storage } from "void/storage";
import { imageUrl, MAX_IMAGE_ICONS, type ImageIcon } from "./image-icons";
import { avatarTypeOf } from "./profiles";
import { imageIcon } from "./todo-icon";

type Row = { id: string; name: string };

const objectKey = (id: string) => `icons/${id}`;

function primary() {
  return bindings.DB.withSession("first-primary");
}

function present(row: Row): ImageIcon {
  return { id: row.id, name: row.name, url: imageUrl(row.id) };
}

function tooMany() {
  const status = 409;
  return new HTTPException(status, {
    res: Response.json({ error: "too_many_icons", message: `You can add up to ${MAX_IMAGE_ICONS} images` }, { status }),
  });
}

export async function listImageIcons(userId: string) {
  const { results } = await primary()
    .prepare("SELECT id, name FROM custom_icons WHERE user_id = ? ORDER BY created_at, id")
    .bind(userId)
    .all<Row>();
  return results.map(present);
}

export async function hasImageIcon(userId: string, id: string) {
  return (await primary().prepare("SELECT 1 FROM custom_icons WHERE user_id = ? AND id = ?").bind(userId, id).first()) !== null;
}

async function countOf(userId: string) {
  return (await primary().prepare("SELECT count(*) AS count FROM custom_icons WHERE user_id = ?").bind(userId).first<{ count: number }>())?.count ?? 0;
}

export async function addImageIcon(userId: string, name: string, image: Uint8Array) {
  const contentType = avatarTypeOf(image);
  if (contentType === null) throw new Error("The icon was saved without checking its format");
  if ((await countOf(userId)) >= MAX_IMAGE_ICONS) throw tooMany();
  const id = crypto.randomUUID();
  await storage.put(objectKey(id), image, { httpMetadata: { contentType } });
  const discard = () => storage.delete(objectKey(id)).catch((cleanup) => console.error("Could not delete an icon that was never saved", cleanup));
  let changes: number;
  try {
    const inserted = await bindings.DB.prepare(
      "INSERT INTO custom_icons (id, user_id, name) SELECT ?, ?, ? WHERE (SELECT count(*) FROM custom_icons WHERE user_id = ?) < ?",
    )
      .bind(id, userId, name, userId, MAX_IMAGE_ICONS)
      .run();
    changes = inserted.meta.changes;
  } catch (error) {
    await discard();
    throw error;
  }
  if (changes === 0) {
    await discard();
    throw tooMany();
  }
  return present({ id, name });
}

export async function renameImageIcon(userId: string, id: string, name: string) {
  const row = await bindings.DB.prepare("UPDATE custom_icons SET name = ? WHERE user_id = ? AND id = ? RETURNING id, name")
    .bind(name, userId, id)
    .first<Row>();
  return row ? present(row) : null;
}

export async function deleteImageIcon(userId: string, id: string) {
  const icon = imageIcon(id);
  const [deleted] = await bindings.DB.batch([
    bindings.DB.prepare("DELETE FROM custom_icons WHERE user_id = ? AND id = ? RETURNING id").bind(userId, id),
    bindings.DB.prepare("UPDATE todos SET icon = NULL, updated_at = datetime('now') WHERE user_id = ? AND icon = ?").bind(userId, icon),
    bindings.DB.prepare("UPDATE trashed_todos SET icon = NULL WHERE user_id = ? AND icon = ?").bind(userId, icon),
  ]);
  if (deleted.results.length === 0) return false;
  await storage.delete(objectKey(id)).catch((error) => console.error("Could not delete the image of a deleted icon", error));
  return true;
}

export async function imageOf(userId: string, id: string) {
  return (await hasImageIcon(userId, id)) ? storage.get(objectKey(id)) : null;
}
