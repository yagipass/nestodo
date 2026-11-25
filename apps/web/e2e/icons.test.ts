import { describe, expect, it } from "vite-plus/test";
import { json, png, signedInUser, type Todo } from "./helpers";

const MAX_IMAGE_ICONS = 50;

async function messageOf(res: Response) {
  return JSON.stringify(await res.json());
}

describe("the member's images", () => {
  it("lists images in the order they were added, so the picker does not reshuffle them", async () => {
    const member = await signedInUser();
    const cat = await member.addedImage("ミケ");
    const school = await member.addedImage("みなみ小");

    expect(await member.images()).toEqual([cat, school]);
    expect(cat.url).toBe(`/api/v1/icons/${cat.id}/image`);
  });

  it("serves an image back with its type for every app to draw and cache for good, while the list of images is never cached", async () => {
    const member = await signedInUser();
    const image = png([30, 160, 90]);
    const { url } = await member.addedImage("FC", image);

    const res = await member.image(url);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("image/png");
    expect(res.headers.get("cache-control")).toContain("immutable");
    expect(Buffer.from(await res.arrayBuffer())).toEqual(image);
    expect((await member.request("/api/v1/icons")).headers.get("cache-control")).toBe("no-store");
  });

  it("loads the images with the outline page, so the first render already draws them", async () => {
    const member = await signedInUser();
    const cat = await member.addedImage("ミケ");

    expect((await member.page()).icons).toEqual([cat]);
  });

  it("keeps each member's images to themselves, so nobody can see or use another member's pictures", async () => {
    const owner = await signedInUser();
    const other = await signedInUser();
    const cat = await owner.addedImage("ミケ");

    expect(await other.images()).toEqual([]);
    expect((await other.image(cat.url)).status).toBe(404);
    expect((await other.renameImage(cat.id, "mine")).status).toBe(404);
    expect((await other.deleteImage(cat.id)).status).toBe(404);
    const refused = await other.request("/api/v1/todos", { method: "POST", body: { title: "vet", icon: `image:${cat.id}` } });
    expect(refused.status).toBe(400);
    expect(await json(refused)).toEqual({ error: "invalid_icon", message: "Icon not found" });
    expect(await owner.images()).toEqual([cat]);
  });

  it("asks for a name and refuses one that is too long, since the picker, the CLI and Claude tell images apart by it", async () => {
    const member = await signedInUser();

    for (const name of ["", "   ", "a".repeat(31)]) {
      const res = await member.addImage(name);
      expect(res.status, JSON.stringify(name)).toBe(400);
    }
    expect(await messageOf(await member.addImage(" "))).toContain("Enter a name");
    expect((await member.addedImage("  ミケ  ")).name).toBe("ミケ");
    expect(await member.images()).toHaveLength(1);
  });

  it("refuses a file that is not a picture, so the apps never try to draw one", async () => {
    const member = await signedInUser();

    const res = await member.addImage("notes", Buffer.from("not an image"));
    expect(res.status).toBe(400);
    expect(await messageOf(res)).toContain("PNG, JPEG or WebP");
    expect(await member.images()).toEqual([]);
  });

  it(`stops at ${MAX_IMAGE_ICONS} images with a message the picker can show, so the picker stays short`, async () => {
    const member = await signedInUser();
    for (let index = 0; index < MAX_IMAGE_ICONS; index++) await member.addedImage(`image ${index}`);

    const res = await member.addImage("one more");
    expect(res.status).toBe(409);
    expect(await json(res)).toEqual({ error: "too_many_icons", message: `You can add up to ${MAX_IMAGE_ICONS} images` });
    expect(await member.images()).toHaveLength(MAX_IMAGE_ICONS);
  });

  it("renames an image without changing its id or picture, so the todos showing it keep it", async () => {
    const member = await signedInUser();
    const cat = await member.addedImage("mike");
    const todo = await member.createTodo("vet", { icon: `image:${cat.id}` });

    const res = await member.renameImage(cat.id, " ミケ ");
    expect(res.status).toBe(200);
    expect((await json<{ icon: unknown }>(res)).icon).toEqual({ ...cat, name: "ミケ" });
    expect((await member.todos()).find((candidate) => candidate.id === todo.id)?.icon).toBe(`image:${cat.id}`);
    expect((await member.renameImage(cat.id, "")).status).toBe(400);
  });
});

describe("todos showing an image", () => {
  it("lets a todo show one of the member's images, set when it is added or later", async () => {
    const member = await signedInUser();
    const cat = await member.addedImage("ミケ");
    const added = await member.createTodo("vet", { icon: `image:${cat.id}` });
    const other = await member.createTodo("food");

    expect(added.icon).toBe(`image:${cat.id}`);
    const res = await member.patch(other.id, { icon: `image:${cat.id}` });
    expect(res.status).toBe(200);
    expect((await json<{ todo: Todo }>(res)).todo.icon).toBe(`image:${cat.id}`);
  });

  it("refuses an image that does not exist, so a todo never points at nothing", async () => {
    const member = await signedInUser();
    const todo = await member.createTodo("vet");

    const res = await member.patch(todo.id, { icon: "image:3f2a9c1e-5b7d-4e8f-9a0b-1c2d3e4f5a6b" });
    expect(res.status).toBe(400);
    expect(await json(res)).toEqual({ error: "invalid_icon", message: "Icon not found" });
    expect((await member.patch(todo.id, { icon: "image:cat" })).status).toBe(400);
    expect((await member.todos())[0].icon).toBeNull();
  });

  it("takes a deleted image off every todo that showed it, also one in the trash, so none points at a missing picture", async () => {
    const member = await signedInUser();
    const cat = await member.addedImage("ミケ");
    const kept = await member.createTodo("vet", { icon: `image:${cat.id}` });
    const trashed = await member.createTodo("food", { icon: `image:${cat.id}` });
    const emoji = await member.createTodo("groceries", { icon: "🛒" });
    expect((await member.request(`/api/v1/todos/${trashed.id}`, { method: "DELETE" })).status).toBe(204);

    expect((await member.deleteImage(cat.id)).status).toBe(204);

    const icons = Object.fromEntries((await member.todos()).map((todo) => [todo.id, todo.icon]));
    expect(icons).toEqual({ [kept.id]: null, [emoji.id]: "🛒" });
    const restored = await member.restore(trashed.id);
    expect(restored.status).toBe(200);
    expect((await json<{ todo: Todo }>(restored)).todo.icon).toBeNull();
    expect(await member.images()).toEqual([]);
    expect((await member.image(cat.url)).status).toBe(404);
    expect((await member.deleteImage(cat.id)).status).toBe(404);
  });
});
