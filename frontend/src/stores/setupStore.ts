import { createSignal } from "solid-js";
import { AppSetupRequest } from "@bindings/AppSetupRequest"

// ── State ─────────────────────────────────────────────────────────
const [setupCompleted, setSetupCompleted] = createSignal<boolean | null>(null);
const [setupLoading, setSetupLoading] = createSignal(false);
const [setupError, setSetupError] = createSignal<string | null>(null);

/**
 * Check whether the app has been set up yet.
 *
 * Calls `GET /setup` and caches the result in `setupCompleted`.
 * Returns `true` if setup is done, `false` if not, `null` while loading.
 */
async function checkSetupStatus(): Promise<boolean> {
  try {
    setSetupLoading(true);
    setSetupError(null);

    const res = await fetch("/setup");
    if (!res.ok) {
      throw new Error(`GET /setup responded with ${res.status}`);
    }

    const json = await res.json();
    if (json.success && json.data != null) {
      const completed = json.data.completed as boolean;
      setSetupCompleted(completed);
      return completed;
    } else {
      throw new Error(json.error ?? "Unexpected response from /setup");
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setSetupError(msg);
    console.error("[setupStore] Failed to check setup status:", msg);
    // Assume not completed on error so the user sees the setup page
    setSetupCompleted(false);
    return false;
  } finally {
    setSetupLoading(false);
  }
}

/**
 * Submit the initial setup form.
 *
 * Sends `POST /setup` with the access code and admin user details.
 * On success, marks setup as completed.
 */
async function submitSetup(request: AppSetupRequest): Promise<boolean> {
  try {
    setSetupLoading(true);
    setSetupError(null);

    const res = await fetch("/setup", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json = await res.json();

    if (res.ok && json.success) {
      setSetupCompleted(true);
      return true;
    } else {
      const msg = json.error ?? `Setup failed with status ${res.status}`;
      setSetupError(msg);
      return false;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setSetupError(msg);
    console.error("[setupStore] Setup submission failed:", msg);
    return false;
  } finally {
    setSetupLoading(false);
  }
}

/**
 * Clear the setup error (e.g. when dismissing a notification).
 */
function clearSetupError(): void {
  setSetupError(null);
}

export {
  setupCompleted,
  setupLoading,
  setupError,
  checkSetupStatus,
  submitSetup,
  clearSetupError,
};
