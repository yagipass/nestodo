import { randomUUID } from "node:crypto";
import { describe, expect, it } from "vite-plus/test";
import { json, png, runCron, signedInUser, storedObjectKeys, type Comment, type CommentImage } from "./helpers";

const DAILY = "0 18 * * *";
const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;
const MAX_BYTES = 2 * 1024 * 1024;

function later(ms: number) {
  return new Date(Date.now() + ms);
}

function stored(image: CommentImage) {
  return storedObjectKeys(`comment-images/${image.id}`).length > 0;
}

function pngOfSize(bytes: number) {
  const image = Buffer.alloc(bytes);
  png([0, 0, 0]).copy(image, 0, 0, 8);
  return image;
}

async function messageOf(res: Response) {
  return JSON.stringify(await res.json());
}

describe("pictures on a comment", () => {
  it("shows the pictures in the order they were chosen, and serves each back with its type for the apps to draw and cache for good", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("fix the leak");
    const before = png([200, 40, 40]);
    const after = png([40, 200, 40]);
    const first = await member.uploadedCommentImage(before);
    const second = await member.uploadedCommentImage(after);

    const res = await member.postComment(todo.id, { body: "fixed, see the photos", images: [second.id, first.id] });

    expect(res.status).toBe(201);
    expect((await json<{ comment: Comment }>(res)).comment.images).toEqual([second, first]);
    const [, comment] = await member.timeline(todo.id);
    expect(comment).toMatchObject({ type: "comment", body: "fixed, see the photos", images: [second, first] });
    const served = await member.request(first.url);
    expect(served.status).toBe(200);
    expect(served.headers.get("content-type")).toBe("image/png");
    expect(served.headers.get("cache-control")).toContain("immutable");
    expect(Buffer.from(await served.arrayBuffer())).toEqual(before);
  });

  it("gives every comment an empty list of pictures when it has none, so the apps never have to guess", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("call the plumber", { comment: "no answer" });

    expect((await member.comment(todo.id, "try again tomorrow")).images).toEqual([]);
    expect((await member.timeline(todo.id)).filter((item) => item.type === "comment").map((item) => (item as Comment).images)).toEqual([[], []]);
  });

  it("allows a comment that is only pictures, stored with no text, while one with neither text nor pictures is still refused", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("paint the fence");
    const photo = await member.uploadedCommentImage();

    const res = await member.postComment(todo.id, { body: " \n", images: [photo.id] });
    expect(res.status).toBe(201);
    expect((await json<{ comment: Comment }>(res)).comment).toMatchObject({ body: "", images: [photo] });

    const blank = await member.postComment(todo.id, { body: "  ", images: [] });
    expect(blank.status).toBe(400);
    expect(await messageOf(blank)).toContain("Comment must not be empty");
    expect((await member.todos())[0].commentCount).toBe(1);
  });

  it("refuses more than 4 pictures or the same picture twice, so a comment stays readable and every picture shows once", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("move house");
    const photos = await Promise.all(Array.from({ length: 5 }, () => member.uploadedCommentImage()));

    const tooMany = await member.postComment(todo.id, { body: "boxes", images: photos.map((photo) => photo.id) });
    const twice = await member.postComment(todo.id, { body: "boxes", images: [photos[0].id, photos[0].id] });

    expect([tooMany.status, twice.status]).toEqual([400, 400]);
    expect(await messageOf(tooMany)).toContain("up to 4 images");
    expect(await messageOf(twice)).toContain("Each image can be added once");
    expect((await member.todos())[0].commentCount).toBe(0);
    expect((await member.postComment(todo.id, { body: "boxes", images: photos.slice(0, 4).map((photo) => photo.id) })).status).toBe(201);
  });

  it("refuses a picture that is already on another comment, so deleting one comment never takes a picture away from another", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("garden");
    const photo = await member.uploadedCommentImage();
    await member.postComment(todo.id, { body: "planted", images: [photo.id] });

    const res = await member.postComment(todo.id, { body: "again", images: [photo.id] });

    expect(res.status).toBe(400);
    expect(await json(res)).toEqual({ error: "invalid_images", message: "Image not found" });
    expect((await member.timeline(todo.id)).filter((item) => item.type === "comment")).toMatchObject([{ body: "planted", images: [photo] }]);
  });

  it("posts only one of two comments sent at once with the same picture, so a double send never leaves an empty comment behind", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("garage door");
    const photo = await member.uploadedCommentImage();

    const statuses = (await Promise.all([1, 2].map(() => member.postComment(todo.id, { images: [photo.id] })))).map((res) => res.status);

    expect(statuses.sort()).toEqual([201, 400]);
    expect((await member.timeline(todo.id)).filter((item) => item.type === "comment")).toMatchObject([{ body: "", images: [photo] }]);
    expect((await member.todos())[0].commentCount).toBe(1);
  });

  it("keeps each member's pictures to themselves, so nobody can see or reuse another member's photo by its id", async () => {
    const owner = await signedInUser();
    const other = await signedInUser();
    const photo = await owner.uploadedCommentImage();
    const theirs = await other.createTodo("mine");

    expect((await other.request(photo.url)).status).toBe(404);
    expect((await other.postComment(theirs.id, { images: [photo.id] })).status).toBe(400);
    expect((await other.timeline(theirs.id)).filter((item) => item.type === "comment")).toEqual([]);
    const todo = await owner.createTodo("ours");
    expect((await owner.postComment(todo.id, { images: [photo.id] })).status).toBe(201);
  });

  it("takes a PNG, JPEG or WebP picture up to 2 MB and refuses anything else, so the apps never get a file they cannot draw", async () => {
    const member = await signedInUser();

    expect((await member.uploadCommentImage(pngOfSize(MAX_BYTES))).status).toBe(201);
    const large = await member.uploadCommentImage(pngOfSize(MAX_BYTES + 1));
    const text = await member.uploadCommentImage(Buffer.from("not an image"));

    expect([large.status, text.status]).toEqual([400, 400]);
    expect(await messageOf(large)).toContain("2048 KB or smaller");
    expect(await messageOf(text)).toContain("PNG, JPEG or WebP");
  });
});

describe("editing a comment with pictures", () => {
  it("lets the text of a comment with pictures be cleared, keeping the pictures, but not the text of one without", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("repair the bike");
    const photo = await member.uploadedCommentImage();
    const withPhoto = await json<{ comment: Comment }>(await member.postComment(todo.id, { body: "chain is rusty", images: [photo.id] }));
    const textOnly = await member.comment(todo.id, "ordered a new chain");

    const cleared = await member.editComment(withPhoto.comment.id, "");
    const refused = await member.editComment(textOnly.id, " ");

    expect(cleared.status).toBe(200);
    expect((await json<{ comment: Comment }>(cleared)).comment).toMatchObject({ body: "", images: [photo] });
    expect(refused.status).toBe(400);
    expect(await messageOf(refused)).toContain("Comment must not be empty");
    expect((await member.timeline(todo.id)).filter((item) => item.type === "comment")).toMatchObject([
      { body: "", images: [photo] },
      { body: "ordered a new chain", images: [] },
    ]);
  });
});

describe("forgetting pictures", () => {
  it("deletes the pictures with their comment, so nothing of a deleted comment stays stored", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("wallpaper");
    const photo = await member.uploadedCommentImage();
    const kept = await member.uploadedCommentImage();
    const { comment } = await json<{ comment: Comment }>(await member.postComment(todo.id, { body: "old one", images: [photo.id] }));
    await member.postComment(todo.id, { body: "new one", images: [kept.id] });

    expect((await member.deleteComment(comment.id)).status).toBe(204);

    expect(stored(photo)).toBe(false);
    expect((await member.request(photo.url)).status).toBe(404);
    expect(stored(kept)).toBe(true);
  });

  it("keeps the pictures of a deleted todo's comments while it can be undone, and forgets them once the trash is emptied", async () => {
    const member = await signedInUser(`${randomUUID()}@e2e.test`);
    const todo = await member.createTodo("roof");
    const photo = await member.uploadedCommentImage();
    await member.postComment(todo.id, { body: "the broken tile", images: [photo.id] });
    const log = await member.timeline(todo.id);
    await member.request(`/api/v1/todos/${todo.id}`, { method: "DELETE" });

    expect((await member.deleteComment((log[1] as Comment).id)).status).toBe(404);
    expect(stored(photo)).toBe(true);
    await member.restore(todo.id);
    expect(await member.timeline(todo.id)).toEqual(log);
    expect((await member.request(photo.url)).status).toBe(200);

    await member.request(`/api/v1/todos/${todo.id}`, { method: "DELETE" });
    await runCron(DAILY, later(9 * DAY));
    expect(stored(photo)).toBe(false);
    expect((await member.request(photo.url)).status).toBe(404);
  });

  it("forgets a picture that was uploaded but never posted a day later, so abandoned uploads do not pile up, while posted ones stay", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("tiles");
    const abandoned = await member.uploadedCommentImage();
    const posted = await member.uploadedCommentImage();
    await member.postComment(todo.id, { images: [posted.id] });

    await runCron(DAILY, later(23 * HOUR));
    expect(stored(abandoned)).toBe(true);

    await runCron(DAILY, later(25 * HOUR));
    expect(stored(abandoned)).toBe(false);
    expect((await member.request(abandoned.url)).status).toBe(404);
    expect((await member.postComment(todo.id, { images: [abandoned.id] })).status).toBe(400);
    expect(stored(posted)).toBe(true);
    expect((await member.request(posted.url)).status).toBe(200);
  });
});
