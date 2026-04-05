import { createSignal, onMount, Show } from "solid-js";
import { isAdmin } from "@/stores/authStore";
import type { ApiResponse } from "@bindings/ApiResponse";

export default function Admin() {
  // ── Magic link ──────────────────────────────────────────────────
  const [magicLink, setMagicLink] = createSignal<string | null>(null);
  const [magicLoading, setMagicLoading] = createSignal(false);
  const [copied, setCopied] = createSignal(false);
  const [magicError, setMagicError] = createSignal<string | null>(null);

  // ── Editor domain ───────────────────────────────────────────────
  const [editorDomain, setEditorDomain] = createSignal("");
  const [editorDomainSaved, setEditorDomainSaved] = createSignal<string | null>(null);
  const [domainLoading, setDomainLoading] = createSignal(false);
  const [domainSaveMsg, setDomainSaveMsg] = createSignal<string | null>(null);
  const [domainError, setDomainError] = createSignal<string | null>(null);

  onMount(async () => {
    if (!isAdmin()) return;

    // Fetch magic link
    setMagicLoading(true);
    try {
      const res = await fetch("/api/admin/magic-link");
      if (!res.ok) throw new Error(`${res.status}`);
      const json = await res.json();
      if (json.success && json.data) {
        const url = `${window.location.origin}/?access=${json.data}`;
        setMagicLink(url);
      }
    } catch {
      setMagicError("Failed to load magic link.");
    } finally {
      setMagicLoading(false);
    }

    // Fetch editor domain
    try {
      const res = await fetch("/api/admin/settings/editor-domain");
      if (res.ok) {
        const json: ApiResponse<string | null> = await res.json();
        if (json.success) {
          setEditorDomain(json.data ?? "");
          setEditorDomainSaved(json.data ?? null);
        }
      }
    } catch {
      // silently ignore
    }
  });

  const copyLink = async () => {
    const link = magicLink();
    if (!link) return;
    await navigator.clipboard.writeText(link);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleSaveDomain = async () => {
    setDomainLoading(true);
    setDomainError(null);
    setDomainSaveMsg(null);

    const domain = editorDomain().trim().toLowerCase() || null;

    try {
      const res = await fetch("/api/admin/settings/editor-domain", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ domain }),
      });

      const json: ApiResponse<string | null> = await res.json();

      if (res.ok && json.success) {
        setEditorDomainSaved(json.data ?? null);
        setEditorDomain(json.data ?? "");
        setDomainSaveMsg(domain ? `Editor domain set to @${domain}` : "Editor domain cleared.");
        setTimeout(() => setDomainSaveMsg(null), 3000);
      } else {
        setDomainError(json.error ?? "Failed to save.");
      }
    } catch (e) {
      setDomainError("Failed to save editor domain.");
    } finally {
      setDomainLoading(false);
    }
  };

  return (
    <section class="section">
      <div class="container">
        <h1 class="title">⚙️ Admin</h1>
        <p class="subtitle">Manage site settings.</p>

        <Show when={isAdmin()}>
          {/* ── Magic Link ───────────────────────────────────────── */}
          <div class="box">
            <h2 class="title is-5">🔗 Magic Link</h2>
            <p class="mb-3">
              Share this link to give direct site access without requiring a password.
            </p>

            {magicLoading() && <progress class="progress is-small is-primary" max="100" />}

            {magicError() && (
              <div class="notification is-danger is-light">{magicError()}</div>
            )}

            {magicLink() && (
              <div class="field has-addons">
                <div class="control is-expanded">
                  <input
                    class="input is-family-monospace"
                    type="text"
                    readOnly
                    value={magicLink()!}
                    onClick={(e) => e.currentTarget.select()}
                  />
                </div>
                <div class="control">
                  <button class="button is-primary" onClick={copyLink}>
                    {copied() ? "Copied!" : "Copy"}
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* ── Editor Domain ────────────────────────────────────── */}
          <div class="box">
            <h2 class="title is-5">✏️ Editor Email Domain</h2>
            <p class="mb-3">
              Registered users with this email domain can self-promote to Editor.
              Leave empty to disable self-service editor access.
            </p>

            <Show when={domainSaveMsg()}>
              <div class="notification is-success is-light">
                {domainSaveMsg()}
              </div>
            </Show>

            <Show when={domainError()}>
              <div class="notification is-danger is-light">
                {domainError()}
              </div>
            </Show>

            <div class="field has-addons">
              <div class="control">
                <a class="button is-static">@</a>
              </div>
              <div class="control is-expanded">
                <input
                  class="input"
                  type="text"
                  placeholder="example.com"
                  value={editorDomain()}
                  onInput={(e) => setEditorDomain(e.currentTarget.value)}
                  disabled={domainLoading()}
                />
              </div>
              <div class="control">
                <button
                  class="button is-primary"
                  classList={{ "is-loading": domainLoading() }}
                  onClick={handleSaveDomain}
                  disabled={domainLoading()}
                >
                  Save
                </button>
              </div>
            </div>

            <Show when={editorDomainSaved()}>
              <p class="help">
                Currently set to: <strong>@{editorDomainSaved()}</strong>
              </p>
            </Show>
          </div>
        </Show>
      </div>
    </section>
  );
}
