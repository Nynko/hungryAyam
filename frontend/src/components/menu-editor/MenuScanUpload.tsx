import { createSignal, Show, For, onCleanup } from "solid-js";
import type { MenuScanResponse } from "@bindings/MenuScanResponse";
import type { ApiResponse } from "@bindings/ApiResponse";

interface MenuScanUploadProps {
  onScanComplete: (result: MenuScanResponse) => void;
  onCancel: () => void;
}

const MAX_FILES = 5;
const ACCEPTED_TYPES = "image/jpeg,image/png,image/webp,image/gif,image/heic,.heic";

export default function MenuScanUpload(props: MenuScanUploadProps) {
  let inputRef!: HTMLInputElement;
  const [files, setFiles] = createSignal<File[]>([]);
  const [previews, setPreviews] = createSignal<string[]>([]);
  const [url, setUrl] = createSignal("");
  const [scanning, setScanning] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [progress, setProgress] = createSignal("");

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

  const canScan = () => files().length > 0 || url().trim().length > 0;

  const handleScan = async () => {
    setScanning(true);
    setError(null);

    try {
      setProgress("Preparing scan...");

      const form = new FormData();

      if (files().length > 0) {
        const prepared = await Promise.all(files().map(convertHeicIfNeeded));
        for (const f of prepared) {
          form.append("images", f);
        }
      }

      const trimmedUrl = url().trim();
      if (trimmedUrl) {
        form.append("url", trimmedUrl);
      }

      setProgress("Submitting scan request...");

      const res = await fetch("/api/menu-scan", { method: "POST", body: form });

      const text = await res.text();
      let json: ApiResponse<{ job_id: string }>;
      try {
        json = JSON.parse(text);
      } catch {
        throw new Error(`Server returned an unexpected response (${res.status}). Please try again.`);
      }

      if (!res.ok || !json.success || json.data == null) {
        throw new Error(json.error ?? `Failed to start scan (${res.status})`);
      }

      const { job_id } = json.data;

      // Poll until the job completes (max 10 minutes)
      const POLL_INTERVAL_MS = 3000;
      const MAX_POLLS = 200;

      for (let i = 0; i < MAX_POLLS; i++) {
        await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
        const elapsed = Math.round(((i + 1) * POLL_INTERVAL_MS) / 1000);
        setProgress(`Analyzing menu... (${elapsed}s elapsed)`);

        const pollRes = await fetch(`/api/menu-scan-jobs/${job_id}`);
        const pollText = await pollRes.text();
        let pollJson: ApiResponse<{ status: string; result?: MenuScanResponse; error?: string }>;
        try {
          pollJson = JSON.parse(pollText);
        } catch {
          throw new Error("Failed to check scan status. Please try again.");
        }

        if (!pollRes.ok || !pollJson.success || pollJson.data == null) {
          throw new Error(pollJson.error ?? "Failed to check scan status.");
        }

        const { status, result, error: jobError } = pollJson.data;

        if (status === "completed" && result != null) {
          props.onScanComplete(result);
          return;
        }

        if (status === "failed") {
          throw new Error(jobError ?? "Scan failed. Please try again.");
        }
      }

      throw new Error("Scan timed out. Please try again.");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      setScanning(false);
      setProgress("");
    }
  };

  return (
    <div class="container" style={{ "max-width": "700px" }}>
      <div class="has-text-centered mb-5">
        <h2 class="title is-4">Automatic menu creation</h2>
        <p class="subtitle is-6 has-text-grey">
          Add photos, a URL, or both — they'll be combined for the best result.
        </p>
      </div>

      <Show when={error()}>
        <div class="notification is-danger is-light mb-4">
          <button class="delete" onClick={() => setError(null)} />
          <strong>Error:</strong> {error()}
        </div>
      </Show>

      <Show when={!scanning()}>
        {/* Image upload */}
        <div
          class="box has-text-centered mb-4"
          style={{
            border: "2px dashed hsl(0, 0%, 71%)",
            cursor: "pointer",
            "min-height": "130px",
            display: "flex",
            "flex-direction": "column",
            "align-items": "center",
            "justify-content": "center",
          }}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          onClick={() => inputRef.click()}
        >
          <span style={{ "font-size": "2rem" }}>📸</span>
          <p class="mt-2">
            <strong>Drop images here</strong> or click to browse
          </p>
          <p class="has-text-grey is-size-7 mt-1">
            JPEG, PNG, WebP, or HEIC — up to {MAX_FILES} images, 10 MB each
          </p>
          <input
            ref={inputRef!}
            type="file"
            accept={ACCEPTED_TYPES}
            multiple
            style={{ display: "none" }}
            onChange={(e) => {
              if (e.currentTarget.files) addFiles(e.currentTarget.files);
              e.currentTarget.value = "";
            }}
          />
        </div>

        <Show when={files().length > 0}>
          <div class="mb-4">
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

        {/* URL input */}
        <div class="box">
          <div class="field">
            <label class="label">Menu page URL (optional)</label>
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
              <span class="icon is-left">🔗</span>
            </div>
            <p class="help">
              The page will be fetched and its images and text analyzed alongside any photos you uploaded.
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

      <div class="buttons is-centered mt-5">
        <button
          class="button is-primary"
          classList={{ "is-loading": scanning() }}
          disabled={scanning() || !canScan()}
          onClick={handleScan}
        >
          Scan Menu
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
