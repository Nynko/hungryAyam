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
import { isAuthenticated, isAdmin } from "@/stores/authStore";
import {
  fetchSessionsForRestaurant,
  fetchOpenSessions,
  fetchSession,
  sessionStatusColor,
  formatPrice,
  groupOrderItems,
  orderError,
  clearOrderError,
  updateSession,
} from "@/stores/orderStore";

// ── Types ─────────────────────────────────────────────────────────

interface RegularItemSummary { item_name: string; quantity: number; note: string | null; }
interface OfferItemCount { name: string; qty: number; }
interface OfferSlotSummary { label: string; items: OfferItemCount[]; }
interface OfferGroupSummary { offer_title: string; count: number; slots: OfferSlotSummary[]; }
interface SessionOrderSummary { regular_items: RegularItemSummary[]; offer_groups: OfferGroupSummary[]; }

// ── Data fetchers ─────────────────────────────────────────────────

async function fetchSessionSummary(sessionId: string): Promise<SessionOrderSummary> {
  const res = await fetch(`/api/order-sessions/${sessionId}/summary`);
  if (!res.ok) throw new Error(`Failed to load summary (${res.status})`);
  const json: ApiResponse<SessionOrderSummary> = await res.json();
  if (!json.success || json.data == null) throw new Error(json.error ?? "Unexpected response");
  return json.data;
}

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

  // Refetch summary whenever order count changes (new order placed / removed)
  const [summary] = createResource(
    () => `${props.session.id}:${orders().length}`,
    () => fetchSessionSummary(props.session.id),
  );

  const regularItems = () => summary()?.regular_items ?? [];
  const offerGroups = () => summary()?.offer_groups ?? [];

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
            Total: €{formatPrice(totalRevenue())}
          </span>
        </div>

        {/* ── Aggregated command for the restaurant ────────────── */}
        <Show when={regularItems().length > 0 || offerGroups().length > 0}>
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

            {/* Regular items */}
            <Show when={regularItems().length > 0}>
              <ul class="ml-4 mb-2" style={{ "list-style": "disc" }}>
                <For each={regularItems()}>
                  {(item) => (
                    <li>
                      <span class="has-text-weight-bold">{item.quantity}x</span>
                      {" "}
                      <span>{item.item_name}</span>
                      <Show when={item.note}>
                        <span class="has-text-grey is-italic is-size-7 ml-1">
                          ({item.note})
                        </span>
                      </Show>
                    </li>
                  )}
                </For>
              </ul>
            </Show>

            {/* Offer groups */}
            <Show when={offerGroups().length > 0}>
              <Show when={regularItems().length > 0}>
                <hr class="my-2" style={{ "border-color": "hsl(204, 86%, 80%)" }} />
              </Show>
              <For each={offerGroups()}>
                {(group) => (
                  <div class="mb-3">
                    <p class="has-text-weight-bold is-size-7 mb-1">
                      🍽️ {group.offer_title} ×{group.count}
                    </p>
                    <For each={group.slots}>
                      {(slot) => (
                        <div class="ml-3 mb-1">
                          <span class="has-text-weight-semibold is-size-7">{slot.label}:</span>
                          <For each={slot.items}>
                            {(item) => (
                              <div class="ml-4 is-size-7">
                                {item.qty}x {item.name}
                              </div>
                            )}
                          </For>
                        </div>
                      )}
                    </For>
                  </div>
                )}
              </For>
            </Show>
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
                                {group.quantity}x {group.itemName}
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
                      €{formatPrice(order.total_price_cents)}
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
          <div>
            <A
              href={`/restaurants/${props.restaurant.id}`}
              class="has-text-weight-bold is-size-5"
            >
              {props.restaurant.name}
            </A>
            <Show when={props.restaurant.address || props.restaurant.phone_number}>
              <p class="has-text-grey is-size-7">
                <Show when={props.restaurant.address}>
                  <span>📍 {props.restaurant.address}</span>
                </Show>
                <Show when={props.restaurant.address && props.restaurant.phone_number}>
                  <span class="mx-1">·</span>
                </Show>
                <Show when={props.restaurant.phone_number}>
                  <a href={`tel:${props.restaurant.phone_number}`} class="has-text-grey">📞 {props.restaurant.phone_number}</a>
                </Show>
              </p>
            </Show>
          </div>
        </div>

        {/* Session banner with admin controls */}
        <ActiveSessionBanner
          session={props.session}
          restaurantId={props.restaurant.id}
          smsPhoneNumber={props.restaurant.sms_phone_number}
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
      Requested: 0,
      SmsSent: 0,
      Confirmed: 0,
      Finished: 0,
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
                        { key: "Open" as StatusFilter, label: "🟢 Open" },
                        { key: "Closed" as StatusFilter, label: "🟡 Closed" },
                        { key: "Requested" as StatusFilter, label: "📨 Requested" },
                        { key: "SmsSent" as StatusFilter, label: "📱 SMS Sent" },
                        { key: "Confirmed" as StatusFilter, label: "✅ Confirmed" },
                        { key: "Finished" as StatusFilter, label: "🏁 Finished" },
                        { key: "Cancelled" as StatusFilter, label: "❌ Cancelled" },
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

                    // ── Inline pickup time edit ────────────────
                    const [editingPickup, setEditingPickup] = createSignal(false);
                    const [pickupInput, setPickupInput] = createSignal("");
                    const [pickupSaving, setPickupSaving] = createSignal(false);
                    const [localPickupTime, setLocalPickupTime] = createSignal(
                      session.pickup_time ?? null,
                    );

                    const startEditPickup = (e: MouseEvent) => {
                      e.stopPropagation();
                      const pt = localPickupTime();
                      if (pt) {
                        const d = new Date(pt);
                        const pad = (n: number) => String(n).padStart(2, "0");
                        setPickupInput(
                          `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`,
                        );
                      } else {
                        setPickupInput("");
                      }
                      setEditingPickup(true);
                    };

                    const savePickup = async (e: MouseEvent) => {
                      e.stopPropagation();
                      setPickupSaving(true);
                      const val = pickupInput().trim();
                      const result = await updateSession({
                        id: session.id,
                        start_date: null,
                        end_date: null,
                        update_pickup_time: true,
                        pickup_time: val ? new Date(val).toISOString() : null,
                        allow_late: null,
                      });
                      setPickupSaving(false);
                      if (result) {
                        setLocalPickupTime(result.pickup_time ?? null);
                        setEditingPickup(false);
                      }
                    };

                    return (
                      <div
                        class="box mb-3 p-3"
                        style={{
                          "border-left": "4px solid",
                          "border-left-color":
                            session.status === "Open"
                              ? "hsl(141, 71%, 48%)"   // green
                              : session.status === "Closed"
                                ? "hsl(48, 100%, 67%)" // yellow
                                : session.status === "Requested"
                                  ? "hsl(204, 86%, 53%)" // blue
                                  : session.status === "SmsSent"
                                    ? "hsl(217, 71%, 53%)" // darker blue/link
                                    : session.status === "Confirmed"
                                      ? "hsl(171, 100%, 41%)" // teal/primary
                                      : session.status === "Finished"
                                        ? "hsl(0, 0%, 71%)"   // grey
                                        : "hsl(348, 100%, 61%)", // red (Cancelled)
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
                            <Show when={localPickupTime()}>
                              <span class="is-size-7 has-text-grey">
                                🕐 Pickup: {new Date(localPickupTime()!).toLocaleString(undefined, { timeStyle: "short", dateStyle: "short" })}
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
                                €{formatPrice(totalCents())}
                              </span>
                            </Show>
                            <span class="icon is-small has-text-grey">
                              <span>{isExpanded() ? "▼" : "▶"}</span>
                            </span>
                          </div>
                        </div>

                        {/* Expanded session: pickup edit + orders */}
                        <Show when={isExpanded()}>
                          {/* Pickup time row (admin only) */}
                          <Show when={isAdmin()}>
                            <div class="mt-3 mb-2 is-flex is-align-items-center is-flex-wrap-wrap" style={{ gap: "0.5rem" }}>
                              <span class="is-size-7"><strong>Pickup time:</strong></span>
                              <Show
                                when={editingPickup()}
                                fallback={
                                  <>
                                    <span class="is-size-7">
                                      <Show
                                        when={localPickupTime()}
                                        fallback={<span class="has-text-grey-light">not set</span>}
                                      >
                                        {(pt) => new Date(pt()).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" })}
                                      </Show>
                                    </span>
                                    <button
                                      class="button is-ghost is-small py-0 px-1"
                                      style={{ height: "auto", "min-height": "unset" }}
                                      title="Edit pickup time"
                                      onClick={startEditPickup}
                                    >
                                      ✏️
                                    </button>
                                  </>
                                }
                              >
                                <input
                                  class="input is-small"
                                  style={{ width: "13rem" }}
                                  type="datetime-local"
                                  value={pickupInput()}
                                  onInput={(e) => setPickupInput(e.currentTarget.value)}
                                  onClick={(e) => e.stopPropagation()}
                                />
                                <button
                                  class="button is-primary is-small"
                                  classList={{ "is-loading": pickupSaving() }}
                                  disabled={pickupSaving()}
                                  onClick={savePickup}
                                >
                                  Save
                                </button>
                                <button
                                  class="button is-small"
                                  disabled={pickupSaving()}
                                  onClick={(e) => { e.stopPropagation(); setEditingPickup(false); }}
                                >
                                  Cancel
                                </button>
                              </Show>
                            </div>
                          </Show>
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
   * Fetch all non-terminal sessions for every restaurant in parallel,
   * then load each session's full data (with orders) individually.
   */
  const loadActiveSessions = async (restaurantList: Restaurant[]) => {
    setActiveLoading(true);

    const pairs: { restaurant: Restaurant; session: OrderSession }[] = [];

    await Promise.all(
      restaurantList.map(async (r) => {
        const sessions = await fetchOpenSessions(r.id);
        const fullSessions = await Promise.all(
          sessions.map((s) => fetchSession(s.id)),
        );
        for (const full of fullSessions) {
          if (full) pairs.push({ restaurant: r, session: full });
        }
      }),
    );

    setActiveSessions(pairs);
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
              📋 Active Sessions
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