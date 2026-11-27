export const MAX_COMMENT_IMAGES = 4;
export const MAX_COMMENT_IMAGE_BYTES = 2 * 1024 * 1024;
export const KEEP_UNATTACHED_IMAGE_HOURS = 24;

export type CommentImage = { id: string; url: string };

const IMAGE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

export function isCommentImageId(id: string) {
  return IMAGE_ID.test(id);
}

export function commentImageUrl(id: string) {
  return `/api/v1/comment-images/${id}`;
}

export function presentCommentImage(id: string): CommentImage {
  return { id, url: commentImageUrl(id) };
}
