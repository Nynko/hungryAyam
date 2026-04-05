import { createSignal, createMemo } from "solid-js";
import type { User } from "@bindings/User";
import type { ApiResponse } from "@bindings/ApiResponse";
import type { AuthResponseDto } from "@bindings/AuthResponseDto";
import type { SiteAccessResponse } from "@bindings/SiteAccessResponse";

// ── State ─────────────────────────────────────────────────────────
const [currentUser, setCurrentUser] = createSignal<User | null>(null);
const [hasSiteAccess, setHasSiteAccess] = createSignal(false);
const [authLoading, setAuthLoading] = createSignal(false);
const [authError, setAuthError] = createSignal<string | null>(null);
const [authChecked, setAuthChecked] = createSignal(false);

// ── Derived state ─────────────────────────────────────────────────
const isAuthenticated = createMemo(() => currentUser() !== null);
const isGuest = createMemo(
  () => currentUser() !== null && currentUser()!.auth_method === "NameWithCookie"
);
const isPasswordUser = createMemo(
  () => currentUser() !== null && currentUser()!.auth_method === "Password"
);
/** User has Editor or Admin role — can manage restaurants, menus, offers, sessions. */
const isEditor = createMemo(() => {
  const role = currentUser()?.role;
  return role === "Editor" || role === "Admin";
});
/** User has Admin role — full control including user management and app settings. */
const isAdmin = createMemo(() => currentUser()?.role === "Admin");

// ── Check current auth (on app load) ──────────────────────────────

/**
 * Call `GET /api/auth/me` to check if we have a valid session.
 *
 * On success, caches the user and sets `hasSiteAccess` to true
 * (authenticated users have implicit site access).
 *
 * On failure (401), sets user to null. This is expected for
 * unauthenticated visitors and is not treated as an error.
 */
async function checkAuth(): Promise<User | null> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    const res = await fetch("/api/auth/me");

    if (res.status === 401) {
      setCurrentUser(null);
      return null;
    }

    if (!res.ok) {
      throw new Error(`GET /api/auth/me responded with ${res.status}`);
    }

    const json: ApiResponse<User> = await res.json();

    if (json.success && json.data != null) {
      setCurrentUser(json.data);
      setHasSiteAccess(true);
      return json.data;
    } else {
      setCurrentUser(null);
      return null;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    console.error("[authStore] checkAuth failed:", msg);
    setCurrentUser(null);
    return null;
  } finally {
    setAuthLoading(false);
    setAuthChecked(true);
  }
}

// ── Site access ───────────────────────────────────────────────────

/**
 * Verify the shared site password via `POST /api/auth/site-access`.
 *
 * On success, the server sets the `site_access` HttpOnly cookie
 * and we track it locally in `hasSiteAccess`.
 */
async function verifySiteAccess(code: string): Promise<boolean> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    const res = await fetch("/api/auth/site-access", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code }),
    });

    const json: ApiResponse<SiteAccessResponse> = await res.json();

    if (res.ok && json.success && json.data?.granted) {
      setHasSiteAccess(true);
      return true;
    } else {
      const msg = json.error ?? "Invalid site access code.";
      setAuthError(msg);
      return false;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setAuthError(msg);
    console.error("[authStore] verifySiteAccess failed:", msg);
    return false;
  } finally {
    setAuthLoading(false);
  }
}

// ── Check site access cookie ──────────────────────────────────────

/**
 * Check whether the `site_access` cookie is already valid by calling
 * `GET /api/auth/site-access`. Sets `hasSiteAccess` if the server
 * confirms the cookie. Should only be called when `hasSiteAccess` is false.
 */
async function checkSiteAccess(): Promise<void> {
  try {
    const res = await fetch("/api/auth/site-access");
    if (res.ok) {
      setHasSiteAccess(true);
    } else {
      // Real cookie is gone — clear the stale hint so we don't re-check on next load
      document.cookie = "site_access_hint=; Max-Age=0; Path=/; SameSite=Lax";
    }
  } catch (e) {
    // Silently ignore — network error
  }
}

// ── Magic link ───────────────────────────────────────────────────

/**
 * Verify a magic-link token via `GET /api/auth/site-access/:token`.
 *
 * The token is the SHA-256 hash of the site access code, embedded
 * directly in the shared URL. On success the server sets the
 * `site_access` HttpOnly cookie and we track it locally.
 */
async function verifyMagicLink(token: string): Promise<boolean> {
  try {
    const res = await fetch(`/api/auth/site-access/${encodeURIComponent(token)}`);
    if (res.ok) {
      setHasSiteAccess(true);
      return true;
    }
    return false;
  } catch (e) {
    console.error("[authStore] verifyMagicLink failed:", e);
    return false;
  }
}

// ── Guest login ───────────────────────────────────────────────────

export interface GuestLoginResult {
  user: User;
  existingUser: boolean;
}

/**
 * Create a guest user via `POST /api/auth/guest`, or reconnect as
 * an existing guest with the same name.
 *
 * The server sets the `session_token` cookie. On success, the
 * returned user is cached as the current user.
 *
 * Returns `{ user, existingUser }` where `existingUser` is true if
 * reconnecting to an existing account.
 */
async function loginAsGuest(name: string): Promise<GuestLoginResult | null> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    const res = await fetch("/api/auth/guest", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name }),
    });

    const json: ApiResponse<AuthResponseDto> = await res.json();

    if (res.ok && json.success && json.data != null) {
      setCurrentUser(json.data.user);
      setHasSiteAccess(true);
      return {
        user: json.data.user,
        existingUser: json.data.existing_user ?? false,
      };
    } else {
      const msg = json.error ?? `Guest login failed with status ${res.status}`;
      setAuthError(msg);
      return null;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setAuthError(msg);
    console.error("[authStore] loginAsGuest failed:", msg);
    return null;
  } finally {
    setAuthLoading(false);
  }
}

// ── Password login ────────────────────────────────────────────────

/**
 * Log in with email + password via `POST /api/auth/login`.
 *
 * The server sets the `session_token` cookie. On success, the
 * returned user is cached as the current user.
 */
async function login(email: string, password: string): Promise<User | null> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    const res = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });

    const json: ApiResponse<AuthResponseDto> = await res.json();

    if (res.ok && json.success && json.data != null) {
      setCurrentUser(json.data.user);
      setHasSiteAccess(true);
      return json.data.user;
    } else {
      const msg = json.error ?? "Invalid email or password.";
      setAuthError(msg);
      return null;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setAuthError(msg);
    console.error("[authStore] login failed:", msg);
    return null;
  } finally {
    setAuthLoading(false);
  }
}

// ── Logout ────────────────────────────────────────────────────────

/**
 * Log out via `POST /api/auth/logout`.
 *
 * Clears both session and site-access cookies on the server,
 * and resets local state.
 */
async function logout(): Promise<void> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    await fetch("/api/auth/logout", { method: "POST" });
  } catch (e) {
    console.error("[authStore] logout failed:", e);
  } finally {
    setCurrentUser(null);
    setHasSiteAccess(false);
    setAuthLoading(false);
  }
}

// ── Self-service: register (guest → password) ────────────────────

/**
 * Upgrade the current guest account to a password account via
 * `POST /api/auth/register`.
 *
 * On success, existing sessions are invalidated — the caller should
 * redirect to the login page.
 */
async function selfRegister(
  email: string,
  password: string,
  name?: string,
): Promise<boolean> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    const body: Record<string, string> = { email, password };
    if (name) body.name = name;

    const res = await fetch("/api/auth/register", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });

    const json: ApiResponse<null> = await res.json();

    if (res.ok && json.success) {
      // Sessions were invalidated — clear local state
      setCurrentUser(null);
      return true;
    } else {
      setAuthError(json.error ?? "Registration failed.");
      return false;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setAuthError(msg);
    return false;
  } finally {
    setAuthLoading(false);
  }
}

// ── Self-service: toggle editor ──────────────────────────────────

/**
 * Toggle between User and Editor role via `POST /api/auth/toggle-editor`.
 *
 * Updates the local user on success.
 */
async function toggleEditor(): Promise<boolean> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    const res = await fetch("/api/auth/toggle-editor", { method: "POST" });
    const json: ApiResponse<User> = await res.json();

    if (res.ok && json.success && json.data) {
      setCurrentUser(json.data);
      return true;
    } else {
      setAuthError(json.error ?? "Failed to toggle editor role.");
      return false;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setAuthError(msg);
    return false;
  } finally {
    setAuthLoading(false);
  }
}

// ── Self-service: change name ────────────────────────────────────

/**
 * Change the current user's display name via `PUT /api/auth/profile/name`.
 *
 * Updates the local user on success.
 */
async function changeName(name: string): Promise<boolean> {
  try {
    setAuthLoading(true);
    setAuthError(null);

    const res = await fetch("/api/auth/profile/name", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name }),
    });

    const json: ApiResponse<User> = await res.json();

    if (res.ok && json.success && json.data) {
      setCurrentUser(json.data);
      return true;
    } else {
      setAuthError(json.error ?? "Failed to change name.");
      return false;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setAuthError(msg);
    return false;
  } finally {
    setAuthLoading(false);
  }
}

// ── Helpers ───────────────────────────────────────────────────────

/**
 * Clear only the auth error signal (e.g. when dismissing a notification).
 */
function clearAuthError(): void {
  setAuthError(null);
}

export {
  // State
  currentUser,
  hasSiteAccess,
  authLoading,
  authError,
  authChecked,

  // Derived
  isAuthenticated,
  isGuest,
  isPasswordUser,
  isEditor,
  isAdmin,

  // Actions
  checkAuth,
  checkSiteAccess,
  verifySiteAccess,
  verifyMagicLink,
  loginAsGuest,
  login,
  logout,
  selfRegister,
  toggleEditor,
  changeName,
  clearAuthError,
};