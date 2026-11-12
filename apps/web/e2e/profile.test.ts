import { randomUUID } from "node:crypto";
import { deflateSync } from "node:zlib";
import { describe, expect, it } from "vite-plus/test";
import { json, Session, signedInUser, type Profile } from "./helpers";

const MAX_AVATAR_BYTES = 512 * 1024;

function crc32(data: Buffer) {
  let crc = ~0;
  for (const byte of data) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return ~crc >>> 0;
}

function chunk(type: string, data: Buffer) {
  const typed = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(typed));
  return Buffer.concat([length, typed, crc]);
}

function png([red, green, blue]: [number, number, number]) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(1, 0);
  header.writeUInt32BE(1, 4);
  header.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(Buffer.from([0, red, green, blue]))),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function pngOfSize(size: number) {
  const bytes = Buffer.alloc(size);
  png([0, 0, 0]).copy(bytes);
  return bytes;
}

async function withAvatar(member: Session, image: Uint8Array = png([200, 80, 160])) {
  const res = await member.uploadAvatar(image);
  expect(res.status).toBe(200);
  const { avatarUrl } = await json<Profile>(res);
  expect(avatarUrl).not.toBeNull();
  return avatarUrl!;
}

async function issuesOf(res: Response) {
  const body = await json<{ issues: { issues: { path: string; message: string }[] }[] }>(res);
  return body.issues.flatMap(({ issues }) => issues);
}

describe("the profile", () => {
  it("starts with no name or picture, so every app falls back to the email address and the generic icon", async () => {
    const email = `${randomUUID()}@e2e.test`;

    expect(await (await signedInUser(email)).profile()).toEqual({ email, name: null, avatarUrl: null });
  });

  it("follows the member to another device, because the name belongs to the account", async () => {
    const email = `${randomUUID()}@e2e.test`;
    expect((await (await signedInUser(email)).saveName("Kim")).status).toBe(200);

    expect((await (await signedInUser(email)).profile()).name).toBe("Kim");
  });

  it("reaches the header of every page, so the web app shows a new name on its next navigation", async () => {
    const member = await signedInUser();
    await member.saveName("Kim");
    const avatarUrl = await withAvatar(member);

    expect(await member.pageProfile()).toMatchObject({ name: "Kim", avatarUrl });
  });

  it("drops the spaces around a name and clears a blank one, so the apps go back to the email address", async () => {
    const member = await signedInUser();

    expect((await json<Profile>(await member.saveName("  Kim  "))).name).toBe("Kim");
    expect((await json<Profile>(await member.saveName("   "))).name).toBeNull();
    expect((await member.profile()).name).toBeNull();
  });

  it("refuses a name that reverses text, names the field so the apps show the message under it, and keeps the saved name", async () => {
    const member = await signedInUser();
    await member.saveName("Kim");

    const res = await member.saveName("Kim‮miK");

    expect(res.status).toBe(400);
    expect((await issuesOf(res)).map(({ path }) => path)).toEqual(["name"]);
    expect((await member.profile()).name).toBe("Kim");
  });

  it("is kept apart for each member of the family", async () => {
    await (await signedInUser()).saveName("Kim");

    expect((await (await signedInUser()).profile()).name).toBeNull();
  });

  it("is refused without signing in", async () => {
    expect((await new Session().request("/api/v1/profile", { origin: null })).status).toBe(401);
    expect((await new Session().request("/api/v1/profile", { method: "PUT", origin: null, body: { name: "Kim" } })).status).toBe(401);
  });
});

describe("the avatar", () => {
  it("serves back the uploaded image with its type, so every app can show it", async () => {
    const member = await signedInUser();
    const image = png([200, 80, 160]);

    const res = await member.avatar(await withAvatar(member, image));

    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("image/png");
    expect(Buffer.from(await res.arrayBuffer())).toEqual(image);
  });

  it("gets a new address on every upload and may be cached for good, so a changed picture shows at once without downloading the same one again", async () => {
    const member = await signedInUser();
    const first = await withAvatar(member, png([255, 0, 0]));
    const second = await withAvatar(member, png([0, 0, 255]));

    const res = await member.avatar(second);

    expect(second).not.toBe(first);
    expect(res.headers.get("cache-control")).toBe("private, max-age=31536000, immutable");
  });

  it("stops serving the replaced picture, so a picture the member changed does not stay reachable", async () => {
    const member = await signedInUser();
    const replaced = await withAvatar(member, png([255, 0, 0]));
    await withAvatar(member, png([0, 0, 255]));

    expect((await member.avatar(replaced)).status).toBe(404);
  });

  it("is gone once removed, so the apps go back to the generic icon", async () => {
    const member = await signedInUser();
    const removed = await withAvatar(member);

    expect((await member.removeAvatar()).status).toBe(204);

    expect((await member.profile()).avatarUrl).toBeNull();
    expect((await member.avatar(removed)).status).toBe(404);
  });

  it("can be removed when there is none, so a retried removal does not fail", async () => {
    expect((await (await signedInUser()).removeAvatar()).status).toBe(204);
  });

  it("cannot be read by another member, because each member's data stays their own", async () => {
    const owner = await signedInUser();
    const avatarUrl = await withAvatar(owner);

    expect((await (await signedInUser()).avatar(avatarUrl)).status).toBe(404);
    expect((await new Session().avatar(avatarUrl)).status).toBe(401);
  });

  it("refuses SVG, which could run script from this site, and keeps the current picture", async () => {
    const member = await signedInUser();
    const avatarUrl = await withAvatar(member);

    const res = await member.uploadAvatar(Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>'));

    expect(res.status).toBe(400);
    expect((await issuesOf(res)).map(({ path }) => path)).toEqual(["image"]);
    expect((await member.profile()).avatarUrl).toBe(avatarUrl);
  });

  it("accepts an image right at the size limit, so a picture the apps shrank always fits", async () => {
    const member = await signedInUser();

    expect((await member.uploadAvatar(pngOfSize(MAX_AVATAR_BYTES))).status).toBe(200);
  });

  it("refuses an image over the size limit", async () => {
    const member = await signedInUser();

    const res = await member.uploadAvatar(pngOfSize(MAX_AVATAR_BYTES + 1));

    expect(res.status).toBe(400);
    expect((await member.profile()).avatarUrl).toBeNull();
  });

  it("refuses an image that is not base64", async () => {
    expect((await (await signedInUser()).uploadAvatar("not base64!")).status).toBe(400);
  });
});
