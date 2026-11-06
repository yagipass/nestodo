import { describe, expect, it } from "vite-plus/test";
import { Session, signedInUser } from "./helpers";

describe("the outline page", () => {
  it("does not show another user's todos", async () => {
    const owner = await signedInUser();
    const other = await signedInUser();
    await owner.createTodo("owner only");

    expect((await other.page()).todos).toEqual([]);
    expect((await owner.page()).todos.map((t) => t.title)).toEqual(["owner only"]);
  });

  it("opens with completed todos hidden in a browser that last hid them", async () => {
    const client = await signedInUser();

    expect((await client.page("show-completed=false")).showDone).toBe(false);
  });

  it("opens with completed todos shown until the browser hides them", async () => {
    const client = await signedInUser();

    expect((await client.page()).showDone).toBe(true);
    expect((await client.page("show-completed=true")).showDone).toBe(true);
  });

  it("is not shown to signed-out visitors", async () => {
    const res = await new Session().request("/", { page: true });
    expect(res.status).toBe(401);
  });
});
