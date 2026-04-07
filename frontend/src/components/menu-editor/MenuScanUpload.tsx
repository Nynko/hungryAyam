import { createSignal, Show, For, onCleanup } from "solid-js";
import type { MenuScanResponse } from "@bindings/MenuScanResponse";
import type { ApiResponse } from "@bindings/ApiResponse";

interface MenuScanUploadProps {
  onScanComplete: (result: MenuScanResponse) => void;
  onCancel: () => void;
}

const MAX_FILES = 5;
const ACCEPTED_TYPES = "image/jpeg,image/png,image/webp,image/gif,image/heic,.heic";

type ScanMode = "images" | "url";

export default function MenuScanUpload(props: MenuScanUploadProps) {
  let inputRef!: HTMLInputElement;
  const [mode, setMode] = createSignal<ScanMode>("images");
  const [files, setFiles] = createSignal<File[]>([]);
  const [previews, setPreviews] = createSignal<string[]>([]);
  const [url, setUrl] = createSignal("");
  const [scanning, setScanning] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [progress, setProgress] = createSignal("");

  // Clean up object URLs on unmount
  onCleanup(() => {
    for (const u of previews()) {
      URL.revokeObjectURL(u);
    }
  });

  const addFiles = (newFiles: FileList | File[]) => {
    const current = files();
    const remaining = MAX_FILES - current.length;
    if (remaining <= 0) return;

    const toAdd = Array.from(newFiles).slice(0, remaining);
    const newPreviews = toAdd.map((f) => URL.createObjectURL(f));

    setFiles([...current, ...toAdd]);
    setPreviews([...previews(), ...newPreviews]);
    setError(null);
  };

  const removeFile = (index: number) => {
    URL.revokeObjectURL(previews()[index]);
    setFiles(files().filter((_, i) => i !== index));
    setPreviews(previews().filter((_, i) => i !== index));
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    if (e.dataTransfer?.files) {
      addFiles(e.dataTransfer.files);
    }
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
  };

  const convertHeicIfNeeded = async (file: File): Promise<File> => {
    const isHeic =
      file.type === "image/heic" ||
      file.type === "image/heif" ||
      file.name.toLowerCase().endsWith(".heic") ||
      file.name.toLowerCase().endsWith(".heif");

    if (!isHeic) return file;

    const heic2any = (await import("heic2any")).default;
    const blob = await heic2any({
      blob: file,
      toType: "image/jpeg",
      quality: 0.85,
    });
    const converted = Array.isArray(blob) ? blob[0] : blob;
    return new File(
      [converted],
      file.name.replace(/\.hei[cf]$/i, ".jpg"),
      { type: "image/jpeg" }
    );
  };

  const handleScanImages = async () => {
    const currentFiles = files();
    if (currentFiles.length === 0) return;

    setScanning(true);
    setError(null);

    try {
      setProgress("Preparing images...");
      const prepared = await Promise.all(currentFiles.map(convertHeicIfNeeded));

      setProgress("Scanning menu (this may take 15–30 seconds)...");
      const form = new FormData();
      for (const f of prepared) {
        form.append("images", f);
      }

      const res = await fetch("/api/menu-scan", { method: "POST", body: form });
      const json: ApiResponse<MenuScanResponse> = await res.json();

      if (!res.ok || !json.success || json.data == null) {
        throw new Error(json.error ?? `Scan failed (${res.status})`);
      }

      props.onScanComplete(json.data);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      setScanning(false);
      setProgress("");
    }
  };

  const handleScanUrl = async () => {
    const menuUrl = url().trim();
    if (!menuUrl) return;

    setScanning(true);
    setError(null);

    try {
      setProgress("Fetching page and extracting menu (this may take 30–60 seconds)...");

      const res = await fetch("/api/menu-scan-url", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: menuUrl }),
      });
      const json: ApiResponse<MenuScanResponse> = await res.json();

      if (!res.ok || !json.success || json.data == null) {
        throw new Error(json.error ?? `Scan failed (${res.status})`);
      }

      props.onScanComplete(json.data);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      setScanning(false);
      setProgress("");
    }
  };

  const handleScan = () => {
    if (mode() === "url") {
      handleScanUrl();
    } else {
      handleScanImages();
    }
  };

  const canScan = () => {
    if (mode() === "url") return url().trim().length > 0;
    return files().length > 0;
  };

  return (
    <div class="container" style={{ "max-width": "700px" }}>
      <div class="has-text-centered mb-5">
        <h2 class="title is-4">Automatic menu creation</h2>
        <p class="subtitle is-6 has-text-grey">
          Upload photos of a menu or paste a link to a menu page.
        </p>
      </div>

      {/* Mode tabs */}
      <div class="tabs is-centered is-boxed mb-4">
        <ul>
          <li classList={{ "is-active": mode() === "images" }}>
            <a onClick={() => setMode("images")}>
              <span class="icon is-small"><span>📸</span></span>
              <span>Upload Photos</span>
            </a>
          </li>
          <li classList={{ "is-active": mode() === "url" }}>
            <a onClick={() => setMode("url")}>
              <span class="icon is-small"><span>🔗</span></span>
              <span>Paste a Link</span>
            </a>
          </li>
        </ul>
      </div>

      {/* Error display */}
      <Show when={error()}>
        <div class="notification is-danger is-light mb-4">
          <button class="delete" onClick={() => setError(null)} />
          <strong>Error:</strong> {error()}
        </div>
      </Show>

      {/* ── Image upload mode ──────────────────────────────── */}
      <Show when={mode() === "images" && !scanning()}>
        <div
          class="box has-text-centered"
          style={{
            border: "2px dashed hsl(0, 0%, 71%)",
            cursor: "pointer",
            "min-height": "150px",
            display: "flex",
            "flex-direction": "column",
            "align-items": "center",
            "justify-content": "center",
          }}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          onClick={() => inputRef.click()}
        >
          <span style={{ "font-size": "2.5rem" }}>📸</span>
          <p class="mt-2">
            <strong>Drop images here</strong> or click to browse
          </p>
          <p class="has-text-grey is-size-7 mt-1">
            JPEG, PNG, WebP, or HEIC — max {MAX_FILES} images, 10 MB each
          </p>
          <input
            ref={inputRef!}
            type="file"
            accept={ACCEPTED_TYPES}
            multiple
            style={{ display: "none" }}
            onChange={(e) => {
              if (e.currentTarget.files) {
                addFiles(e.currentTarget.files);
              }
              e.currentTarget.value = "";
            }}
          />
        </div>

        {/* Thumbnail grid */}
        <Show when={files().length > 0}>
          <div class="mt-4 mb-4">
            <p class="is-size-7 has-text-grey mb-2">
              {files().length}/{MAX_FILES} images selected
            </p>
            <div class="columns is-multiline is-mobile">
              <For each={previews()}>
                {(previewUrl, index) => (
                  <div class="column is-3-tablet is-4-mobile">
                    <div class="card">
                      <div class="card-image">
                        <figure class="image is-4by3">
                          <img
                            src={previewUrl}
                            alt={`Menu photo ${index() + 1}`}
                            style={{ "object-fit": "cover" }}
                          />
                        </figure>
                      </div>
                      <footer class="card-footer">
                        <a
                          class="card-footer-item has-text-danger is-size-7"
                          onClick={(e) => {
                            e.preventDefault();
                            removeFile(index());
                          }}
                        >
                          Remove
                        </a>
                      </footer>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>
        </Show>
      </Show>

      {/* ── URL mode ───────────────────────────────────────── */}
      <Show when={mode() === "url" && !scanning()}>
        <div class="box">
          <div class="field">
            <label class="label">Menu page URL</label>
            <div class="control has-icons-left">
              <input
                class="input"
                type="url"
                placeholder="https://restaurant.com/menu"
                value={url()}
                onInput={(e) => {
                  setUrl(e.currentTarget.value);
                  setError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && canScan()) handleScan();
                }}
              />
              <span class="icon is-left"><span>🔗</span></span>
            </div>
            <p class="help">
              Paste a link to a restaurant's online menu. The page will be fetched and analyzed, including any food images.
            </p>
          </div>
        </div>
      </Show>

      {/* Scanning progress */}
      <Show when={scanning()}>
        <div class="has-text-centered py-6">
          <progress class="progress is-primary is-small" max="100" />
          <p class="has-text-grey mt-2">{progress()}</p>
        </div>
      </Show>

      {/* Action buttons */}
      <div class="buttons is-centered mt-5">
        <button
          class="button is-primary"
          classList={{ "is-loading": scanning() }}
          disabled={scanning() || !canScan()}
          onClick={handleScan}
        >
          {mode() === "url" ? "Scan from URL" : "Scan Menu"}
        </button>
        <button
          class="button is-light"
          disabled={scanning()}
          onClick={() => props.onCancel()}
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
