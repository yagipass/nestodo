import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "./icons";
import type { Sync } from "./sync";
import { byDay, isPostKey, type Comment, type Entry, type TimelineItem } from "./thread";
import { Tokens } from "./tokens";
import { MAX_COMMENT_LENGTH } from "./todos";

const FETCH_DELAY = 150;

const EVENT_LABELS = { created: "Created", completed: "Completed", reopened: "Reopened" } as const;

const EVENT_ICONS = { created: "add", completed: "tick", reopened: "reopen" } as const;

type Props = {
  todoKey: string;
  title: string;
  path: string[];
  done: boolean;
  count: number;
  serverId: number | null;
  query: string;
  sync: Sync;
  editable: boolean;
  avatar: ReactNode;
  focusRequest: number;
  draft: string;
  onDraft: (key: string, update: (current: string) => string) => void;
  onCount: (key: string, delta: number) => void;
  onChanged: () => void;
  onBack: () => void;
  onClose: () => void;
};

function composingKey(event: KeyboardEvent) {
  return event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229;
}

function isSubmit(event: KeyboardEvent) {
  const { key, shiftKey, ctrlKey, metaKey, altKey } = event;
  return isPostKey({ key, shiftKey, ctrlKey, metaKey, altKey, isComposing: composingKey(event) });
}

async function timelineOf(id: number) {
  const res = await fetch(`/api/v1/todos/${id}/timeline`, { headers: { Accept: "application/json" } });
  if (!res.ok) throw new Error(`timeline failed with ${res.status}`);
  return ((await res.json()) as { items: TimelineItem[] }).items;
}

function DeleteDialog({ comment, onConfirm, onClose }: { comment: Comment; onConfirm: () => void; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const action = useRef<HTMLButtonElement>(null);

  useLayoutEffect(() => {
    dialog.current?.showModal();
    action.current?.focus();
  }, []);

  return (
    <dialog ref={dialog} className="confirm" aria-labelledby="delete-comment-title" onClose={onClose}>
      <h2 id="delete-comment-title">Delete this comment?</h2>
      <p className="confirm-quote">{comment.body}</p>
      <div className="confirm-actions">
        <button type="button" className="pill" onClick={() => dialog.current?.close()}>
          Cancel
        </button>
        <button ref={action} type="button" className="pill danger" onClick={onConfirm}>
          Delete
        </button>
      </div>
    </dialog>
  );
}

export function ThreadPane({
  todoKey,
  title,
  path,
  done,
  count,
  serverId,
  query,
  sync,
  editable,
  avatar,
  focusRequest,
  draft,
  onDraft,
  onCount,
  onChanged,
  onBack,
  onClose,
}: Props) {
  const [items, setItems] = useState<TimelineItem[] | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [local, setLocal] = useState<Entry[]>([]);
  const [editing, setEditing] = useState<{ id: number; body: string } | null>(null);
  const [deleting, setDeleting] = useState<Comment | null>(null);
  const [reload, setReload] = useState(0);
  const list = useRef<HTMLOListElement>(null);
  const composer = useRef<HTMLTextAreaElement>(null);
  const shownDone = useRef(done);
  const counter = useRef(0);
  const mutations = useRef(0);

  useEffect(() => {
    if (serverId === null) {
      setItems([{ type: "event", kind: "created", at: new Date().toISOString() }]);
      return;
    }
    const stale = new AbortController();
    const timer = setTimeout(() => {
      const seen = mutations.current;
      timelineOf(serverId).then(
        (latest) => {
          if (stale.signal.aborted) return;
          if (mutations.current !== seen) {
            setReload((n) => n + 1);
            return;
          }
          setItems(latest);
          setLoadFailed(false);
          setLocal((entries) => entries.filter((entry) => entry.type === "pending"));
        },
        () => {
          if (!stale.signal.aborted) setLoadFailed(true);
        },
      );
    }, FETCH_DELAY);
    return () => {
      stale.abort();
      clearTimeout(timer);
    };
  }, [serverId, reload]);

  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState === "visible" && !sync.pending) setReload((n) => n + 1);
    };
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [sync]);

  useEffect(() => {
    if (shownDone.current === done) return;
    shownDone.current = done;
    setLocal((entries) => [...entries, { type: "event", kind: done ? "completed" : "reopened", at: new Date().toISOString() }]);
  }, [done]);

  useEffect(() => {
    if (focusRequest === 0) return;
    const area = composer.current;
    area?.focus();
    area?.setSelectionRange(area.value.length, area.value.length);
  }, [focusRequest]);

  const entries: Entry[] = items === null ? [] : [...items, ...local];
  const days = byDay(entries, new Date());
  const written = entries.some((entry) => entry.type !== "event");

  useLayoutEffect(() => {
    const element = list.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [entries.length]);

  function post() {
    const body = draft.trim();
    if (!editable || body === "") return;
    counter.current += 1;
    const key = `pending${counter.current}`;
    const settled = () => setLocal((current) => current.filter((entry) => !(entry.type === "pending" && entry.key === key)));
    setLocal((current) => [...current, { type: "pending", key, body, at: new Date().toISOString() }]);
    onDraft(todoKey, () => "");
    onCount(todoKey, 1);
    mutations.current += 1;
    sync.comment(todoKey, body).then(
      (comment) => {
        mutations.current += 1;
        settled();
        setItems((current) => [...(current ?? []), { type: "comment", ...comment }]);
        onChanged();
      },
      () => {
        settled();
        onCount(todoKey, -1);
        onDraft(todoKey, (current) => (current === "" ? body : current));
      },
    );
  }

  function save() {
    if (!editing || !editable) return;
    const body = editing.body.trim();
    if (body === "") return;
    const { id } = editing;
    setEditing(null);
    setItems((current) => current?.map((item) => (item.type === "comment" && item.id === id ? { ...item, body } : item)) ?? null);
    mutations.current += 1;
    sync.editComment(id, body).then(
      (comment) => {
        mutations.current += 1;
        setItems((current) => current?.map((item) => (item.type === "comment" && item.id === id ? { type: "comment", ...comment } : item)) ?? null);
        onChanged();
      },
      () => {},
    );
  }

  function remove(comment: Comment) {
    setDeleting(null);
    setItems((current) => current?.filter((item) => !(item.type === "comment" && item.id === comment.id)) ?? null);
    onCount(todoKey, -1);
    mutations.current += 1;
    sync.deleteComment(comment.id).then(
      () => {
        mutations.current += 1;
        onChanged();
      },
      () => {},
    );
  }

  function onComposerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (composingKey(event)) return;
    if (isSubmit(event)) {
      event.preventDefault();
      post();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onBack();
    }
  }

  function onEditKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (composingKey(event)) return;
    if (isSubmit(event)) {
      event.preventDefault();
      save();
    } else if (event.key === "Escape") {
      event.preventDefault();
      setEditing(null);
    }
  }

  return (
    <aside className="thread" aria-label={`Comments on ${title || "Untitled"}`}>
      <header className="thread-head">
        <div className="thread-heading">
          {path.length > 0 && <p className="thread-path">{path.join(" › ")}</p>}
          <h2>{title || "Untitled"}</h2>
          <p className="thread-meta">
            <span className={done ? "thread-state done" : "thread-state"}>{done ? "Completed" : "Open"}</span>
            {count === 0 ? "No comments yet" : count === 1 ? "1 comment" : `${count} comments`}
          </p>
        </div>
        <button type="button" className="icon-button" aria-label="Close comments" data-tip="Close (Esc)" onClick={onClose}>
          <Icon name="close" size={18} />
        </button>
      </header>

      <ol ref={list} className="thread-log" aria-busy={items === null}>
        {loadFailed && items === null && (
          <li className="thread-notice">
            Couldn't load the comments.
            <button type="button" className="text-button" onClick={() => setReload((n) => n + 1)}>
              Try again
            </button>
          </li>
        )}
        {days.map((day, dayIndex) => (
          <li key={`${day.label}-${dayIndex}`} className="thread-day">
            <h3 className="thread-date">{day.label}</h3>
            <ol>
              {day.entries.map(({ entry, time }, index) => {
                if (entry.type === "event") {
                  return (
                    <li key={`event-${index}`} className={`thread-event ${entry.kind}`}>
                      <span className="thread-node" aria-hidden="true">
                        <Icon name={EVENT_ICONS[entry.kind]} size={12} strokeWidth={3} />
                      </span>
                      <span>
                        <strong>{EVENT_LABELS[entry.kind]}</strong> at <time dateTime={entry.at}>{time}</time>
                      </span>
                    </li>
                  );
                }
                if (entry.type === "pending") {
                  return (
                    <li key={entry.key} className="thread-comment pending">
                      <span className="thread-avatar" aria-hidden="true">
                        {avatar}
                      </span>
                      <div className="thread-comment-body">
                        <p className="thread-comment-meta">Sending…</p>
                        <p className="thread-text">
                          <Tokens text={entry.body} query={query} focusable />
                        </p>
                      </div>
                    </li>
                  );
                }
                const edit = editing?.id === entry.id ? editing : null;
                return (
                  <li key={entry.id} className="thread-comment">
                    <span className={entry.via === "mcp" ? "thread-avatar mcp" : "thread-avatar"} aria-hidden="true">
                      {entry.via === "mcp" ? <Icon name="bot" size={16} /> : avatar}
                    </span>
                    <div className="thread-comment-body">
                      <p className="thread-comment-meta">
                        {entry.via === "mcp" && <span className="thread-via">via MCP</span>}
                        <time dateTime={entry.at}>{time}</time>
                        {entry.editedAt && <span>(edited)</span>}
                        {editable && !edit && (
                          <span className="thread-actions">
                            <button
                              type="button"
                              className="icon-button"
                              aria-label="Edit this comment"
                              onClick={() => setEditing({ id: entry.id, body: entry.body })}
                            >
                              <Icon name="edit" size={15} />
                            </button>
                            <button type="button" className="icon-button" aria-label="Delete this comment" onClick={() => setDeleting(entry)}>
                              <Icon name="trash" size={15} />
                            </button>
                          </span>
                        )}
                      </p>
                      {edit ? (
                        <div className="thread-edit">
                          <textarea
                            aria-label="Edit the comment"
                            value={edit.body}
                            maxLength={MAX_COMMENT_LENGTH}
                            rows={3}
                            autoFocus
                            onChange={(event) => setEditing({ id: entry.id, body: event.target.value })}
                            onKeyDown={onEditKeyDown}
                          />
                          <div className="thread-edit-actions">
                            <button type="button" className="pill" onClick={() => setEditing(null)}>
                              Cancel
                            </button>
                            <button type="button" className="pill primary" disabled={edit.body.trim() === ""} onClick={save}>
                              Save
                            </button>
                          </div>
                        </div>
                      ) : (
                        <p className="thread-text">
                          <Tokens text={entry.body} query={query} focusable />
                        </p>
                      )}
                    </div>
                  </li>
                );
              })}
            </ol>
          </li>
        ))}
        {items !== null && !written && <li className="thread-notice">Nothing written yet. Note what you did or found.</li>}
      </ol>

      <form
        className="thread-compose"
        onSubmit={(event) => {
          event.preventDefault();
          post();
        }}
      >
        <textarea
          ref={composer}
          aria-label="Add a comment"
          placeholder="Add to the log…"
          value={draft}
          readOnly={!editable}
          maxLength={MAX_COMMENT_LENGTH}
          rows={3}
          onChange={(event) => {
            const value = event.target.value;
            onDraft(todoKey, () => value);
          }}
          onKeyDown={onComposerKeyDown}
        />
        <div className="thread-compose-foot">
          <span className="thread-hint">
            <kbd>⇧</kbd>
            <kbd>Enter</kbd> to post, <kbd>Esc</kbd> back to the item
          </span>
          <button type="submit" className="pill primary" disabled={!editable || draft.trim() === ""}>
            Post
          </button>
        </div>
      </form>

      {deleting && <DeleteDialog comment={deleting} onConfirm={() => remove(deleting)} onClose={() => setDeleting(null)} />}
    </aside>
  );
}
