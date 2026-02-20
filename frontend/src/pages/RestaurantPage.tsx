import { createSignal, createResource, Show, For } from "solid-js";
import { A, useParams } from "@solidjs/router";
import type { Restaurant } from "@bindings/Restaurant";
import type { Menu } from "@bindings/Menu";
import type { ApiResponse } from "@bindings/ApiResponse";
import MenuView from "@/components/MenuView";
import { Card } from "@/components/Card";
import { isAuthenticated } from "@/stores/authStore";

async function fetchRestaurant(id: string): Promise<Restaurant> {
  const res = await fetch(`/api/restaurants/${id}`);
  if (!res.ok) {
    if (res.status === 404) throw new Error("Restaurant not found");
    throw new Error(`Failed to load restaurant (${res.status})`);
  }
  const json: ApiResponse<Restaurant> = await res.json();
  if (!json.success || json.data == null) {
    throw new Error(json.error ?? "Unexpected response");
  }
  return json.data;
}

async function fetchMenus(restaurantId: string): Promise<Menu[]> {
  const res = await fetch(`/api/restaurants/${restaurantId}/menus`);
  if (!res.ok) {
    throw new Error(`Failed to load menus (${res.status})`);
  }
  const json: ApiResponse<Menu[]> = await res.json();
  if (!json.success || json.data == null) {
    throw new Error(json.error ?? "Unexpected response");
  }
  return json.data;
}

export default function RestaurantPage() {
  const params = useParams<{ id: string }>();

  const [restaurant] = createResource(() => params.id, fetchRestaurant);
  const [menus] = createResource(() => params.id, fetchMenus);

  const activeMenus = () =>
    (menus() ?? []).filter((m) => m.is_active).sort((a, b) => a.name.localeCompare(b.name));

  const inactiveMenus = () =>
    (menus() ?? []).filter((m) => !m.is_active).sort((a, b) => a.name.localeCompare(b.name));

  const [showInactive, setShowInactive] = createSignal(false);

  return (
    <section class="section">
      <div class="container">
        {/* ── Back link ───────────────────────────────────────── */}
        <div class="mb-4">
          <A href="/restaurants" class="button is-light is-small">
            <span class="mr-1">←</span> Back to Restaurants
          </A>
        </div>

        {/* ── Loading ─────────────────────────────────────────── */}
        <Show when={restaurant.loading}>
          <div class="has-text-centered py-6">
            <progress class="progress is-primary is-small" max="100" />
            <p class="has-text-grey mt-2">Loading restaurant…</p>
          </div>
        </Show>

        {/* ── Error ───────────────────────────────────────────── */}
        <Show when={restaurant.error}>
          <div class="notification is-danger is-light">
            <p>
              <strong>Error:</strong> {(restaurant.error as Error)?.message ?? "Something went wrong"}
            </p>
            <A href="/restaurants" class="button is-small is-danger is-outlined mt-3">
              ← Go back
            </A>
          </div>
        </Show>

        {/* ── Restaurant loaded ───────────────────────────────── */}
        <Show when={restaurant()}>
          {(r) => (
            <>
              {/* ── Hero header ────────────────────────────────── */}
              <Card class="mb-5">
                <Show when={r().image_url}>
                  <div class="card-image">
                    <figure
                      class="image is-3by1"
                      style={{
                        "background-color": "var(--bulma-scheme-main-bis)",
                        display: "flex",
                        "align-items": "center",
                        "justify-content": "center",
                        overflow: "hidden",
                      }}
                    >
                      <img
                        src={r().image_url!}
                        alt={r().name}
                        style={{
                          "object-fit": "contain",
                          "max-width": "100%",
                          "max-height": "100%",
                        }}
                      />
                    </figure>
                  </div>
                </Show>
                <div class="card-content">
                  <div class="is-flex is-justify-content-space-between is-align-items-center">
                    <div>
                      <h1 class="title is-3 mb-1">
                        <Show when={!r().image_url}>
                          <span class="mr-2">🍽️</span>
                        </Show>
                        {r().name}
                      </h1>
                      <p class="has-text-grey is-size-7">
                        Added {new Date(r().created_at).toLocaleDateString()}
                      </p>
                    </div>
                  </div>
                </div>
              </Card>

              {/* ── Menus section ──────────────────────────────── */}
              <div class="mb-4">
                <div class="is-flex is-justify-content-space-between is-align-items-center mb-4">
                  <h2 class="title is-4 mb-0">📋 Menus</h2>
                  <Show when={isAuthenticated()}>
                    <A
                      href={`/restaurants/${r().id}/menus/new`}
                      class="button is-primary"
                    >
                      <span class="icon is-small">
                        <span>➕</span>
                      </span>
                      <span>Create Menu</span>
                    </A>
                  </Show>
                </div>

                {/* Menus loading */}
                <Show when={menus.loading}>
                  <div class="has-text-centered py-4">
                    <progress class="progress is-primary is-small" max="100" />
                    <p class="has-text-grey mt-2">Loading menus…</p>
                  </div>
                </Show>

                {/* Menus error */}
                <Show when={menus.error}>
                  <div class="notification is-danger is-light">
                    <strong>Failed to load menus:</strong>{" "}
                    {(menus.error as Error)?.message ?? "Unknown error"}
                  </div>
                </Show>

                {/* No menus */}
                <Show
                  when={
                    !menus.loading &&
                    !menus.error &&
                    (menus() ?? []).length === 0
                  }
                >
                  <div class="notification is-info is-light has-text-centered">
                    <p class="is-size-4 mb-2">📭</p>
                    <p>This restaurant doesn't have any menus yet.</p>
                  </div>
                </Show>

                {/* Active menus */}
                <Show when={activeMenus().length > 0}>
                  <For each={activeMenus()}>
                    {(menu) => (
                      <div class="mb-5">
                        <MenuView menu={menu} />
                        <Show when={isAuthenticated()}>
                          <div class="has-text-right mt-2">
                            <A
                              href={`/restaurants/${r().id}/menus/${menu.id}/edit`}
                              class="button is-small is-info is-outlined"
                            >
                              <span class="icon is-small">
                                <span>✏️</span>
                              </span>
                              <span>Edit this menu</span>
                            </A>
                          </div>
                        </Show>
                      </div>
                    )}
                  </For>
                </Show>

                {/* No active menus but has inactive ones */}
                <Show
                  when={
                    activeMenus().length === 0 &&
                    inactiveMenus().length > 0 &&
                    !menus.loading
                  }
                >
                  <div class="notification is-warning is-light has-text-centered mb-4">
                    <p>No active menus. There {inactiveMenus().length === 1 ? "is" : "are"} {inactiveMenus().length} inactive menu{inactiveMenus().length !== 1 ? "s" : ""}.</p>
                  </div>
                </Show>

                {/* Toggle inactive menus */}
                <Show when={inactiveMenus().length > 0}>
                  <div class="has-text-centered mt-4 mb-4">
                    <button
                      class="button is-small is-light"
                      onClick={() => setShowInactive(!showInactive())}
                    >
                      <span class="mr-1">{showInactive() ? "▼" : "▶"}</span>
                      {showInactive() ? "Hide" : "Show"} inactive menus ({inactiveMenus().length})
                    </button>
                  </div>

                  <Show when={showInactive()}>
                    <For each={inactiveMenus()}>
                      {(menu) => (
                        <div style={{ opacity: "0.7" }} class="mb-5">
                          <MenuView menu={menu} />
                          <Show when={isAuthenticated()}>
                            <div class="has-text-right mt-2">
                              <A
                                href={`/restaurants/${r().id}/menus/${menu.id}/edit`}
                                class="button is-small is-info is-outlined"
                              >
                                <span class="icon is-small">
                                  <span>✏️</span>
                                </span>
                                <span>Edit this menu</span>
                              </A>
                            </div>
                          </Show>
                        </div>
                      )}
                    </For>
                  </Show>
                </Show>
              </div>
            </>
          )}
        </Show>
      </div>
    </section>
  );
}