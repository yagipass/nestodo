import { ROOT, type Op } from "./outline";
import type { Comment } from "./thread";

export type Send = (method: string, path: string, body?: unknown) => Promise<Response>;

const TYPING_DELAY = 500;

type Typed = Extract<Op, { type: "title" }>;

type Settle<T> = { resolve: (value: T) => void; reject: (error: unknown) => void };

type CommentJob =
  | ({ type: "comment"; key: string; body: string } & Settle<Comment>)
  | ({ type: "comment-edit"; id: number; body: string } & Settle<Comment>)
  | ({ type: "comment-delete"; id: number } & Settle<void>);

type Job = Op | CommentJob;

export class Sync {
  private readonly ids = new Map<string, number>();
  private readonly keys = new Map<number, string>();
  private readonly queue: Job[] = [];
  private readonly typed = new Map<string, { op: Typed; timer: ReturnType<typeof setTimeout> }>();
  private running = false;
  failed = false;
  revision = 0;

  constructor(
    ids: Iterable<[string, number]>,
    private readonly send: Send,
    private readonly onFail: () => void = () => {},
    private readonly onTrack: () => void = () => {},
  ) {
    this.track(ids);
  }

  get pending() {
    return this.running || this.queue.length > 0 || this.typed.size > 0;
  }

  keyOf(id: number) {
    return this.keys.get(id) ?? null;
  }

  idOf(key: string) {
    return this.ids.get(key) ?? null;
  }

  track(ids: Iterable<[string, number]>) {
    for (const [key, id] of ids) {
      this.ids.set(key, id);
      this.keys.set(id, key);
    }
  }

  apply(ops: readonly Op[]) {
    this.revision += 1;
    for (const op of ops) {
      if (op.type === "delete") {
        for (const key of op.removed) this.drop(key);
      } else if (op.type === "title") {
        this.drop(op.key);
      } else {
        this.flushTyped(op.key);
      }
      this.queue.push(op);
    }
    void this.pump();
  }

  title(key: string, title: string) {
    this.revision += 1;
    this.drop(key);
    const timer = setTimeout(() => {
      this.flushTyped(key);
      void this.pump();
    }, TYPING_DELAY);
    this.typed.set(key, { op: { type: "title", key, title }, timer });
  }

  flush() {
    for (const key of [...this.typed.keys()]) this.flushTyped(key);
    void this.pump();
  }

  comment(key: string, body: string) {
    return this.enqueue<Comment>((settle) => ({ type: "comment", key, body, ...settle }));
  }

  editComment(id: number, body: string) {
    return this.enqueue<Comment>((settle) => ({ type: "comment-edit", id, body, ...settle }));
  }

  deleteComment(id: number) {
    return this.enqueue<void>((settle) => ({ type: "comment-delete", id, ...settle }));
  }

  private enqueue<T>(job: (settle: Settle<T>) => CommentJob) {
    this.revision += 1;
    const done = new Promise<T>((resolve, reject) => {
      if (this.failed) reject(new Error("sync stopped"));
      else this.queue.push(job({ resolve, reject }));
    });
    void this.pump();
    return done;
  }

  private drop(key: string) {
    const pending = this.typed.get(key);
    if (!pending) return;
    clearTimeout(pending.timer);
    this.typed.delete(key);
  }

  private flushTyped(key: string) {
    const pending = this.typed.get(key);
    if (!pending) return;
    this.drop(key);
    this.queue.push(pending.op);
  }

  private async pump() {
    if (this.running || this.failed) return;
    this.running = true;
    try {
      while (this.queue.length > 0) {
        const job = this.queue[0];
        try {
          await this.run(job);
        } catch (error) {
          if ("reject" in job) job.reject(error);
          throw error;
        }
        this.queue.shift();
      }
    } catch {
      this.failed = true;
      for (const job of this.queue.splice(1)) if ("reject" in job) job.reject(new Error("sync stopped"));
      this.onFail();
    } finally {
      this.running = false;
    }
  }

  private serverId(key: string) {
    const id = this.ids.get(key);
    if (id === undefined) throw new Error(`${key} has no server id`);
    return id;
  }

  private parentId(key: string) {
    return key === ROOT ? null : this.serverId(key);
  }

  private async expect(res: Response, what: string) {
    if (!res.ok) throw new Error(`${what} failed with ${res.status}`);
    return res;
  }

  private async run(job: Job) {
    switch (job.type) {
      case "create": {
        const res = await this.expect(
          await this.send("POST", "/api/v1/todos", { title: job.title, parentId: this.parentId(job.parent), index: job.index }),
          "create",
        );
        const { todo } = (await res.json()) as { todo: { id: number } };
        this.track([[job.key, todo.id]]);
        this.onTrack();
        return;
      }
      case "comment": {
        const res = await this.expect(await this.send("POST", `/api/v1/todos/${this.serverId(job.key)}/comments`, { body: job.body }), "comment");
        job.resolve(((await res.json()) as { comment: Comment }).comment);
        return;
      }
      case "comment-edit": {
        const res = await this.expect(await this.send("PATCH", `/api/v1/comments/${job.id}`, { body: job.body }), "comment-edit");
        job.resolve(((await res.json()) as { comment: Comment }).comment);
        return;
      }
      case "comment-delete": {
        const res = await this.send("DELETE", `/api/v1/comments/${job.id}`);
        if (!res.ok && res.status !== 404) throw new Error(`comment-delete failed with ${res.status}`);
        job.resolve();
        return;
      }
      case "restore": {
        const path = `/api/v1/todos/${this.serverId(job.key)}/restore`;
        await this.expect(await this.send("POST", path, { parentId: this.parentId(job.parent), index: job.index }), "restore");
        return;
      }
      case "delete": {
        const path = `/api/v1/todos/${this.serverId(job.key)}`;
        const res = await this.send("DELETE", job.removed.length > 1 ? `${path}?recursive=true` : path);
        if (!res.ok && res.status !== 404) throw new Error(`delete failed with ${res.status}`);
        return;
      }
      default: {
        const path = `/api/v1/todos/${this.serverId(job.key)}`;
        const body = job.type === "title" ? { title: job.title } : job.type === "move" ? { parentId: this.parentId(job.parent), index: job.index } : job.fields;
        await this.expect(await this.send("PATCH", path, body), job.type);
      }
    }
  }
}
