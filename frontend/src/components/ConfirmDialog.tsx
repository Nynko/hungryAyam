import { confirmDialog, resolveConfirm } from "@/stores/confirmStore";

/**
 * Global confirm dialog component.
 * Mount once in the layout — it reads state from confirmStore.
 * Trigger from anywhere with `await showConfirm({ ... })`.
 */
export default function ConfirmDialog() {
  const state = () => confirmDialog();
  const isOpen = () => state() !== null;

  const confirmText = () => state()?.confirmText ?? "Confirm";
  const cancelText = () => state()?.cancelText ?? "Cancel";
  const isDanger = () => state()?.danger ?? false;

  return (
    <div class="modal" classList={{ "is-active": isOpen() }}>
      <div class="modal-background" onClick={() => resolveConfirm(false)} />
      <div class="modal-card" style={{ "max-width": "440px" }}>
        <header class="modal-card-head">
          <p class="modal-card-title">{state()?.title}</p>
          <button
            class="delete"
            aria-label="close"
            onClick={() => resolveConfirm(false)}
          />
        </header>

        <section class="modal-card-body">
          <p>{state()?.message}</p>
        </section>

        <footer class="modal-card-foot">
          <div class="buttons">
            <button
              class={`button ${isDanger() ? "is-danger" : "is-primary"}`}
              onClick={() => resolveConfirm(true)}
            >
              {confirmText()}
            </button>
            <button class="button" onClick={() => resolveConfirm(false)}>
              {cancelText()}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}