import { COMMENT_IMAGE_EDGE, fitWithin } from "./comment-images";
import { AVATAR_SIZE } from "./profiles";

export function centerSquare(width: number, height: number) {
  const side = Math.min(width, height);
  return { x: (width - side) / 2, y: (height - side) / 2, side };
}

export function avatarImageFrom(file: Blob) {
  return squareImageFrom(file, AVATAR_SIZE);
}

export async function squareImageFrom(file: Blob, size: number) {
  const bitmap = await createImageBitmap(file);
  try {
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("The browser cannot draw the image");
    const { x, y, side } = centerSquare(bitmap.width, bitmap.height);
    context.imageSmoothingQuality = "high";
    context.drawImage(bitmap, x, y, side, side, 0, 0, size, size);
    return await encode(canvas, "image/webp", 0.85);
  } finally {
    bitmap.close();
  }
}

function encode(canvas: HTMLCanvasElement, type: string, quality: number) {
  return new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("The browser cannot encode the image"))), type, quality),
  );
}

export async function commentImageFrom(file: Blob) {
  const bitmap = await createImageBitmap(file);
  try {
    const { width, height } = fitWithin(bitmap.width, bitmap.height, COMMENT_IMAGE_EDGE);
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("The browser cannot draw the image");
    context.imageSmoothingQuality = "high";
    context.drawImage(bitmap, 0, 0, width, height);
    const webp = await encode(canvas, "image/webp", 0.85);
    if (webp.type === "image/webp") return webp;
    context.globalCompositeOperation = "destination-over";
    context.fillStyle = "#fff";
    context.fillRect(0, 0, width, height);
    return await encode(canvas, "image/jpeg", 0.85);
  } finally {
    bitmap.close();
  }
}

export async function base64Of(blob: Blob) {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  for (let start = 0; start < bytes.length; start += 0x8000) binary += String.fromCharCode(...bytes.subarray(start, start + 0x8000));
  return btoa(binary);
}
