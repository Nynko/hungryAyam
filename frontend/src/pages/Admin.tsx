import { createSignal, onMount } from "solid-js";

export default function Admin() {
  const [magicLink, setMagicLink] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [copied, setCopied] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/admin/magic-link");
      if (!res.ok) throw new Error(`${res.status}`);
      const json = await res.json();
      if (json.success && json.data) {
        const url = `${window.location.origin}/?access=${json.data}`;
        setMagicLink(url);
      }
    } catch (e) {
      setError("Failed to load magic link.");
    } finally {
      setLoading(false);
    }
  });

  const copyLink = async () => {
    const link = magicLink();
    if (!link) return;
    await navigator.clipboard.writeText(link);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <section class="section">
      <div class="container">
        <h1 class="title">⚙️ Admin</h1>
        <p class="subtitle">Manage restaurants, menus, and users.</p>

        <div class="box">
          <h2 class="title is-5">🔗 Magic Link</h2>
          <p class="mb-3">
            Share this link to give direct site access without requiring a password.
          </p>

          {loading() && <progress class="progress is-small is-primary" max="100" />}

          {error() && (
            <div class="notification is-danger is-light">{error()}</div>
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
      </div>
    </section>
  );
}
