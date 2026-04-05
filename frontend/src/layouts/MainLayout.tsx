import { type ParentComponent, Show } from "solid-js";
import { A } from "@solidjs/router";
import { createSignal } from "solid-js";
import { appImageUrl, appTitle } from "../env";
import {
  isAuthenticated,
  isGuest,
  isPasswordUser,
  isAdmin,
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
            <Show when={isAdmin()}>
              <A href="/admin" class="navbar-item" onClick={closeBurger} activeClass="is-active">
                ⚙️ Admin
              </A>
            </Show>
          </div>

          <div class="navbar-end">
            {/* Authenticated user info */}
            <Show when={isAuthenticated()}>
              <div class="navbar-item">
                <span class="has-text-weight-semibold navbar-user-chip">
                  <span>👤 {currentUser()?.name}</span>
                  <Show when={isGuest()}>
                    <span class="tag is-small navbar-role-tag">guest</span>
                  </Show>
                </span>
              </div>
            </Show>

            <div class="navbar-item">
              <div class="buttons">
                {/* Guest: show "Log in" to upgrade + "Log out" */}
                <Show when={isGuest()}>
                  <A href="/login" class="button navbar-action-button" onClick={closeBurger}>
                    Log in
                  </A>
                  <button class="button navbar-action-button is-subtle" onClick={handleLogout}>
                    Log out
                  </button>
                </Show>

                {/* Password user: show "My Account" + "Log out" */}
                <Show when={isPasswordUser()}>
                  <A href="/login" class="button navbar-action-button" onClick={closeBurger}>
                    My Account
                  </A>
                  <button class="button navbar-action-button is-subtle" onClick={handleLogout}>
                    Log out
                  </button>
                </Show>

                {/* Not authenticated: show "Log in" */}
                <Show when={!isAuthenticated()}>
                  <A href="/login" class="button navbar-action-button" onClick={closeBurger}>
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
            <strong>HungryAyam</strong> — Group food ordering made simple.
          </p>
        </div>
      </footer>

      <ConfirmDialog />
    </>
  );
};

export default MainLayout;
