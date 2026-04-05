import { createSignal, Show, Switch, Match } from "solid-js";
import {
  authLoading,
  authError,
  clearAuthError,
  isAuthenticated,
  isGuest,
  hasSiteAccess,
  verifySiteAccess,
  loginAsGuest,
  login,
  selfRegister,
} from "@/stores/authStore";

type AuthTab = "guest" | "login" | "register";

interface AuthPanelProps {
  /** Called after any successful authentication action. */
  onAuthenticated?: () => void;
  /** Called after a successful registration (guest → password). */
  onRegistered?: () => void;
}

export default function AuthPanel(props: AuthPanelProps) {
  const defaultTab = (): AuthTab => (isGuest() ? "register" : "guest");

  const [activeTab, setActiveTab] = createSignal<AuthTab>(defaultTab());
  const [successMessage, setSuccessMessage] = createSignal<string | null>(null);

  // ── Guest form state ──────────────────────────────────────────
  const [guestName, setGuestName] = createSignal("");
  const [guestAccessCode, setGuestAccessCode] = createSignal("");

  const handleGuest = async (e: SubmitEvent) => {
    e.preventDefault();
    const name = guestName().trim();
    if (!name) return;

    // If user doesn't have site access yet, verify the code first
    if (!hasSiteAccess()) {
      const code = guestAccessCode().trim();
      if (!code) return;

      const ok = await verifySiteAccess(code);
      if (!ok) return; // Error is set by verifySiteAccess
    }

    // Now create the guest user (or reconnect as existing)
    const result = await loginAsGuest(name);
    if (result) {
      setGuestName("");
      setGuestAccessCode("");
      if (result.existingUser) {
        setSuccessMessage(`Welcome back, ${result.user.name}! You've been reconnected to your existing account.`);
      }
      props.onAuthenticated?.();
    }
  };

  // ── Login form state ──────────────────────────────────────────
  const [email, setEmail] = createSignal("");
  const [password, setPassword] = createSignal("");

  const handleLogin = async (e: SubmitEvent) => {
    e.preventDefault();
    const e_ = email().trim();
    const p = password();
    if (!e_ || !p) return;

    const user = await login(e_, p);
    if (user) {
      setEmail("");
      setPassword("");
      props.onAuthenticated?.();
    }
  };

  // ── Register form state ───────────────────────────────────────
  const [regEmail, setRegEmail] = createSignal("");
  const [regPassword, setRegPassword] = createSignal("");
  const [regName, setRegName] = createSignal("");

  /** Suggest a name from the email local part (e.g. john.doe@example.com → john.doe) */
  const suggestNameFromEmail = () => {
    const emailVal = regEmail().trim();
    const at = emailVal.indexOf("@");
    if (at > 0) {
      setRegName(emailVal.substring(0, at));
    }
  };

  const handleRegister = async (e: SubmitEvent) => {
    e.preventDefault();
    const em = regEmail().trim();
    const pw = regPassword();
    const nm = regName().trim() || undefined;
    if (!em || !pw) return;

    const ok = await selfRegister(em, pw, nm);
    if (ok) {
      setRegEmail("");
      setRegPassword("");
      setRegName("");
      setSuccessMessage("Account created! Please log in with your email and password.");
      setActiveTab("login");
      props.onRegistered?.();
    }
  };

  const canSubmitGuest = () => {
    const hasName = guestName().trim().length > 0;
    if (hasSiteAccess()) {
      return hasName;
    }
    return hasName && guestAccessCode().trim().length > 0;
  };

  return (
    <div>
      {/* ── Tabs ─────────────────────────────────────────────── */}
      <div class="tabs is-boxed mb-0">
        <ul>
          <Show when={!isGuest()}>
            <li classList={{ "is-active": activeTab() === "guest" }}>
              <a onClick={() => { setActiveTab("guest"); clearAuthError(); }}>
                <span class="icon is-small"><span>👤</span></span>
                <span>Continue as Guest</span>
              </a>
            </li>
          </Show>
          <li classList={{ "is-active": activeTab() === "login" }}>
            <a onClick={() => { setActiveTab("login"); clearAuthError(); }}>
              <span class="icon is-small"><span>🔐</span></span>
              <span>Log in</span>
            </a>
          </li>
          <Show when={isGuest()}>
            <li classList={{ "is-active": activeTab() === "register" }}>
              <a onClick={() => { setActiveTab("register"); clearAuthError(); }}>
                <span class="icon is-small"><span>📝</span></span>
                <span>Create Account</span>
              </a>
            </li>
          </Show>
        </ul>
      </div>

      {/* ── Success notification ──────────────────────────────── */}
      <Show when={successMessage()}>
        <div class="notification is-success is-light mt-3">
          <button class="delete" type="button" onClick={() => setSuccessMessage(null)} />
          {successMessage()}
        </div>
      </Show>

      {/* ── Error notification ────────────────────────────────── */}
      <Show when={authError()}>
        <div class="notification is-danger is-light mt-3">
          <button class="delete" type="button" onClick={clearAuthError} />
          {authError()}
        </div>
      </Show>

      {/* ── Tab content ──────────────────────────────────────── */}
      <div class="box" style={{ "border-top-left-radius": "0", "border-top-right-radius": "0", "margin-top": "0" }}>
        <Switch>
          {/* ── Guest ─────────────────────────────────────────── */}
          <Match when={activeTab() === "guest"}>
            <p class="mb-4 has-text-grey">
              Pick a display name and start ordering — no account needed.
            </p>
            <form onSubmit={handleGuest}>
              {/* Guest access code — only shown if not already verified */}
              <Show when={!hasSiteAccess()}>
                <div class="field">
                  <label class="label">Guest Access Code</label>
                  <div class="control">
                    <input
                      class="input"
                      type="password"
                      placeholder="Enter the shared access code..."
                      value={guestAccessCode()}
                      onInput={(e) => setGuestAccessCode(e.currentTarget.value)}
                      disabled={authLoading()}
                      autofocus
                    />
                  </div>
                  <p class="help">
                    Ask the organizer for the access code.
                  </p>
                </div>
              </Show>

              {/* Name field */}
              <div class="field">
                <label class="label">Your Name</label>
                <div class="control">
                  <input
                    class="input"
                    type="text"
                    placeholder="e.g. Alex"
                    value={guestName()}
                    onInput={(e) => setGuestName(e.currentTarget.value)}
                    disabled={authLoading()}
                    autofocus={hasSiteAccess()}
                  />
                </div>
              </div>

              <div class="field">
                <div class="control">
                  <button
                    class="button is-primary is-fullwidth"
                    type="submit"
                    classList={{ "is-loading": authLoading() }}
                    disabled={authLoading() || !canSubmitGuest()}
                  >
                    Continue as Guest
                  </button>
                </div>
              </div>
            </form>
          </Match>

          {/* ── Login ─────────────────────────────────────────── */}
          <Match when={activeTab() === "login"}>
            <p class="mb-4 has-text-grey">
              Log in with your email and password.
            </p>
            <form onSubmit={handleLogin}>
              <div class="field">
                <label class="label">Email</label>
                <div class="control has-icons-left">
                  <input
                    class="input"
                    type="email"
                    placeholder="you@example.com"
                    value={email()}
                    onInput={(e) => setEmail(e.currentTarget.value)}
                    disabled={authLoading()}
                    autofocus
                  />
                  <span class="icon is-left">📧</span>
                </div>
              </div>
              <div class="field">
                <label class="label">Password</label>
                <div class="control has-icons-left">
                  <input
                    class="input"
                    type="password"
                    placeholder="••••••••"
                    value={password()}
                    onInput={(e) => setPassword(e.currentTarget.value)}
                    disabled={authLoading()}
                  />
                  <span class="icon is-left">🔒</span>
                </div>
              </div>
              <div class="field">
                <div class="control">
                  <button
                    class="button is-primary is-fullwidth"
                    type="submit"
                    classList={{ "is-loading": authLoading() }}
                    disabled={authLoading() || !email().trim() || !password()}
                  >
                    Log in
                  </button>
                </div>
              </div>
            </form>
          </Match>

          {/* ── Register (guest upgrade) ──────────────────────── */}
          <Match when={activeTab() === "register"}>
            <p class="mb-4 has-text-grey">
              Create a password account. Your order history will be preserved.
            </p>
            <form onSubmit={handleRegister}>
              <div class="field">
                <label class="label">Email</label>
                <div class="control">
                  <input
                    class="input"
                    type="email"
                    placeholder="you@example.com"
                    value={regEmail()}
                    onInput={(e) => setRegEmail(e.currentTarget.value)}
                    onBlur={suggestNameFromEmail}
                    disabled={authLoading()}
                    autofocus
                  />
                </div>
              </div>
              <div class="field">
                <label class="label">Password</label>
                <div class="control">
                  <input
                    class="input"
                    type="password"
                    placeholder="Minimum 8 characters"
                    value={regPassword()}
                    onInput={(e) => setRegPassword(e.currentTarget.value)}
                    disabled={authLoading()}
                  />
                </div>
              </div>
              <div class="field">
                <label class="label">Display Name</label>
                <div class="control">
                  <input
                    class="input"
                    type="text"
                    placeholder="Leave blank to keep current name"
                    value={regName()}
                    onInput={(e) => setRegName(e.currentTarget.value)}
                    disabled={authLoading()}
                  />
                </div>
                <p class="help">
                  Optional — suggested from your email. Leave blank to keep your current name.
                </p>
              </div>
              <div class="field">
                <div class="control">
                  <button
                    class="button is-primary is-fullwidth"
                    type="submit"
                    classList={{ "is-loading": authLoading() }}
                    disabled={authLoading() || !regEmail().trim() || !regPassword()}
                  >
                    Create Account
                  </button>
                </div>
              </div>
            </form>
          </Match>
        </Switch>
      </div>
    </div>
  );
}
