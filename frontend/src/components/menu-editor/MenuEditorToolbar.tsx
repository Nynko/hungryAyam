import { Show, createSignal } from "solid-js";
import {
  editorState,
  actionQueue,
  editorSaving,
  editorError,
  dirty,
  updateMenuName,
  updateMenuDescription,
  updateMenuIsActive,
  updateMenuPermanent,
  saveMenu,
  discardChanges,
  resetMenu,
} from "@/stores/menuEditorStore";
import { showConfirm } from "@/stores/confirmStore";

interface MenuEditorToolbarProps {
  onSaved?: () => void;
  onCancel: () => void;
  /** When true, hide menu metadata fields (name, description, toggles) and
   *  the reset button. Only show save/discard/cancel actions. */
  availabilityOnly?: boolean;
}

export default function MenuEditorToolbar(props: MenuEditorToolbarProps) {
  const [resetResult, setResetResult] = createSignal<string | null>(null);

  const handleReset = async () => {
    const confirmed = await showConfirm({
      title: "Reset menu?",
      message:
        "This will set ALL items in this menu to unavailable. " +
        "Items are kept in the menu so you can easily re-select which ones to offer today. " +
        "Any unsaved changes will be saved first.",
      confirmText: "Reset Menu",
      cancelText: "Cancel",
      danger: true,
    });
    if (!confirmed) return;

    // Save pending changes first if dirty
    if (dirty()) {
      const saved = await saveMenu();
      if (!saved) return; // save failed, don't reset
    }

    const count = await resetMenu();
    if (count !== null) {
      setResetResult(`✅ Reset complete — ${count} item${count !== 1 ? "s" : ""} set to unavailable.`);
      setTimeout(() => setResetResult(null), 5000);
    }
  };

  const handleSave = async () => {
    const result = await saveMenu();
    if (result && props.onSaved) {
      props.onSaved();
    }
  };

  const handleDiscard = async () => {
    if (!dirty()) {
      discardChanges();
      return;
    }
    const confirmed = await showConfirm({
      title: "Discard changes?",
      message: "All unsaved changes will be lost. This cannot be undone.",
      confirmText: "Discard",
      cancelText: "Stay",
      danger: true,
    });
    if (confirmed) {
      discardChanges();
    }
  };

  const handleCancel = async () => {
    if (!dirty()) {
      props.onCancel();
      return;
    }
    const confirmed = await showConfirm({
      title: "Leave editor?",
      message: "You have unsaved changes. Are you sure you want to leave?",
      confirmText: "Leave",
      cancelText: "Stay",
      danger: true,
    });
    if (confirmed) {
      props.onCancel();
    }
  };

  return (
    <div class="box mb-5">
      {/* ── Error banner ──────────────────────────────────── */}
      <Show when={editorError()}>
        <div class="notification is-danger is-light mb-4">
          <button class="delete" onClick={() => {}} />
          <strong>Error:</strong> {editorError()}
        </div>
      </Show>

      {/* ── Reset result banner ───────────────────────────── */}
      <Show when={resetResult()}>
        <div class="notification is-success is-light mb-4">
          <button class="delete" onClick={() => setResetResult(null)} />
          {resetResult()}
        </div>
      </Show>

      {/* ── Menu metadata fields (hidden in availabilityOnly mode) ── */}
      <Show when={!props.availabilityOnly}>
        <div class="columns is-multiline">
          {/* Name */}
          <div class="column is-6">
            <div class="field">
              <label class="label">Menu name</label>
              <div class="control">
                <input
                  class="input"
                  type="text"
                  placeholder="e.g. Lunch Menu"
                  value={editorState.draft.name}
                  onInput={(e) => updateMenuName(e.currentTarget.value)}
                />
              </div>
            </div>
          </div>

          {/* Description */}
          <div class="column is-6">
            <div class="field">
              <label class="label">Description</label>
              <div class="control">
                <input
                  class="input"
                  type="text"
                  placeholder="Optional description"
                  value={editorState.draft.description ?? ""}
                  onInput={(e) =>
                    updateMenuDescription(e.currentTarget.value || null)
                  }
                />
              </div>
            </div>
          </div>

          {/* Toggles */}
          <div class="column is-6">
            <div class="field is-grouped">
              <div class="control">
                <label class="checkbox">
                  <input
                    type="checkbox"
                    checked={editorState.draft.is_active}
                    onChange={(e) => updateMenuIsActive(e.currentTarget.checked)}
                  />{" "}
                  Active
                </label>
                <p class="help has-text-grey">
                  Active menus are visible to users
                </p>
              </div>

              <div class="control ml-5">
                <label class="checkbox">
                  <input
                    type="checkbox"
                    checked={editorState.draft.permanent}
                    onChange={(e) => updateMenuPermanent(e.currentTarget.checked)}
                  />{" "}
                  Permanent
                </label>
                <p class="help has-text-grey">
                  Permanent menus keep items between resets
                </p>
              </div>
            </div>
          </div>

          {/* Action count (edit mode only) */}
          <Show when={!editorState.isNewMenu && actionQueue().length > 0}>
            <div class="column is-6">
              <div class="field">
                <label class="label">Pending changes</label>
                <p class="is-size-7 has-text-grey">
                  <span class="tag is-info is-light mr-1">
                    {actionQueue().length}
                  </span>
                  action{actionQueue().length !== 1 ? "s" : ""} queued
                </p>
              </div>
            </div>
          </Show>
        </div>
      </Show>

      {/* ── Availability-only mode header ──────────────────── */}
      <Show when={props.availabilityOnly}>
        <div class="mb-3">
          <p class="has-text-weight-semibold is-size-5 mb-1">
            {editorState.draft.name || "Menu"}
          </p>
          <p class="has-text-grey is-size-7">
            Choose which items are available today. You can also add new items.
          </p>
        </div>
      </Show>

      {/* ── Action buttons ────────────────────────────────── */}
      <hr class="my-3" />
      <div class="is-flex is-justify-content-space-between is-align-items-center">
        <div class="buttons">
          <button
            class="button is-primary"
            classList={{ "is-loading": editorSaving() }}
            disabled={
              editorSaving() ||
              !editorState.draft.name.trim() ||
              (!editorState.isNewMenu && !dirty())
            }
            onClick={handleSave}
          >
            <span class="icon is-small">
              <span>💾</span>
            </span>
            <span>
              {editorState.isNewMenu ? "Create Menu" : "Save Changes"}
            </span>
          </button>

          <Show when={dirty()}>
            <button
              class="button is-warning is-outlined"
              disabled={editorSaving()}
              onClick={handleDiscard}
            >
              <span class="icon is-small">
                <span>↩️</span>
              </span>
              <span>Discard</span>
            </button>
          </Show>
        </div>

        <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
          {/* Reset button — only for non-permanent menus in edit mode, hidden in availabilityOnly */}
          <Show when={!props.availabilityOnly && !editorState.isNewMenu && !editorState.draft.permanent}>
            <button
              class="button is-danger is-outlined"
              classList={{ "is-loading": editorSaving() }}
              disabled={editorSaving()}
              onClick={handleReset}
              title="Reset all items to unavailable"
            >
              <span class="icon is-small">
                <span>🔄</span>
              </span>
              <span>Reset Menu</span>
            </button>
          </Show>

          <button
            class="button is-light"
            disabled={editorSaving()}
            onClick={handleCancel}
          >
            <span class="mr-1">←</span>
            Cancel
          </button>
        </div>
      </div>

      {/* ── Dirty indicator ───────────────────────────────── */}
      <Show when={dirty()}>
        <p class="has-text-warning is-size-7 mt-2">
          ⚠ You have unsaved changes
        </p>
      </Show>
    </div>
  );
}