import { Show } from "solid-js";
import { isAuthenticated, isGuest, isPasswordUser, currentUser, logout } from "@/stores/authStore";
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
                <div class="box has-text-centered">
                  <p class="is-size-4 mb-3">👋</p>
                  <p class="title is-5">
                    Welcome back, {currentUser()?.name}!
                  </p>
                  <p class="has-text-grey mb-4">
                    You are logged in.
                  </p>
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
                    Log in with an account to upgrade, or continue browsing.
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