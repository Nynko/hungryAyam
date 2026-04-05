import { Show, createSignal, onMount } from "solid-js";
import {
  isAuthenticated,
  isGuest,
  isPasswordUser,
  isEditor,
  isAdmin,
  currentUser,
  logout,
  changeName,
  toggleEditor,
  authLoading,
  authError,
  clearAuthError,
} from "@/stores/authStore";
import type { EditorEligibilityResponse } from "@bindings/EditorEligibilityResponse";
import type { ApiResponse } from "@bindings/ApiResponse";
import AuthPanel from "@/components/AuthPanel";
import { useNavigate } from "@solidjs/router";

export default function Login() {
  const navigate = useNavigate();

  const handleAuthenticated = () => {
    navigate("/restaurants");
  };

  const handleLogout = async () => {
    await logout();
  };

  // ── Profile: name change ────────────────────────────────────────
  const [editingName, setEditingName] = createSignal(false);
  const [newName, setNewName] = createSignal("");

  const startEditName = () => {
    setNewName(currentUser()?.name ?? "");
    setEditingName(true);
    clearAuthError();
  };

  const handleChangeName = async (e: SubmitEvent) => {
    e.preventDefault();
    const name = newName().trim();
    if (!name) return;
    const ok = await changeName(name);
    if (ok) setEditingName(false);
  };

  // Suggest name from email
  const suggestedName = () => {
    const email = currentUser()?.email;
    if (!email) return null;
    const at = email.indexOf("@");
    return at > 0 ? email.substring(0, at) : null;
  };

  // ── Editor eligibility ──────────────────────────────────────────
  const [editorEligible, setEditorEligible] = createSignal(false);

  onMount(async () => {
    if (!isPasswordUser() || isAdmin()) return;
    try {
      const res = await fetch("/api/auth/editor-eligibility");
      if (!res.ok) return;
      const json: ApiResponse<EditorEligibilityResponse> = await res.json();
      if (json.success && json.data) {
        setEditorEligible(json.data.eligible);
      }
    } catch {
      // silently ignore
    }
  });

  const handleToggleEditor = async () => {
    await toggleEditor();
  };

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-centered">
          <div class="column is-6-desktop is-8-tablet">
            <h1 class="title">🔐 Login</h1>
            <p class="subtitle">Access your account or enter with a shared password.</p>

            <Show
              when={!isPasswordUser()}
              fallback={
                <div class="box">
                  <div class="has-text-centered mb-4">
                    <p class="is-size-4 mb-3">👋</p>
                    <p class="title is-5">
                      Welcome back, {currentUser()?.name}!
                    </p>
                    <p class="has-text-grey mb-4">
                      You are logged in
                      <Show when={isEditor() && !isAdmin()}>
                        {" "}as <strong>Editor</strong>
                      </Show>
                      <Show when={isAdmin()}>
                        {" "}as <strong>Admin</strong>
                      </Show>
                      .
                    </p>
                  </div>

                  {/* ── Error notification ──────────────────────── */}
                  <Show when={authError()}>
                    <div class="notification is-danger is-light">
                      <button class="delete" type="button" onClick={clearAuthError} />
                      {authError()}
                    </div>
                  </Show>

                  {/* ── Name change ─────────────────────────────── */}
                  <Show
                    when={editingName()}
                    fallback={
                      <div class="field">
                        <label class="label">Display Name</label>
                        <div class="level">
                          <div class="level-left">
                            <span>{currentUser()?.name}</span>
                          </div>
                          <div class="level-right">
                            <button class="button is-small" onClick={startEditName}>
                              Edit
                            </button>
                          </div>
                        </div>
                      </div>
                    }
                  >
                    <form onSubmit={handleChangeName}>
                      <div class="field">
                        <label class="label">New Display Name</label>
                        <div class="control">
                          <input
                            class="input"
                            type="text"
                            value={newName()}
                            onInput={(e) => setNewName(e.currentTarget.value)}
                            disabled={authLoading()}
                            autofocus
                          />
                        </div>
                        <Show when={suggestedName() && suggestedName() !== newName()}>
                          <p class="help">
                            Suggestion:{" "}
                            <a onClick={() => setNewName(suggestedName()!)}>
                              {suggestedName()}
                            </a>
                          </p>
                        </Show>
                      </div>
                      <div class="field is-grouped">
                        <div class="control">
                          <button
                            class="button is-primary"
                            type="submit"
                            classList={{ "is-loading": authLoading() }}
                            disabled={authLoading() || !newName().trim()}
                          >
                            Save
                          </button>
                        </div>
                        <div class="control">
                          <button
                            class="button"
                            type="button"
                            onClick={() => setEditingName(false)}
                          >
                            Cancel
                          </button>
                        </div>
                      </div>
                    </form>
                  </Show>

                  <hr />

                  {/* ── Editor toggle ───────────────────────────── */}
                  <Show when={editorEligible() && !isAdmin()}>
                    <div class="field">
                      <label class="label">Editor Mode</label>
                      <div class="level">
                        <div class="level-left">
                          <p class="has-text-grey">
                            <Show when={isEditor()} fallback="You are currently a regular user.">
                              You have editor privileges.
                            </Show>
                          </p>
                        </div>
                        <div class="level-right">
                          <button
                            class="button is-small"
                            classList={{
                              "is-warning": !isEditor(),
                              "is-light": isEditor(),
                              "is-loading": authLoading(),
                            }}
                            onClick={handleToggleEditor}
                            disabled={authLoading()}
                          >
                            {isEditor() ? "Step down" : "Become Editor"}
                          </button>
                        </div>
                      </div>
                    </div>
                    <hr />
                  </Show>

                  {/* ── Actions ─────────────────────────────────── */}
                  <div class="buttons is-centered">
                    <button class="button is-primary" onClick={() => navigate("/restaurants")}>
                      Browse Restaurants
                    </button>
                    <button class="button is-danger is-outlined" onClick={handleLogout}>
                      Log out
                    </button>
                  </div>
                </div>
              }
            >
              <Show when={isGuest()}>
                <div class="notification is-info is-light mb-5">
                  <p>
                    👤 You're currently browsing as <strong>{currentUser()?.name}</strong> (guest).
                    Create an account to unlock more features, or continue browsing.
                  </p>
                </div>
              </Show>

              <AuthPanel
                onAuthenticated={handleAuthenticated}
              />
            </Show>
          </div>
        </div>
      </div>
    </section>
  );
}
