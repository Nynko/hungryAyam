import {
  createSignal,
  createResource,
  Show,
  For,
  createMemo,
  onMount,
} from "solid-js";
import { A } from "@solidjs/router";
import type { Restaurant } from "@bindings/Restaurant";
import type { OrderSession } from "@bindings/OrderSession";
import type { ApiResponse } from "@bindings/ApiResponse";
import type { OrderSessionStatus } from "@bindings/OrderSessionStatus";
import { Card } from "@/components/Card";
import { isImageSrc } from "@/lib/imageUrl";
import ActiveSessionBanner from "@/components/ActiveSessionBanner";
import { isAuthenticated } from "@/stores/authStore";
import {
  fetchSessionsForRestaurant,
  fetchActiveSession,
  getActiveSession,
  sessionStatusColor,
  formatPrice,
  groupOrderItems,
  orderError,
  clearOrderError,
} from "@/stores/orderStore";

// ── Data fetchers ─────────────────────────────────────────────────

async function fetchRestaurants(): Promise<Restaurant[]> {
  const res = await fetch("/api/restaurants");
  if (!res.ok) throw new Error(`Failed to load restaurants (${res.status})`);
  const json: ApiResponse<Restaurant[]> = await res.json();
  if (!json.success || json.data == null) {
    throw new Error(json.error ?? "Unexpected response");
  }
  return json.data;
}

// ── Status filter type ────────────────────────────────────────────

type StatusFilter = "all" | OrderSessionStatus;

// ── Helper: format date compactly ─────────────────────────────────

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    dateStyle: "short",
    timeStyle: "short",
  });
}

// ══════════════════════════════════════════════════════════════════
// Sub-component: expanded session detail (order table)
// ══════════════════════════════════════════════════════════════════

function SessionOrderList(props: { session: OrderSession }) {
  const orders = createMemo(() => props.session.orders ?? []);
  const totalRevenue = createMemo(() =>
    orders().reduce((sum, o) => sum + o.total_price_cents, 0),
  );

  /** Aggregate all items across every order in the session. */
  const aggregatedCommand = createMemo(() => {
    const allItems = orders().flatMap((o) => o.items);
    return groupOrderItems(allItems);
  });

  return (
    <div class="mt-3">
      <Show
        when={orders().length > 0}
        fallback={
          <p class="has-text-grey is-size-7 is-italic ml-2">
            No orders in this session yet.
          </p>
        }
      >
        {/* Summary bar */}
        <div class="is-flex is-flex-wrap-wrap mb-3" style={{ gap: "1rem" }}>
          <span class="tag is-light">
            {orders().length} order{orders().length !== 1 ? "s" : ""}
          </span>
          <span class="tag is-light">
            Total: ${formatPrice(totalRevenue())}
          </span>
        </div>

        {/* ── Aggregated command for the restaurant ────────────── */}
        <Show when={aggregatedCommand().length > 0}>
          <div
            class="box mb-4 p-3"
            style={{
              background: "hsl(204, 86%, 96%)",
              "border-left": "4px solid hsl(204, 86%, 53%)",
            }}
          >
            <p class="has-text-weight-bold is-size-7 mb-2">
              🧾 Full command to send
            </p>
            <ul class="ml-4" style={{ "list-style": "disc" }}>
              <For each={aggregatedCommand()}>
                {(group) => (
                  <li>
                    <span class="has-text-weight-bold">{group.quantity}</span>
                    {" "}
                    <span>{group.itemName}</span>
                    <Show when={group.notes.length > 0}>
                      <span class="has-text-grey is-italic is-size-7 ml-1">
                        ({group.notes.join(", ")})
                      </span>
                    </Show>
                  </li>
                )}
              </For>
            </ul>
          </div>
        </Show>

        {/* Individual orders */}
        <div class="table-container">
          <table class="table is-fullwidth is-striped is-hoverable is-size-7">
            <thead>
              <tr>
                <th>#</th>
                <th>User</th>
                <th>Items</th>
                <th>Total</th>
                <th>Placed</th>
              </tr>
            </thead>
            <tbody>
              <For each={orders()}>
                {(order, idx) => (
                  <tr>
                    <td>{idx() + 1}</td>
                    <td>
                      <span class="has-text-weight-medium">
                        {order.user_name}
                      </span>
                    </td>
                    <td>
                      <Show
                        when={order.items.length > 0}
                        fallback={<span class="has-text-grey">—</span>}
                      >
                        <For each={groupOrderItems(order.items)}>
                          {(group, idx) => (
                            <>
                              <Show when={idx() > 0}>
                                <span class="has-text-grey mx-1">–</span>
                              </Show>
                              <span class="has-text-weight-medium">
                                {group.quantity} {group.itemName}
                              </span>
                              <Show when={group.notes.length > 0}>
                                <span class="has-text-grey is-italic ml-1">
                                  ({group.notes.join(", ")})
                                </span>
                              </Show>
                            </>
                          )}
                        </For>
                      </Show>
                    </td>
                    <td class="has-text-weight-bold">
                      ${formatPrice(order.total_price_cents)}
                    </td>
                    <td>{fmtDate(order.created_at)}</td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
    </div>
  );
}

// ══════════════════════════════════════════════════════════════════
// Sub-component: one active session card (used in the top section)
// ══════════════════════════════════════════════════════════════════

function ActiveSessionCard(props: {
  session: OrderSession;
  restaurant: Restaurant;
  onChanged: () => void;
}) {
  const [showOrders, setShowOrders] = createSignal(false);
  const orderCount = createMemo(() => props.session.orders?.length ?? 0);

  return (
    <Card class="mb-4">
      <div class="card-content">
        {/* Restaurant name link */}
        <div class="is-flex is-align-items-center mb-3" style={{ gap: "0.5rem" }}>
          <Show when={props.restaurant.image_url}>
            {isImageSrc(props.restaurant.image_url!) ? (
              <figure
                class="image is-24x24"
                style={{
                  "border-radius": "4px",
                  overflow: "hidden",
                  "flex-shrink": "0",
                  "min-width": "24px",
                }}
              >
                <img
                  src={props.restaurant.image_url!}
                  alt={props.restaurant.name}
                  style={{
                    "object-fit": "cover",
                    width: "100%",
                    height: "100%",
                  }}
                />
              </figure>
            ) : (
              <span style={{ "font-size": "1.25rem", "line-height": "1" }}>
                {props.restaurant.image_url}
              </span>
            )}
          </Show>
          <A
            href={`/restaurants/${props.restaurant.id}`}
            class="has-text-weight-bold is-size-5"
          >
            {props.restaurant.name}
          </A>
        </div>

        {/* Session banner with admin controls */}
        <ActiveSessionBanner
          session={props.session}
          restaurantId={props.restaurant.id}
          onSessionChanged={props.onChanged}
        />

        {/* Toggle to see orders */}
        <Show when={orderCount() > 0}>
          <button
            class="button is-small is-light"
            onClick={() => setShowOrders(!showOrders())}
          >
            <span class="mr-1">{showOrders() ? "▼" : "▶"}</span>
            {showOrders() ? "Hide" : "Show"} orders ({orderCount()})
          </button>

          <Show when={showOrders()}>
            <SessionOrderList session={props.session} />
          </Show>
        </Show>

        <Show when={orderCount() === 0}>
          <p class="has-text-grey is-size-7 is-italic">
            No orders placed yet.
          </p>
        </Show>
      </div>
    </Card>
  );
}

// ══════════════════════════════════════════════════════════════════
// Sub-component: per-restaurant history (lazy-loaded, expandable)
// ══════════════════════════════════════════════════════════════════

function RestaurantHistory(props: { restaurant: Restaurant }) {
  const [sessions, setSessions] = createSignal<OrderSession[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [loaded, setLoaded] = createSignal(false);
  const [expanded, setExpanded] = createSignal(false);
  const [expandedSessionId, setExpandedSessionId] = createSignal<
    string | null
  >(null);
  const [statusFilter, setStatusFilter] = createSignal<StatusFilter>("all");

  const load = async () => {
    if (loaded()) return;
    setLoading(true);
    const result = await fetchSessionsForRestaurant(props.restaurant.id);
    setSessions(result);
    setLoaded(true);
    setLoading(false);
  };

  const toggle = async () => {
    if (!expanded()) {
      await load();
    }
    setExpanded(!expanded());
  };

  const toggleSession = (id: string) => {
    setExpandedSessionId((prev) => (prev === id ? null : id));
  };

  const filteredSessions = createMemo(() => {
    const filter = statusFilter();
    if (filter === "all") return sessions();
    return sessions().filter((s) => s.status === filter);
  });

  const sessionCounts = createMemo(() => {
    const counts: Record<string, number> = {
      all: sessions().length,
      Open: 0,
      Closed: 0,
      Sent: 0,
      Cancelled: 0,
    };
    for (const s of sessions()) {
      counts[s.status] = (counts[s.status] ?? 0) + 1;
    }
    return counts;
  });

  return (
    <Card class="mb-4">
      {/* Restaurant header — clickable to expand */}
      <header
        class="card-header"
        style={{ cursor: "pointer" }}
        onClick={toggle}
      >
        <div
          class="card-header-title is-flex is-justify-content-space-between is-align-items-center"
          style={{ width: "100%" }}
        >
          <div
            class="is-flex is-align-items-center"
            style={{ gap: "0.5rem" }}
          >
            <Show when={props.restaurant.image_url}>
              {isImageSrc(props.restaurant.image_url!) ? (
                <figure
                  class="image is-32x32"
                  style={{
                    "border-radius": "6px",
                    overflow: "hidden",
                    "flex-shrink": "0",
                  }}
                >
                  <img
                    src={props.restaurant.image_url!}
                    alt={props.restaurant.name}
                    style={{
                      "object-fit": "cover",
                      width: "100%",
                      height: "100%",
                    }}
                  />
                </figure>
              ) : (
                <span style={{ "font-size": "1.5rem", "line-height": "1" }}>
                  {props.restaurant.image_url}
                </span>
              )}
            </Show>
            <span class="is-size-5">{props.restaurant.name}</span>
          </div>
          <span class="icon is-small has-text-grey">
            <span>{expanded() ? "▼" : "▶"}</span>
          </span>
        </div>
      </header>

      {/* Expanded content — session list */}
      <Show when={expanded()}>
        <div class="card-content">
          {/* Loading */}
          <Show when={loading()}>
            <div class="has-text-centered py-4">
              <progress class="progress is-primary is-small" max="100" />
              <p class="has-text-grey mt-2 is-size-7">Loading sessions…</p>
            </div>
          </Show>

          <Show when={loaded() && !loading()}>
            {/* No sessions */}
            <Show when={sessions().length === 0}>
              <div class="has-text-centered py-4">
                <p class="is-size-4 mb-2">📭</p>
                <p class="has-text-grey">
                  No order sessions for this restaurant yet.
                </p>
                <A
                  href={`/restaurants/${props.restaurant.id}`}
                  class="button is-small is-primary is-outlined mt-3"
                >
                  Go to restaurant →
                </A>
              </div>
            </Show>

            {/* Has sessions */}
            <Show when={sessions().length > 0}>
              {/* Status filter tabs */}
              <div class="tabs is-small is-toggle mb-4">
                <ul>
                  <For
                    each={
                      [
                        { key: "all" as StatusFilter, label: "All" },
                        {
                          key: "Open" as StatusFilter,
                          label: "🟢 Open",
                        },
                        {
                          key: "Closed" as StatusFilter,
                          label: "🟡 Closed",
                        },
                        {
                          key: "Sent" as StatusFilter,
                          label: "📨 Sent",
                        },
                        {
                          key: "Cancelled" as StatusFilter,
                          label: "❌ Cancelled",
                        },
                      ] as const
                    }
                  >
                    {(tab) => (
                      <li
                        classList={{
                          "is-active": statusFilter() === tab.key,
                        }}
                      >
                        <a onClick={() => setStatusFilter(tab.key)}>
                          {tab.label}
                          <Show when={sessionCounts()[tab.key] > 0}>
                            <span class="ml-1 has-text-grey">
                              ({sessionCounts()[tab.key]})
                            </span>
                          </Show>
                        </a>
                      </li>
                    )}
                  </For>
                </ul>
              </div>

              {/* Filtered session list */}
              <Show
                when={filteredSessions().length > 0}
                fallback={
                  <p class="has-text-grey has-text-centered py-3 is-size-7">
                    No sessions match this filter.
                  </p>
                }
              >
                <For each={filteredSessions()}>
                  {(session) => {
                    const isExpanded = createMemo(
                      () => expandedSessionId() === session.id,
                    );
                    const orderCount = createMemo(
                      () => session.orders?.length ?? 0,
                    );
                    const totalCents = createMemo(() =>
                      (session.orders ?? []).reduce(
                        (sum, o) => sum + o.total_price_cents,
                        0,
                      ),
                    );

                    return (
                      <div
                        class="box mb-3 p-3"
                        style={{
                          "border-left": "4px solid",
                          "border-left-color":
                            session.status === "Open"
                              ? "hsl(141, 71%, 48%)"
                              : session.status === "Closed"
                                ? "hsl(48, 100%, 67%)"
                                : session.status === "Sent"
                                  ? "hsl(204, 86%, 53%)"
                                  : "hsl(348, 100%, 61%)",
                        }}
                      >
                        {/* Session header row */}
                        <div
                          class="is-flex is-justify-content-space-between is-align-items-center is-flex-wrap-wrap"
                          style={{
                            cursor: "pointer",
                            gap: "0.5rem",
                          }}
                          onClick={() => toggleSession(session.id)}
                        >
                          <div
                            class="is-flex is-align-items-center is-flex-wrap-wrap"
                            style={{ gap: "0.5rem" }}
                          >
                            <span
                              class={`tag ${sessionStatusColor(session.status)} is-small`}
                            >
                              {session.status}
                            </span>
                            <span class="is-size-7">
                              {fmtDate(session.start_date)} —{" "}
                              {fmtDate(session.end_date)}
                            </span>
                            <Show when={session.allow_late}>
                              <span class="tag is-light is-small">
                                Late OK
                              </span>
                            </Show>
                          </div>

                          <div
                            class="is-flex is-align-items-center"
                            style={{ gap: "0.75rem" }}
                          >
                            <span class="is-size-7 has-text-grey">
                              {orderCount()} order
                              {orderCount() !== 1 ? "s" : ""}
                            </span>
                            <Show when={totalCents() > 0}>
                              <span class="is-size-7 has-text-weight-bold">
                                ${formatPrice(totalCents())}
                              </span>
                            </Show>
                            <span class="icon is-small has-text-grey">
                              <span>{isExpanded() ? "▼" : "▶"}</span>
                            </span>
                          </div>
                        </div>

                        {/* Expanded session: show orders */}
                        <Show when={isExpanded()}>
                          <SessionOrderList session={session} />
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </Show>
            </Show>
          </Show>
        </div>
      </Show>
    </Card>
  );
}

// ══════════════════════════════════════════════════════════════════
// Main Orders page
// ══════════════════════════════════════════════════════════════════

export default function Orders() {
  const [restaurants] = createResource(fetchRestaurants);

  // ── Active sessions across all restaurants ──────────────────────
  const [activeSessions, setActiveSessions] = createSignal<
    { restaurant: Restaurant; session: OrderSession }[]
  >([]);
  const [activeLoading, setActiveLoading] = createSignal(false);
  const [activeLoaded, setActiveLoaded] = createSignal(false);
  const [refreshKey, setRefreshKey] = createSignal(0);

  /**
   * Fetch the active session for every restaurant in parallel,
   * collecting only those that have an Open session.
   */
  const loadActiveSessions = async (restaurantList: Restaurant[]) => {
    setActiveLoading(true);

    const results = await Promise.all(
      restaurantList.map(async (r) => {
        const session = await fetchActiveSession(r.id);
        return session ? { restaurant: r, session } : null;
      }),
    );

    setActiveSessions(
      results.filter(
        (r): r is { restaurant: Restaurant; session: OrderSession } =>
          r !== null,
      ),
    );
    setActiveLoading(false);
    setActiveLoaded(true);
  };

  // When restaurants finish loading, fetch all active sessions
  createMemo(() => {
    const list = restaurants();
    if (list && list.length > 0) {
      loadActiveSessions(list);
    }
  });

  const handleActiveSessionChanged = async () => {
    // Re-fetch active sessions across all restaurants
    const list = restaurants();
    if (list) {
      await loadActiveSessions(list);
    }
    setRefreshKey((k) => k + 1);
  };

  return (
    <section class="section">
      <div class="container">
        <div class="mb-5">
          <h1 class="title">📋 Orders</h1>
          <p class="subtitle">
            Active order sessions and history by restaurant.
          </p>
        </div>

        {/* Global order error */}
        <Show when={orderError()}>
          <div class="notification is-danger is-light mb-4">
            <button class="delete" onClick={clearOrderError} />
            {orderError()}
          </div>
        </Show>

        {/* Loading restaurants */}
        <Show when={restaurants.loading}>
          <div class="has-text-centered py-6">
            <progress class="progress is-primary is-small" max="100" />
            <p class="has-text-grey mt-2">Loading…</p>
          </div>
        </Show>

        {/* Error loading restaurants */}
        <Show when={restaurants.error}>
          <div class="notification is-danger is-light">
            <p>
              <strong>Error:</strong>{" "}
              {(restaurants.error as Error)?.message ??
                "Failed to load restaurants"}
            </p>
          </div>
        </Show>

        {/* No restaurants */}
        <Show
          when={
            !restaurants.loading &&
            !restaurants.error &&
            (restaurants() ?? []).length === 0
          }
        >
          <div class="notification is-info is-light has-text-centered">
            <p class="is-size-4 mb-2">🍽️</p>
            <p>No restaurants found. Create a restaurant first!</p>
            <A href="/restaurants" class="button is-primary is-small mt-3">
              Go to Restaurants
            </A>
          </div>
        </Show>

        {/* ════════════════════════════════════════════════════════
            Section 1 — Active (Open) sessions
            ════════════════════════════════════════════════════════ */}
        <Show when={(restaurants() ?? []).length > 0}>
          <div class="mb-6">
            <h2 class="title is-4">
              🟢 Active Sessions
            </h2>

            {/* Loading active sessions */}
            <Show when={activeLoading() && !activeLoaded()}>
              <div class="has-text-centered py-4">
                <progress class="progress is-success is-small" max="100" />
                <p class="has-text-grey mt-2 is-size-7">
                  Checking for active sessions…
                </p>
              </div>
            </Show>

            {/* No active sessions */}
            <Show when={activeLoaded() && activeSessions().length === 0}>
              <div class="notification is-light has-text-centered">
                <p class="is-size-4 mb-2">😴</p>
                <p class="has-text-grey">
                  No active order sessions right now. Start one from a{" "}
                  <A href="/restaurants">restaurant page</A>!
                </p>
              </div>
            </Show>

            {/* Active session cards */}
            <Show when={activeSessions().length > 0}>
              <For each={activeSessions()}>
                {(entry) => (
                  <ActiveSessionCard
                    session={entry.session}
                    restaurant={entry.restaurant}
                    onChanged={handleActiveSessionChanged}
                  />
                )}
              </For>
            </Show>
          </div>

          {/* ════════════════════════════════════════════════════════
              Section 2 — History per restaurant
              ════════════════════════════════════════════════════════ */}
          <div>
            <h2 class="title is-4">📁 History by Restaurant</h2>
            <For each={restaurants()!}>
              {(restaurant) => (
                <RestaurantHistory restaurant={restaurant} />
              )}
            </For>
          </div>
        </Show>
      </div>
    </section>
  );
}