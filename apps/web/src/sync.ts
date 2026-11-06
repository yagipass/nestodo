import { ROOT, type Op } from "./outline";

export type Send = (method: string, path: string, body?: unknown) => Promise<Response>;

const TYPING_DELAY = 500;

type Typed = Extract<Op, { type: "title" | "note" }>;

export class Sync {
  private readonly ids = new Map<string, number>();
  private readonly keys = new Map<number, string>();
  private readonly queue: Op[] = [];
  private readonly typed = new Map<string, { op: Typed; timer: ReturnType<typeof setTimeout> }>();
  private running = false;
  failed = false;
  revision = 0;

  constructor(
    ids: Iterable<[string, number]>,
    private readonly send: Send,
    private readonly onFail: () => void = () => {},
  ) {
    this.track(ids);
  }

  get pending() {
    return this.running || this.queue.length > 0 || this.typed.size > 0;
  }

  keyOf(id: number) {
    return this.keys.get(id) ?? null;
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
        for (const key of op.removed) {
          this.drop("title", key);
          this.drop("note", key);
        }
      } else if (op.type === "title" || op.type === "note") {
        this.drop(op.type, op.key);
      } else {
        this.flushTyped("title", op.key);
        this.flushTyped("note", op.key);
      }
      this.queue.push(op);
    }
    void this.pump();
  }

  title(key: string, title: string) {
    this.type({ type: "title", key, title });
  }

  note(key: string, note: string) {
    this.type({ type: "note", key, note });
  }

  flush() {
    for (const { op } of [...this.typed.values()]) this.flushTyped(op.type, op.key);
    void this.pump();
  }

  private type(op: Typed) {
    this.revision += 1;
    this.drop(op.type, op.key);
    const timer = setTimeout(() => {
      this.flushTyped(op.type, op.key);
      void this.pump();
    }, TYPING_DELAY);
    this.typed.set(`${op.type}:${op.key}`, { op, timer });
  }

  private drop(field: Typed["type"], key: string) {
    const pending = this.typed.get(`${field}:${key}`);
    if (!pending) return;
    clearTimeout(pending.timer);
    this.typed.delete(`${field}:${key}`);
  }

  private flushTyped(field: Typed["type"], key: string) {
    const pending = this.typed.get(`${field}:${key}`);
    if (!pending) return;
    this.drop(field, key);
    this.queue.push(pending.op);
  }

  private async pump() {
    if (this.running || this.failed) return;
    this.running = true;
    try {
      while (this.queue.length > 0) {
        await this.run(this.queue[0]);
        this.queue.shift();
      }
    } catch {
      this.failed = true;
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

  private async run(op: Op) {
    if (op.type === "create") {
      const res = await this.send("POST", "/api/v1/todos", {
        title: op.title,
        note: op.note,
        parentId: this.parentId(op.parent),
        index: op.index,
      });
      if (!res.ok) throw new Error(`create failed with ${res.status}`);
      const { todo } = (await res.json()) as { todo: { id: number } };
      this.track([[op.key, todo.id]]);
      return;
    }
    const path = `/api/v1/todos/${this.serverId(op.key)}`;
    const res =
      op.type === "delete"
        ? await this.send("DELETE", op.removed.length > 1 ? `${path}?recursive=true` : path)
        : await this.send(
            "PATCH",
            path,
            op.type === "title"
              ? { title: op.title }
              : op.type === "note"
                ? { note: op.note }
                : op.type === "move"
                  ? { parentId: this.parentId(op.parent), index: op.index }
                  : op.fields,
          );
    if (!res.ok && !(op.type === "delete" && res.status === 404)) throw new Error(`${op.type} failed with ${res.status}`);
  }
}
