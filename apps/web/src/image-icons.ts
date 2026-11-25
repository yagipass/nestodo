import { hasUnsafeCharacters } from "./profiles";

export const MAX_IMAGE_ICONS = 50;
export const MAX_IMAGE_NAME_LENGTH = 30;
export const MAX_IMAGE_BYTES = 128 * 1024;

export type ImageIcon = { id: string; name: string; url: string };

const IMAGE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

export function isImageIconId(id: string) {
  return IMAGE_ID.test(id);
}

export function imageUrl(id: string) {
  return `/api/v1/icons/${id}/image`;
}

export function imageNameProblem(name: string | null) {
  if (name === null) return "Enter a name";
  if (name.length > MAX_IMAGE_NAME_LENGTH) return `The name must be ${MAX_IMAGE_NAME_LENGTH} characters or fewer`;
  if (hasUnsafeCharacters(name)) return "The name cannot contain control or invisible characters";
  return null;
}
