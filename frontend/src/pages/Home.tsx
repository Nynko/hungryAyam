import { A } from "@solidjs/router";
import { appImageUrl, appTitle } from "../env";
import { Show } from "solid-js";

export default function Home() {
  return (
    <section class="hero is-medium is-primary">
      <div class="hero-body">
        <div class="container has-text-centered">
          <Show when={appImageUrl} fallback={<span class="title is-1">🐔</span>}>
            <img src={appImageUrl} alt={appTitle} style={{ height: "10rem", width: "auto" }} />
          </Show>
          <p class="title is-1">{appTitle}</p>
          {/*<p class="subtitle is-4">
            Group food ordering made simple.
          </p>*/}
          <div class="buttons is-centered mt-5">
            <A href="/restaurants" class="button is-light is-medium">
              🍽️ Browse Restaurants
            </A>
            <A href="/orders" class="button is-outlined is-light is-medium">
              📋 View Orders
            </A>
          </div>
        </div>
      </div>
    </section>
  );
}
