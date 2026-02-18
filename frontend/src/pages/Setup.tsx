import { createSignal, Show } from "solid-js";
import { submitSetup, setupLoading, setupError } from "../stores/setupStore";

const appTitle = import.meta.env.VITE_APP_TITLE || "HungryAyam";

export default function Setup() {
  // Access code fields
  const [accessCode, setAccessCode] = createSignal("");
  const [confirmCode, setConfirmCode] = createSignal("");

  // Admin user fields
  const [adminName, setAdminName] = createSignal("");
  const [adminEmail, setAdminEmail] = createSignal("");
  const [adminPassword, setAdminPassword] = createSignal("");
  const [confirmPassword, setConfirmPassword] = createSignal("");

  const [localError, setLocalError] = createSignal<string | null>(null);
  const [showPasswords, setShowPasswords] = createSignal(false);

  const togglePasswords = () => setShowPasswords(!showPasswords());
  const inputType = () => showPasswords() ? "text" : "password";

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setLocalError(null);

    const code = accessCode().trim();
    const confirm = confirmCode().trim();
    const name = adminName().trim();
    const email = adminEmail().trim();
    const password = adminPassword();
    const passwordConfirm = confirmPassword();

    // Validate access code
    if (!code) {
      setLocalError("Access code cannot be empty.");
      return;
    }

    if (code.length < 4) {
      setLocalError("Access code must be at least 4 characters.");
      return;
    }

    if (code !== confirm) {
      setLocalError("Access codes do not match.");
      return;
    }

    // Validate admin fields
    if (!name) {
      setLocalError("Admin name cannot be empty.");
      return;
    }

    if (!email) {
      setLocalError("Admin email cannot be empty.");
      return;
    }

    if (!email.includes("@")) {
      setLocalError("Please enter a valid email address.");
      return;
    }

    if (password.length < 8) {
      setLocalError("Admin password must be at least 8 characters.");
      return;
    }

    if (password !== passwordConfirm) {
      setLocalError("Admin passwords do not match.");
      return;
    }

    const success = await submitSetup({
      access_code: code,
      admin_name: name,
      admin_email: email,
      admin_password: password,
    });

    if (success) {
      window.location.reload();
    }
  };

  const errorMessage = () => localError() || setupError();

  return (
    <section class="hero is-fullheight-with-navbar is-primary">
      <div class="hero-body">
        <div class="container">
          <div class="columns is-centered">
            <div class="column is-6-tablet is-5-desktop">
              <div class="box">
                <div class="has-text-centered mb-5">
                  <p class="title is-3 has-text-primary">🐔 {appTitle}</p>
                  <p class="subtitle is-6 has-text-grey">
                    Welcome! Let's set up your app.
                  </p>
                </div>

                <form onSubmit={handleSubmit}>
                  {/* ── Guest Access Code Section ─────────────────── */}
                  <div class="mb-5">
                    <h2 class="title is-5 has-text-grey-dark">
                      🔑 Guest Access Code
                    </h2>
                    <p class="has-text-grey is-size-7 mb-3">
                      This password will be shared with people you invite to use the app.
                    </p>

                    <div class="field">
                      <label class="label">Access Code</label>
                      <div class="field has-addons">
                        <div class="control is-expanded">
                          <input
                            class="input"
                            type={inputType()}
                            placeholder="Enter a memorable password"
                            value={accessCode()}
                            onInput={(e) => setAccessCode(e.currentTarget.value)}
                            disabled={setupLoading()}
                            autofocus
                          />
                        </div>
                        <div class="control">
                          <button
                            type="button"
                            class="button"
                            onClick={togglePasswords}
                            tabIndex={-1}
                          >
                            {showPasswords() ? "🙈" : "👁️"}
                          </button>
                        </div>
                      </div>
                    </div>

                    <div class="field">
                      <label class="label">Confirm Access Code</label>
                      <div class="control">
                        <input
                          class="input"
                          type={inputType()}
                          placeholder="Confirm your access code"
                          value={confirmCode()}
                          onInput={(e) => setConfirmCode(e.currentTarget.value)}
                          disabled={setupLoading()}
                        />
                      </div>
                    </div>
                  </div>

                  {/* ── Admin Account Section ─────────────────────── */}
                  <div class="mb-5">
                    <h2 class="title is-5 has-text-grey-dark">
                      👑 Admin Account
                    </h2>
                    <p class="has-text-grey is-size-7 mb-3">
                      Create your administrator account to manage the app.
                    </p>

                    <div class="field">
                      <label class="label">Name</label>
                      <div class="control">
                        <input
                          class="input"
                          type="text"
                          placeholder="Your name"
                          value={adminName()}
                          onInput={(e) => setAdminName(e.currentTarget.value)}
                          disabled={setupLoading()}
                        />
                      </div>
                    </div>

                    <div class="field">
                      <label class="label">Email</label>
                      <div class="control">
                        <input
                          class="input"
                          type="email"
                          placeholder="admin@example.com"
                          value={adminEmail()}
                          onInput={(e) => setAdminEmail(e.currentTarget.value)}
                          disabled={setupLoading()}
                        />
                      </div>
                      <p class="help">You'll use this email to log in.</p>
                    </div>

                    <div class="field">
                      <label class="label">Password</label>
                      <div class="control">
                        <input
                          class="input"
                          type={inputType()}
                          placeholder="At least 8 characters"
                          value={adminPassword()}
                          onInput={(e) => setAdminPassword(e.currentTarget.value)}
                          disabled={setupLoading()}
                        />
                      </div>
                    </div>

                    <div class="field">
                      <label class="label">Confirm Password</label>
                      <div class="control">
                        <input
                          class="input"
                          type={inputType()}
                          placeholder="Confirm your password"
                          value={confirmPassword()}
                          onInput={(e) => setConfirmPassword(e.currentTarget.value)}
                          disabled={setupLoading()}
                        />
                      </div>
                    </div>
                  </div>

                  <Show when={errorMessage()}>
                    <div class="notification is-danger is-light">
                      {errorMessage()}
                    </div>
                  </Show>

                  <div class="field mt-5">
                    <div class="control">
                      <button
                        class={`button is-primary is-fullwidth ${setupLoading() ? "is-loading" : ""}`}
                        type="submit"
                        disabled={setupLoading()}
                      >
                        Complete Setup
                      </button>
                    </div>
                  </div>
                </form>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
