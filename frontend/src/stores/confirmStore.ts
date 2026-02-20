import { createSignal } from "solid-js";

export interface ConfirmOptions {
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
}

interface ConfirmState extends ConfirmOptions {
  resolve: (value: boolean) => void;
}

const [confirmState, setConfirmState] = createSignal<ConfirmState | null>(null);

/**
 * Show a confirm dialog and wait for the user's response.
 *
 * Usage:
 * ```ts
 * const confirmed = await showConfirm({
 *   title: "Delete item?",
 *   message: "This cannot be undone.",
 *   confirmText: "Delete",
 *   danger: true,
 * });
 * if (confirmed) { ... }
 * ```
 */
export function showConfirm(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    setConfirmState({ ...options, resolve });
  });
}

/** Read the current dialog state (used by the ConfirmDialog component in the layout). */
export function confirmDialog() {
  return confirmState();
}

/** Resolve the pending dialog and close it. */
export function resolveConfirm(value: boolean) {
  const state = confirmState();
  if (state) {
    state.resolve(value);
    setConfirmState(null);
  }
}