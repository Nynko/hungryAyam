import { type ParentComponent } from "solid-js";
import { A } from "@solidjs/router";
import { createSignal } from "solid-js";

const MainLayout: ParentComponent = (props) => {
  const [burgerActive, setBurgerActive] = createSignal(false);

  const toggleBurger = () => setBurgerActive(!burgerActive());
  const closeBurger = () => setBurgerActive(false);

  return (
    <>
      <nav class="navbar is-primary" role="navigation" aria-label="main navigation">
        <div class="navbar-brand">
          <A href="/" class="navbar-item has-text-weight-bold is-size-5" onClick={closeBurger}>
            🐔 HungryAyam
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
            <div class="navbar-item">
              <div class="buttons">
                <A href="/login" class="button is-light" onClick={closeBurger}>
                  Log in
                </A>
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
            <strong>HungryAyam</strong> 🐔 — Group food ordering made simple.
          </p>
        </div>
      </footer>
    </>
  );
};

export default MainLayout;