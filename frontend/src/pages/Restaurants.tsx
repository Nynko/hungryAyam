import { onMount, createSignal, createMemo, Show, For } from "solid-js";
import {
  restaurants,
  restaurantsLoading,
  restaurantsError,
  fetchRestaurants,
  refetchRestaurants,
} from "@/stores/restaurantStore";
import { isEditor } from "@/stores/authStore";
import RestaurantCard from "@/components/RestaurantCard";
import SearchBar from "@/components/SearchBar";
import Pagination from "@/components/Pagination";
import CreateRestaurantModal from "@/components/CreateRestaurantModal";

const ITEMS_PER_PAGE = 12;

export default function Restaurants() {
  const [search, setSearch] = createSignal("");
  const [currentPage, setCurrentPage] = createSignal(1);
  const [showCreateModal, setShowCreateModal] = createSignal(false);

  onMount(() => {
    fetchRestaurants();
  });

  // ── Derived state ───────────────────────────────────────────────
  const filtered = createMemo(() => {
    const q = search().toLowerCase().trim();
    if (!q) return restaurants();
    return restaurants().filter((r) => r.name.toLowerCase().includes(q));
  });

  const totalPages = createMemo(() =>
    Math.max(1, Math.ceil(filtered().length / ITEMS_PER_PAGE))
  );

  const safePage = createMemo(() => {
    const p = currentPage();
    return p > totalPages() ? 1 : p;
  });

  const paginatedRestaurants = createMemo(() => {
    const start = (safePage() - 1) * ITEMS_PER_PAGE;
    return filtered().slice(start, start + ITEMS_PER_PAGE);
  });

  // ── Handlers ────────────────────────────────────────────────────
  const handleSearch = (value: string) => {
    setSearch(value);
    setCurrentPage(1);
  };

  const handlePageChange = (page: number) => {
    if (page >= 1 && page <= totalPages()) setCurrentPage(page);
  };

  return (
    <section class="section">
      <div class="container">
        {/* ── Header ───────────────────────────────────────────── */}
        <div class="level">
          <div class="level-left">
            <div class="level-item">
              <div>
                <h1 class="title">🍽️ Restaurants</h1>
                <p class="subtitle">
                  Browse available restaurants and start an order.
                </p>
              </div>
            </div>
          </div>
          <div class="level-right">
            <Show when={isEditor()}>
              <div class="level-item">
                <button
                  class="button is-primary"
                  onClick={() => setShowCreateModal(true)}
                >
                  <span class="icon">
                    <span>➕</span>
                  </span>
                  <span>Add Restaurant</span>
                </button>
              </div>
            </Show>
            <div class="level-item">
              <button
                class="button is-primary is-outlined"
                classList={{ "is-loading": restaurantsLoading() }}
                onClick={() => refetchRestaurants()}
              >
                <span class="icon">
                  <span>🔄</span>
                </span>
                <span>Refresh</span>
              </button>
            </div>
          </div>
        </div>

        {/* ── Search ───────────────────────────────────────────── */}
        <SearchBar
          value={search()}
          onInput={handleSearch}
          placeholder="Search restaurants..."
        />

        {/* ── Error state ──────────────────────────────────────── */}
        <Show when={restaurantsError() && !showCreateModal()}>
          <div class="notification is-danger is-light">
            <button class="delete" onClick={() => refetchRestaurants()} />
            <strong>Failed to load restaurants:</strong>{" "}
            {restaurantsError()}
          </div>
        </Show>

        {/* ── Loading state ────────────────────────────────────── */}
        <Show when={restaurantsLoading() && restaurants().length === 0}>
          <div class="has-text-centered py-6">
            <progress class="progress is-primary is-small" max="100" />
            <p class="has-text-grey mt-2">Loading restaurants…</p>
          </div>
        </Show>

        {/* ── Empty state ──────────────────────────────────────── */}
        <Show
          when={
            !restaurantsLoading() &&
            !restaurantsError() &&
            restaurants().length === 0
          }
        >
          <div class="notification is-info is-light has-text-centered">
            <p class="is-size-4 mb-2">🍳</p>
            <p>No restaurants yet. Add one from the admin panel!</p>
          </div>
        </Show>

        {/* ── No results for filter ────────────────────────────── */}
        <Show
          when={
            !restaurantsLoading() &&
            restaurants().length > 0 &&
            filtered().length === 0
          }
        >
          <div class="notification is-warning is-light has-text-centered">
            <p class="is-size-4 mb-2">🔎</p>
            <p>
              No restaurants match "<strong>{search()}</strong>".
            </p>
          </div>
        </Show>

        {/* ── Restaurant card grid ─────────────────────────────── */}
        <Show when={paginatedRestaurants().length > 0}>
          <div class="columns is-multiline">
            <For each={paginatedRestaurants()}>
              {(restaurant) => (
                <div class="column is-4-desktop is-6-tablet is-12-mobile">
                  <RestaurantCard restaurant={restaurant} />
                </div>
              )}
            </For>
          </div>

          {/* ── Pagination ───────────────────────────────────── */}
          <Pagination
            currentPage={safePage()}
            totalPages={totalPages()}
            onPageChange={handlePageChange}
          />

          {/* ── Result count ─────────────────────────────────── */}
          <p class="has-text-grey has-text-centered mt-4 is-size-7">
            Showing {(safePage() - 1) * ITEMS_PER_PAGE + 1}–
            {Math.min(safePage() * ITEMS_PER_PAGE, filtered().length)} of{" "}
            {filtered().length} restaurant{filtered().length !== 1 ? "s" : ""}
            <Show when={search()}>
              {" "}
              matching "<strong>{search()}</strong>"
            </Show>
          </p>
        </Show>

        {/* ── Create restaurant modal ──────────────────────────── */}
        <CreateRestaurantModal
          isOpen={showCreateModal()}
          onClose={() => setShowCreateModal(false)}
        />
      </div>
    </section>
  );
}