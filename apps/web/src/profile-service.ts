import { env as bindings } from "cloudflare:workers";
import { and, db, eq } from "void/db";
import { storage } from "void/storage";
import { profiles } from "@schema";
import { avatarTypeOf, avatarUrl, type Profile } from "./profiles";

type Row = { name: string | null; avatarId: string | null };

const objectKey = (avatarId: string) => `avatars/${avatarId}`;

function rowOf(userId: string) {
  return bindings.DB.withSession("first-primary").prepare("SELECT name, avatar_id AS avatarId FROM profiles WHERE user_id = ?").bind(userId).first<Row>();
}

export async function profileOf(userId: string): Promise<Profile> {
  const row = await rowOf(userId);
  return { email: userId, name: row?.name ?? null, avatarUrl: row?.avatarId ? avatarUrl(row.avatarId) : null };
}

export async function saveName(userId: string, name: string | null) {
  await db.insert(profiles).values({ userId, name }).onConflictDoUpdate({ target: profiles.userId, set: { name } });
}

async function replaceAvatarId(userId: string, avatarId: string) {
  const [previous] = await bindings.DB.batch<Pick<Row, "avatarId">>([
    bindings.DB.prepare("SELECT avatar_id AS avatarId FROM profiles WHERE user_id = ?").bind(userId),
    bindings.DB.prepare("INSERT INTO profiles (user_id, avatar_id) VALUES (?, ?) ON CONFLICT (user_id) DO UPDATE SET avatar_id = excluded.avatar_id").bind(
      userId,
      avatarId,
    ),
  ]);
  return previous.results[0]?.avatarId ?? null;
}

export async function saveAvatar(userId: string, image: Uint8Array) {
  const contentType = avatarTypeOf(image);
  if (contentType === null) throw new Error("The avatar was saved without checking its format");
  const avatarId = crypto.randomUUID();
  await storage.put(objectKey(avatarId), image, { httpMetadata: { contentType } });
  let previous: string | null;
  try {
    previous = await replaceAvatarId(userId, avatarId);
  } catch (error) {
    await storage.delete(objectKey(avatarId)).catch((cleanup) => console.error("Could not delete an avatar that was never saved", cleanup));
    throw error;
  }
  if (previous) await storage.delete(objectKey(previous)).catch((error) => console.error("Could not delete a replaced avatar", error));
}

export async function removeAvatar(userId: string) {
  const row = await rowOf(userId);
  if (!row?.avatarId) return;
  await storage.delete(objectKey(row.avatarId));
  await db
    .update(profiles)
    .set({ avatarId: null })
    .where(and(eq(profiles.userId, userId), eq(profiles.avatarId, row.avatarId)));
}

export async function avatarOf(userId: string, avatarId: string) {
  const row = await rowOf(userId);
  return row?.avatarId === avatarId ? storage.get(objectKey(avatarId)) : null;
}
