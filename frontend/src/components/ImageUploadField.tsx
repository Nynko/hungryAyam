import { createSignal, Show } from "solid-js";
import { isImageSrc } from "@/lib/imageUrl";

interface ImageUploadFieldProps {
  /** Current value: absolute URL, /uploads/… path, emoji string, or null. */
  value: string | null;
  /** Called with the new value after upload/emoji entry, or null when cleared. */
  onChange: (url: string | null) => void;
  disabled?: boolean;
}

const ACCEPTED_TYPES = "image/jpeg,image/png,image/webp,image/gif";

type Mode = "upload" | "emoji";

export default function ImageUploadField(props: ImageUploadFieldProps) {
  let inputRef!: HTMLInputElement;
  const [uploading, setUploading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [mode, setMode] = createSignal<Mode>("upload");
  const [emojiDraft, setEmojiDraft] = createSignal("");

  const isEmoji = () => !!props.value && !isImageSrc(props.value);

  const handleFileChange = async (e: Event) => {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;

    setError(null);
    setUploading(true);

    try {
      const form = new FormData();
      form.append("file", file);

      const res = await fetch("/api/uploads", { method: "POST", body: form });
      const json = await res.json();

      if (!res.ok || !json.success) {
        throw new Error(json.error ?? `Upload failed (${res.status})`);
      }

      props.onChange(json.data.url as string);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setUploading(false);
      inputRef.value = "";
    }
  };

  const handleEmojiConfirm = () => {
    const val = emojiDraft().trim();
    if (!val) return;
    setEmojiDraft("");
    setError(null);
    props.onChange(val);
  };

  const handleClear = () => {
    setError(null);
    setEmojiDraft("");
    props.onChange(null);
  };

  // ── No value yet ────────────────────────────────────────────────
  const EmptyState = () => (
    <div>
      <div class="tabs is-small mb-2">
        <ul>
          <li classList={{ "is-active": mode() === "upload" }}>
            <a onClick={() => setMode("upload")}>Upload</a>
          </li>
          <li classList={{ "is-active": mode() === "emoji" }}>
            <a onClick={() => setMode("emoji")}>Emoji</a>
          </li>
        </ul>
      </div>

      <Show when={mode() === "upload"}>
        <button
          type="button"
          class="button is-light"
          classList={{ "is-loading": uploading() }}
          disabled={props.disabled || uploading()}
          onClick={() => inputRef.click()}
        >
          <span class="icon"><span>📷</span></span>
          <span>Upload image</span>
        </button>
        <p class="help">JPEG, PNG, WebP or GIF · max 10 MB · saved as WebP</p>
      </Show>

      <Show when={mode() === "emoji"}>
        <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
          <input
            class="input"
            style={{ width: "5rem", "font-size": "1.5rem", "text-align": "center" }}
            type="text"
            maxlength="8"
            placeholder="🍽️"
            value={emojiDraft()}
            onInput={(e) => setEmojiDraft(e.currentTarget.value)}
            disabled={props.disabled}
          />
          <button
            type="button"
            class="button is-light"
            disabled={props.disabled || !emojiDraft().trim()}
            onClick={handleEmojiConfirm}
          >
            Use
          </button>
        </div>
        <p class="help">Paste or type an emoji</p>
      </Show>
    </div>
  );

  // ── Value set ───────────────────────────────────────────────────
  const FilledState = () => (
    <div class="is-flex is-align-items-flex-start" style={{ gap: "0.75rem" }}>
      {/* Preview */}
      <div
        style={{
          width: "80px",
          height: "80px",
          "flex-shrink": "0",
          overflow: "hidden",
          "border-radius": "4px",
          "background-color": "var(--bulma-scheme-main-bis)",
          display: "flex",
          "align-items": "center",
          "justify-content": "center",
        }}
      >
        <Show
          when={!isEmoji()}
          fallback={
            <span style={{ "font-size": "2.5rem", "line-height": "1" }}>{props.value}</span>
          }
        >
          <img
            src={props.value!}
            alt="Image preview"
            style={{ width: "100%", height: "100%", "object-fit": "cover" }}
            onError={(e) => { (e.currentTarget as HTMLImageElement).style.opacity = "0.3"; }}
          />
        </Show>
      </div>

      {/* Actions */}
      <div class="is-flex is-flex-direction-column" style={{ gap: "0.4rem" }}>
        <Show when={!isEmoji()}>
          <button
            type="button"
            class="button is-small is-light"
            classList={{ "is-loading": uploading() }}
            disabled={props.disabled || uploading()}
            onClick={() => inputRef.click()}
          >
            Replace image
          </button>
        </Show>
        <button
          type="button"
          class="button is-small is-danger is-outlined"
          disabled={props.disabled || uploading()}
          onClick={handleClear}
        >
          Remove
        </button>
      </div>
    </div>
  );

  return (
    <div class="field">
      <label class="label">Image</label>

      <input
        ref={inputRef}
        type="file"
        accept={ACCEPTED_TYPES}
        style={{ display: "none" }}
        disabled={props.disabled || uploading()}
        onChange={handleFileChange}
      />

      <Show when={props.value} fallback={<EmptyState />}>
        <FilledState />
      </Show>

      <Show when={error()}>
        <p class="help is-danger">{error()}</p>
      </Show>
    </div>
  );
}
