import { type ParentComponent, Show } from "solid-js";
import { A } from "@solidjs/router";
import { createSignal } from "solid-js";
import { appImageUrl, appTitle } from "../env";
import {
  isAuthenticated,
  isGuest,
  isPasswordUser,
  currentUser,
  logout,
} from "@/stores/authStore";
import ConfirmDialog from "@/components/ConfirmDialog";

const MainLayout: ParentComponent = (props) => {
  const [burgerActive, setBurgerActive] = createSignal(false);

  const toggleBurger = () => setBurgerActive(!burgerActive());
  const closeBurger = () => setBurgerActive(false);

  const handleLogout = async () => {
    closeBurger();
    await logout();
  };

  return (
    <>
      <nav class="navbar is-primary" role="navigation" aria-label="main navigation">
        <div class="navbar-brand">
          <A href="/" class="navbar-item has-text-weight-bold is-size-5" onClick={closeBurger}>
            <Show when={appImageUrl} fallback={<span class="mr-2">🐔</span>}>
              <img src={appImageUrl} alt={appTitle} class="mr-2" style={{ "max-height": "1.75rem" }} />
            </Show>
            {appTitle}
          </A>

          <a
            role="button"
            class={`navbar-burger ${burgerActive() ? "is-active" : ""}`}
            aria-label="menu"
            aria-expanded={burgerActive() ? "true" : "false"}
            onClick={toggleBurger}
          >
            <span aria-hidden="true"></span>
            <span aria-hidden="true"></span>
            <span aria-hidden="true"></span>
            <span aria-hidden="true"></span>
          </a>
        </div>

        <div class={`navbar-menu ${burgerActive() ? "is-active" : ""}`}>
          <div class="navbar-start">
            <A href="/restaurants" class="navbar-item" onClick={closeBurger} activeClass="is-active">
              🍽️ Restaurants
            </A>
            <A href="/orders" class="navbar-item" onClick={closeBurger} activeClass="is-active">
              📋 Orders
            </A>
            <A href="/statistics" class="navbar-item" onClick={closeBurger} activeClass="is-active">
              📊 Statistics
            </A>
            <A href="/admin" class="navbar-item" onClick={closeBurger} activeClass="is-active">
              ⚙️ Admin
            </A>
          </div>

          <div class="navbar-end">
            {/* Authenticated user info */}
            <Show when={isAuthenticated()}>
              <div class="navbar-item">
                <span class="has-text-weight-semibold">
                  👤 {currentUser()?.name}
                  <Show when={isGuest()}>
                    <span class="tag is-light is-small ml-2">guest</span>
                  </Show>
                </span>
              </div>
            </Show>

            <div class="navbar-item">
              <div class="buttons">
                {/* Guest: show "Log in" to upgrade + "Log out" */}
                <Show when={isGuest()}>
                  <A href="/login" class="button is-light" onClick={closeBurger}>
                    Log in
                  </A>
                  <button class="button is-light is-outlined" onClick={handleLogout}>
                    Log out
                  </button>
                </Show>

                {/* Password user: show "Log out" only */}
                <Show when={isPasswordUser()}>
                  <button class="button is-light" onClick={handleLogout}>
                    Log out
                  </button>
                </Show>

                {/* Not authenticated: show "Log in" */}
                <Show when={!isAuthenticated()}>
                  <A href="/login" class="button is-light" onClick={closeBurger}>
                    Log in
                  </A>
                </Show>
              </div>
            </div>
          </div>
        </div>
      </nav>

      <main>
        {props.children}
      </main>

      <footer class="footer">
        <div class="content has-text-centered">
          <p>
            <strong>{appTitle}</strong> — Group food ordering made simple.
          </p>
        </div>
      </footer>

      <ConfirmDialog />
    </>
  );
};

export default MainLayout;