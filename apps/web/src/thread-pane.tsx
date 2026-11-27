import { Fragment, useEffect, useLayoutEffect, useRef, useState, type ClipboardEvent, type DragEvent, type KeyboardEvent, type ReactNode } from "react";
import { base64Of, commentImageFrom } from "./avatar-image";
import { MAX_COMMENT_IMAGES, type CommentImage } from "./comment-images";
import { Icon } from "./icons";
import { problemOf } from "./problem";
import type { Sync } from "./sync";
import { byDay, isPostKey, type Attachment, type Comment, type Entry, type TimelineItem } from "./thread";
import { Tokens } from "./tokens";
import { MAX_COMMENT_LENGTH } from "./todos";

const FETCH_DELAY = 150;

const NO_LINKS = [] as const;

type MarkdownView = typeof import("./markdown-view").Markdown;

let markdownView: MarkdownView | null = null;

function loadMarkdownView() {
  return import("./markdown-view").then(({ Markdown }) => (markdownView = Markdown));
}

const EVENT_LABELS = { created: "Created", completed: "Completed", reopened: "Reopened" } as const;

const EVENT_ICONS = { created: "add", completed: "tick", reopened: "reopen" } as const;

let attachments = 0;

export type Crumb = { key: string; title: string; icon: ReactNode };

type Props = {
  todoKey: string;
  title: string;
  path: Crumb[];
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
  attached: readonly Attachment[];
  onAttached: (key: string, update: (current: readonly Attachment[]) => readonly Attachment[]) => void;
  onCount: (key: string, delta: number) => void;
  onBack: () => void;
  onClose: () => void;
};

function composingKey(event: KeyboardEvent) {
  return event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229;
}

function isSubmit(event: KeyboardEvent) {
  const { key, shiftKey, altKey } = event;
  return isPostKey({ key, shiftKey, altKey, isComposing: composingKey(event) });
}

async function timelineOf(id: number) {
  const res = await fetch(`/api/v1/todos/${id}/timeline`, { headers: { Accept: "application/json" } });
  if (!res.ok) throw new Error(`timeline failed with ${res.status}`);
  return ((await res.json()) as { items: TimelineItem[] }).items;
}

async function uploadCommentImage(file: Blob) {
  const image = await commentImageFrom(file)
    .then(base64Of)
    .catch(() => null);
  if (image === null) throw new Error("The image could not be read.");
  const res = await fetch("/api/v1/comment-images", {
    method: "POST",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ image }),
  }).catch(() => null);
  if (!res?.ok) throw new Error(await problemOf(res, "Could not add the image."));
  return ((await res.json()) as { image: CommentImage }).image;
}

function imageFiles(files: FileList) {
  return [...files].filter((file) => file.type.startsWith("image/"));
}

function imageCount(count: number) {
  return count === 1 ? "1 image" : `${count} images`;
}

function CommentImages({ urls, onOpen }: { urls: readonly string[]; onOpen: (url: string) => void }) {
  if (urls.length === 0) return null;
  return (
    <div className="thread-images">
      {urls.map((url, index) => (
        <button key={url} type="button" className="thread-image" aria-label={`Open image ${index + 1} of ${urls.length}`} onClick={() => onOpen(url)}>
          <img src={url} alt="" loading="lazy" />
        </button>
      ))}
    </div>
  );
}

function ImageDialog({ url, onClose }: { url: string; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);

  useLayoutEffect(() => {
    dialog.current?.showModal();
  }, []);

  return (
    <dialog ref={dialog} className="image-view" aria-label="Image" onClose={onClose} onClick={() => dialog.current?.close()}>
      <img src={url} alt="" />
      <button type="button" className="icon-button image-view-close" aria-label="Close image" autoFocus>
        <Icon name="close" size={18} />
      </button>
    </dialog>
  );
}

function Body({ text, query, view: View }: { text: string; query: string; view: MarkdownView | null }) {
  if (View) return <View text={text} query={query} />;
  return (
    <div className="thread-text">
      <p>
        <Tokens text={text} query={query} links={NO_LINKS} />
      </p>
    </div>
  );
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
      <p className="confirm-quote">{comment.body || imageCount(comment.images.length)}</p>
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
  attached,
  onAttached,
  onCount,
  onBack,
  onClose,
}: Props) {
  const [items, setItems] = useState<TimelineItem[] | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [local, setLocal] = useState<Entry[]>([]);
  const [editing, setEditing] = useState<{ id: number; body: string; hasImages: boolean } | null>(null);
  const [deleting, setDeleting] = useState<Comment | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const list = useRef<HTMLOListElement>(null);
  const composer = useRef<HTMLTextAreaElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  const shownDone = useRef(done);
  const counter = useRef(0);
  const mutations = useRef(0);
  const [view, setView] = useState(() => markdownView);

  useEffect(() => {
    if (view) return;
    let gone = false;
    loadMarkdownView().then(
      (loaded) => {
        if (!gone) setView(() => loaded);
      },
      () => {},
    );
    return () => {
      gone = true;
    };
  }, [view]);

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
  const uploading = attached.some((attachment) => attachment.image === null);
  const postable = editable && !uploading && (draft.trim() !== "" || attached.length > 0);

  useLayoutEffect(() => {
    const element = list.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [entries.length]);

  function attach(files: readonly File[]) {
    if (!editable || files.length === 0) return;
    const room = Math.max(0, MAX_COMMENT_IMAGES - attached.length);
    setProblem(files.length > room ? `A comment can have up to ${MAX_COMMENT_IMAGES} images` : null);
    for (const file of files.slice(0, room)) {
      attachments += 1;
      const key = `attachment${attachments}`;
      const preview = URL.createObjectURL(file);
      onAttached(todoKey, (current) => [...current, { key, preview, image: null }]);
      uploadCommentImage(file).then(
        (image) => onAttached(todoKey, (current) => current.map((attachment) => (attachment.key === key ? { ...attachment, image } : attachment))),
        (error: unknown) => {
          URL.revokeObjectURL(preview);
          onAttached(todoKey, (current) => current.filter((attachment) => attachment.key !== key));
          setProblem(error instanceof Error ? error.message : "Could not add the image.");
        },
      );
    }
  }

  function detach(attachment: Attachment) {
    URL.revokeObjectURL(attachment.preview);
    onAttached(todoKey, (current) => current.filter((other) => other.key !== attachment.key));
  }

  function onPaste(event: ClipboardEvent<HTMLTextAreaElement>) {
    const files = imageFiles(event.clipboardData.files);
    if (files.length === 0 || event.clipboardData.types.includes("text/plain")) return;
    event.preventDefault();
    attach(files);
  }

  function onDragOver(event: DragEvent<HTMLFormElement>) {
    if (editable && event.dataTransfer.types.includes("Files")) event.preventDefault();
  }

  function onDrop(event: DragEvent<HTMLFormElement>) {
    if (!editable || !event.dataTransfer.types.includes("Files")) return;
    event.preventDefault();
    const files = imageFiles(event.dataTransfer.files);
    if (files.length === 0) setProblem("Only images can be added");
    else attach(files);
  }

  function post() {
    if (!postable) return;
    const body = draft.trim();
    const sent = attached;
    counter.current += 1;
    const key = `pending${counter.current}`;
    const settled = () => setLocal((current) => current.filter((entry) => !(entry.type === "pending" && entry.key === key)));
    const previews = sent.map((attachment) => attachment.preview);
    setLocal((current) => [...current, { type: "pending", key, body, at: new Date().toISOString(), images: previews }]);
    onDraft(todoKey, () => "");
    onAttached(todoKey, () => []);
    setProblem(null);
    onCount(todoKey, 1);
    mutations.current += 1;
    sync.comment(todoKey, body, sent.flatMap((attachment) => (attachment.image ? [attachment.image.id] : []))).then(
      (comment) => {
        mutations.current += 1;
        settled();
        setItems((current) => [...(current ?? []), { type: "comment", ...comment }]);
        for (const preview of previews) URL.revokeObjectURL(preview);
      },
      () => {
        settled();
        onCount(todoKey, -1);
        onDraft(todoKey, (current) => (current === "" ? body : current));
        onAttached(todoKey, (current) => (current.length === 0 ? sent : current));
      },
    );
  }

  function save() {
    if (!editing || !editable) return;
    const body = editing.body.trim();
    if (body === "" && !editing.hasImages) return;
    const { id } = editing;
    setEditing(null);
    setItems((current) => current?.map((item) => (item.type === "comment" && item.id === id ? { ...item, body } : item)) ?? null);
    mutations.current += 1;
    sync.editComment(id, body).then(
      (comment) => {
        mutations.current += 1;
        setItems((current) => current?.map((item) => (item.type === "comment" && item.id === id ? { type: "comment", ...comment } : item)) ?? null);
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
          {path.length > 0 && (
            <p className="thread-path">
              {path.map((crumb, index) => (
                <Fragment key={crumb.key}>
                  {index > 0 && " › "}
                  {crumb.icon && <span className="thread-path-icon">{crumb.icon}</span>}
                  {crumb.title}
                </Fragment>
              ))}
            </p>
          )}
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
                        {entry.body !== "" && <Body text={entry.body} query={query} view={view} />}
                        <CommentImages urls={entry.images} onOpen={setViewing} />
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
                              onClick={() => setEditing({ id: entry.id, body: entry.body, hasImages: entry.images.length > 0 })}
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
                            onChange={(event) => setEditing({ ...edit, body: event.target.value })}
                            onKeyDown={onEditKeyDown}
                          />
                          <div className="thread-edit-actions">
                            <button type="button" className="pill" onClick={() => setEditing(null)}>
                              Cancel
                            </button>
                            <button type="button" className="pill primary" disabled={edit.body.trim() === "" && !edit.hasImages} onClick={save}>
                              Save
                            </button>
                          </div>
                        </div>
                      ) : (
                        entry.body !== "" && <Body text={entry.body} query={query} view={view} />
                      )}
                      <CommentImages urls={entry.images.map((image) => image.url)} onOpen={setViewing} />
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
        onDragOver={onDragOver}
        onDrop={onDrop}
      >
        {attached.length > 0 && (
          <ul className="thread-attachments" aria-label="Images to post">
            {attached.map((attachment) => (
              <li key={attachment.key} className={attachment.image ? "thread-attachment" : "thread-attachment uploading"} aria-busy={attachment.image === null}>
                <img src={attachment.preview} alt="" />
                <button type="button" className="thread-attachment-remove" aria-label="Remove image" onClick={() => detach(attachment)}>
                  <Icon name="close" size={12} strokeWidth={3} />
                </button>
              </li>
            ))}
          </ul>
        )}
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
          onPaste={onPaste}
        />
        {problem && (
          <p className="thread-problem" role="alert">
            {problem}
          </p>
        )}
        <div className="thread-compose-foot">
          <span className="thread-hint">
            <kbd>Enter</kbd> to post, <kbd>Esc</kbd> back to the item
          </span>
          <div className="thread-compose-actions">
            <input
              ref={picker}
              type="file"
              accept="image/*"
              multiple
              hidden
              onChange={(event) => {
                attach([...(event.target.files ?? [])]);
                event.target.value = "";
              }}
            />
            <button
              type="button"
              className="icon-button"
              aria-label="Add images"
              disabled={!editable || attached.length >= MAX_COMMENT_IMAGES}
              onClick={() => picker.current?.click()}
            >
              <Icon name="image" size={18} />
            </button>
            <button type="submit" className="pill primary" disabled={!postable}>
              Post
            </button>
          </div>
        </div>
      </form>

      {deleting && <DeleteDialog comment={deleting} onConfirm={() => remove(deleting)} onClose={() => setDeleting(null)} />}
      {viewing && <ImageDialog url={viewing} onClose={() => setViewing(null)} />}
    </aside>
  );
}
