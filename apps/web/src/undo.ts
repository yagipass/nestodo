import { applyOps, sameOutline, splitsPair, undoFor, type Change, type Focus, type Op, type Outline } from "./outline";

const LIMIT = 50;
const TYPING_PAUSE = 1000;

type Field = "title" | "note";

type Typing = { key: string; field: Field; original: string; at: number; composing: boolean };

type Step = { ops: Op[]; focus?: Focus; after: Outline; typing?: Typing };

function editEnd(before: string, after: string) {
  const max = Math.min(before.length, after.length);
  let prefix = 0;
  while (prefix < max && before[prefix] === after[prefix]) prefix++;
  if (splitsPair(before, prefix) || splitsPair(after, prefix)) prefix--;
  let suffix = 0;
  while (suffix < max - prefix && before[before.length - 1 - suffix] === after[after.length - 1 - suffix]) suffix++;
  if (splitsPair(before, before.length - suffix) || splitsPair(after, after.length - suffix)) suffix--;
  return before.length - suffix;
}

export class UndoHistory {
  private readonly steps: Step[] = [];

  constructor(private readonly now = () => Date.now()) {}

  record(before: Outline, change: Change, focus?: Focus) {
    this.push({ ops: undoFor(before, change.ops), focus, after: change.outline });
  }

  type(before: Outline, after: Outline, key: string, field: Field, composing = false) {
    const now = this.now();
    const top = this.steps.at(-1);
    const typing = top?.typing;
    const going = typing && typing.key === key && typing.field === field && (typing.composing || now - typing.at <= TYPING_PAUSE);
    const original = going ? typing.original : before[key][field];
    const step =
      top && going
        ? top
        : this.push({ ops: [field === "title" ? { type: "title", key, title: original } : { type: "note", key, note: original }], after });
    step.after = after;
    step.focus = { key, pos: editEnd(original, after[key][field]), note: field === "note" };
    step.typing = { key, field, original, at: now, composing };
  }

  composed() {
    const typing = this.steps.at(-1)?.typing;
    if (!typing) return;
    typing.composing = false;
    typing.at = this.now();
  }

  end() {
    const top = this.steps.at(-1);
    if (top) top.typing = undefined;
  }

  undo(current: Outline): Change | null {
    const step = this.steps.pop();
    if (!step) return null;
    if (!sameOutline(current, step.after)) {
      this.steps.length = 0;
      return null;
    }
    return { outline: applyOps(current, step.ops), ops: step.ops, focus: step.focus };
  }

  private push(step: Step) {
    this.end();
    this.steps.push(step);
    if (this.steps.length > LIMIT) this.steps.shift();
    return step;
  }
}
