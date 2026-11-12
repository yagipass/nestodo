export const MAX_NAME_LENGTH = 50;
export const MAX_AVATAR_BYTES = 512 * 1024;
export const AVATAR_SIZE = 256;

const AVATAR_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const UNSAFE_NAME_CHARACTERS = /[\p{Cc}؜‎‏‪-‮⁦-⁩]/u;

export type AvatarType = "image/png" | "image/jpeg" | "image/webp";

export type Profile = { email: string; name: string | null; avatarUrl: string | null };

export function normalizeName(name: string) {
  const normalized = name.normalize("NFC").trim();
  return normalized === "" ? null : normalized;
}

export function nameProblem(name: string | null) {
  if (name === null) return null;
  if (name.length > MAX_NAME_LENGTH) return `The name must be ${MAX_NAME_LENGTH} characters or fewer`;
  if (UNSAFE_NAME_CHARACTERS.test(name)) return "The name cannot contain control characters";
  return null;
}

function startsWith(bytes: Uint8Array, signature: number[], offset = 0) {
  return bytes.length >= offset + signature.length && signature.every((byte, index) => bytes[offset + index] === byte);
}

const ascii = (text: string) => [...text].map((char) => char.charCodeAt(0));

export function avatarTypeOf(bytes: Uint8Array): AvatarType | null {
  if (startsWith(bytes, [0x89, ...ascii("PNG\r\n\x1a\n")])) return "image/png";
  if (startsWith(bytes, [0xff, 0xd8, 0xff])) return "image/jpeg";
  if (startsWith(bytes, ascii("RIFF")) && startsWith(bytes, ascii("WEBP"), 8)) return "image/webp";
  return null;
}

export function avatarProblem(bytes: Uint8Array) {
  if (bytes.length > MAX_AVATAR_BYTES) return `The image must be ${MAX_AVATAR_BYTES / 1024} KB or smaller`;
  if (avatarTypeOf(bytes) === null) return "The image must be a PNG, JPEG or WebP file";
  return null;
}

export function isAvatarId(id: string) {
  return AVATAR_ID.test(id);
}

export function avatarUrl(id: string) {
  return `/api/v1/avatars/${id}`;
}
