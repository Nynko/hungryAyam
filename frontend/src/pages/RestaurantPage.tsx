import { createSignal, createEffect, createResource, createMemo, Show, For, onMount, onCleanup } from "solid-js";
import { A, useParams } from "@solidjs/router";
import type { Restaurant } from "@bindings/Restaurant";
import type { Menu } from "@bindings/Menu";
import type { Order } from "@bindings/Order";
import type { Offer } from "@bindings/Offer";
import type { ApiResponse } from "@bindings/ApiResponse";
import MenuView from "@/components/MenuView";
import OrderableMenuSectionView from "@/components/OrderableMenuSectionView";
import { Card } from "@/components/Card";
import CartPanel from "@/components/CartPanel";
import ActiveSessionBanner from "@/components/ActiveSessionBanner";
import CreateSessionModal from "@/components/CreateSessionModal";
import OfferBanner from "@/components/OfferBanner";
import OfferSlotPicker from "@/components/OfferSlotPicker";
import OffersManager from "@/components/OffersManager";
import RestaurantSettingsPanel from "@/components/RestaurantSettingsPanel";
import AuthPanel from "@/components/AuthPanel";
import { isAuthenticated, isEditor, isAdmin } from "@/stores/authStore";
import {
  fetchOpenSessions,
  getOpenSessions,
  getActiveSession,
  fetchMyOrdersInOpenSessions,
  deleteOrder,
  moveOrderToSession,
  createSession,
  sessionLoading,
  orderLoading,
  getCartCount,
  formatPrice,
  groupOrderItems,
  orderError,
  clearOrderError,
} from "@/stores/orderStore";
import { getOfferCartCount, fetchActiveOffers } from "@/stores/offerStore";
import { showConfirm } from "@/stores/confirmStore";
import { availabilityStatus } from "@/lib/availability";
import { isImageSrc } from "@/lib/imageUrl";

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

  // ── Open sessions ───────────────────────────────────────────────
  const [sessionVersion, setSessionVersion] = createSignal(0);

  // ── Active offers (for menu filtering) ──────────────────────────
  const [activeOffers, setActiveOffers] = createSignal<Offer[]>([]);

  /**
   * Set of menu IDs that are "owned" by an active offer AND are non-permanent.
   * These menus should NOT be shown as standalone menus in ordering mode —
   * they are presented as part of the offer instead.
   */
  const offerOwnedMenuIds = createMemo(() => {
    const allMenus = menus() ?? [];
    const menuMap = new Map(allMenus.map((m) => [m.id, m]));
    const ids = new Set<string>();

    for (const offer of activeOffers()) {
      if (offer.menu_id) {
        const menu = menuMap.get(offer.menu_id);
        // Only hide from standalone display if the menu is non-permanent
        if (menu && !menu.permanent) {
          ids.add(offer.menu_id);
        }
      }
    }

    return ids;
  });

  onMount(async () => {
    await fetchOpenSessions(params.id);
    setSessionVersion((v) => v + 1);
    refreshMyOrders();

    // Load active offers so we can filter offer-owned menus
    const offers = await fetchActiveOffers(params.id);
    setActiveOffers(offers);
  });

  const openSessions = createMemo(() => {
    // Re-read whenever sessionVersion changes (after transitions)
    sessionVersion();
    return getOpenSessions(params.id);
  });

  // For backward compat with ActiveSessionBanner (shows the first open session)
  const activeSession = createMemo(() => openSessions()[0] ?? null);

  const refreshSession = async () => {
    await fetchOpenSessions(params.id);
    setSessionVersion((v) => v + 1);
    // Also refresh my orders when session changes
    refreshMyOrders();
  };

  // ── My orders in active session ─────────────────────────────────
  const [myOrders, setMyOrders] = createSignal<Order[]>([]);
  const [myOrdersLoading, setMyOrdersLoading] = createSignal(false);
  const [myOrdersVersion, setMyOrdersVersion] = createSignal(0);

  const refreshMyOrders = async () => {
    if (!isAuthenticated()) {
      setMyOrders([]);
      return;
    }
    setMyOrdersLoading(true);
    try {
      const orders = await fetchMyOrdersInOpenSessions(params.id);
      setMyOrders(orders);
    } finally {
      setMyOrdersLoading(false);
    }
    setMyOrdersVersion((v) => v + 1);
  };



  // ── Restaurant availability ─────────────────────────────────────
  // Re-check every 60s so the UI updates when a time window opens/closes
  const [availTick, setAvailTick] = createSignal(0);
  const availInterval = setInterval(() => setAvailTick((t) => t + 1), 60_000);
  onCleanup(() => clearInterval(availInterval));

  const restaurantAvailability = createMemo(() => {
    availTick(); // subscribe to tick for periodic re-evaluation
    const r = restaurant();
    if (!r) return { available: true, reason: "" };
    return availabilityStatus(r.availability_rule);
  });

  // ── Ordering mode ───────────────────────────────────────────────
  // Starts false; set once on first load based on availability + menus.
  // Non-editors are auto-started in ordering mode when:
  //   - the restaurant is currently available, AND
  //   - at least one active non-permanent menu has an available item.
  const [orderingMode, setOrderingMode] = createSignal(false);
  const [orderingModeInitialized, setOrderingModeInitialized] = createSignal(false);

  const hasOrderableMenus = (m: Menu[]) =>
    m.every(
      (menu) =>
        menu.is_active &&
        (menu.permanent ||
          menu.sections.some((section) => section.items.some((item) => item.is_available))
        )
    );

  createEffect(() => {
    const r = restaurant();
    const m = menus();
    if (!orderingModeInitialized() && r && m) {
      setOrderingMode(!isEditor() && restaurantAvailability().available && hasOrderableMenus(m));
      setOrderingModeInitialized(true);
    }
  });
  const [showCreateSession, setShowCreateSession] = createSignal(false);

  // ── Offer composer state ────────────────────────────────────────
  const [composingOffer, setComposingOffer] = createSignal<Offer | null>(null);

  const cartCount = createMemo(() => getCartCount(params.id) + getOfferCartCount(params.id));

  // ── Menu helpers ────────────────────────────────────────────────

  /**
   * Active menus, excluding non-permanent menus that are linked to an active
   * offer (those are displayed as offer cards, not standalone menus).
   */
  const activeMenus = () =>
    (menus() ?? [])
      .filter((m) => m.is_active && !offerOwnedMenuIds().has(m.id))
      .sort((a, b) => a.name.localeCompare(b.name));

  /** All active menus (unfiltered), used in non-ordering mode where we show everything. */
  const allActiveMenus = () =>
    (menus() ?? []).filter((m) => m.is_active).sort((a, b) => a.name.localeCompare(b.name));

  const inactiveMenus = () =>
    (menus() ?? []).filter((m) => !m.is_active).sort((a, b) => a.name.localeCompare(b.name));

  const [showInactive, setShowInactive] = createSignal(false);

  // Sessions that can still accept new orders
  const orderableSessions = createMemo(() =>
    openSessions().filter((s) => s.status === "Open"),
  );

  // Can place orders if restaurant is available and at least one open session exists
  // (or none — backend will auto-create if configured to do so)
  const canOrder = createMemo(() => {
    if (!restaurantAvailability().available) return false;
    // If there are non-terminal sessions but none are Open, ordering is blocked
    const nonTerminal = openSessions();
    if (nonTerminal.length > 0 && orderableSessions().length === 0) return false;
    return true;
  });

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
                  <div class="card-image" style={{ display: "flex", "justify-content": "center", "background-color": "var(--bulma-scheme-main-bis)" }}>
                    <Show
                      when={isImageSrc(r().image_url!)}
                      fallback={
                        <span style={{ "font-size": "4rem", "line-height": "1", padding: "1.5rem" }}>
                          {r().image_url}
                        </span>
                      }
                    >
                      <img
                        src={r().image_url!}
                        alt={r().name}
                        style={{ "max-height": "120px", "object-fit": "contain" }}
                      />
                    </Show>
                  </div>
                </Show>
                <div class="card-content">
                  <div class="is-flex is-justify-content-space-between is-align-items-center is-flex-wrap-wrap" style={{ gap: "0.5rem" }}>
                    <div>
                      <h1 class="title is-3 mb-1">
                        <Show when={!r().image_url}>
                          <span class="mr-2">🍽️</span>
                        </Show>
                        {r().name}
                        <Show when={!restaurantAvailability().available}>
                          <span class="tag is-warning ml-2" style={{ "vertical-align": "middle", "font-size": "0.65em" }}>
                            Unavailable
                          </span>
                        </Show>
                      </h1>
                      <Show when={r().address || r().phone_number}>
                        <p class="has-text-grey is-size-7">
                          <Show when={r().address}>
                            <span>📍 {r().address}</span>
                          </Show>
                          <Show when={r().address && r().phone_number}>
                            <span class="mx-1">·</span>
                          </Show>
                          <Show when={r().phone_number}>
                            <a href={`tel:${r().phone_number}`} class="has-text-grey">📞 {r().phone_number}</a>
                          </Show>
                        </p>
                      </Show>
                    </div>

                    {/* Order / Session actions */}
                    <div class="buttons">
                      <button
                        class={`button ${orderingMode() ? "is-primary" : "is-primary is-outlined"}`}
                        disabled={!restaurantAvailability().available && !isEditor()}
                        onClick={() => setOrderingMode(!orderingMode())}
                      >
                        <span class="icon is-small">
                          <span>🛒</span>
                        </span>
                        <span>
                          {orderingMode() ? "Exit Ordering" : "Start Ordering"}
                        </span>
                        <Show when={cartCount() > 0}>
                          <span class="tag is-primary is-light ml-2">
                            {cartCount()}
                          </span>
                        </Show>
                      </button>

                      <Show when={isEditor()}>
                        <button
                          class="button is-info is-outlined"
                          onClick={() => setShowCreateSession(true)}
                          disabled={sessionLoading()}
                        >
                          <span class="icon is-small">
                            <span>📋</span>
                          </span>
                          <span>New Session</span>
                        </button>
                      </Show>
                    </div>
                  </div>
                </div>
              </Card>

              {/* ── Restaurant unavailable banner ─────────────── */}
              <Show when={!restaurantAvailability().available}>
                <div class="notification is-warning mb-4">
                  <div class="is-flex is-align-items-center" style={{ gap: "0.75rem" }}>
                    <span style={{ "font-size": "1.5rem" }}>🚫</span>
                    <div>
                      <p class="has-text-weight-bold">
                        This restaurant is currently unavailable
                      </p>
                      <Show when={restaurantAvailability().reason}>
                        <p class="is-size-7 mt-1">
                          {restaurantAvailability().reason}
                        </p>
                      </Show>
                    </div>
                  </div>
                </div>
              </Show>

              {/* ── Session banners (one per open session) ─────── */}
              <For each={openSessions()}>
                {(session) => (
                  <ActiveSessionBanner
                    session={session}
                    restaurantId={r().id}
                    onSessionChanged={refreshSession}
                  />
                )}
              </For>

              {/* ── My Orders ──────────────────────────────────── */}
              <Show when={isAuthenticated() && (myOrders().length > 0 || myOrdersLoading())}>
                <Card class="mb-4">
                  <div class="card-content">
                    <h3 class="title is-5 mb-3">🧾 My Orders</h3>

                    <Show when={myOrdersLoading() && myOrders().length === 0}>
                      <div class="has-text-centered py-3">
                        <progress class="progress is-primary is-small" max="100" />
                        <p class="has-text-grey is-size-7 mt-1">Loading your orders…</p>
                      </div>
                    </Show>

                    <Show when={myOrders().length > 0}>
                      <For each={myOrders()}>
                        {(order) => {
                          const totalItems = () => order.items.length;
                          const [deleting, setDeleting] = createSignal(false);
                          const [showMove, setShowMove] = createSignal(false);
                          const [moveSessionId, setMoveSessionId] = createSignal<string | null>(null);
                          const [moving, setMoving] = createSignal(false);
                          const [showNewMoveSlot, setShowNewMoveSlot] = createSignal(false);
                          const [newMoveSlotEnd, setNewMoveSlotEnd] = createSignal(() => {
                            const d = new Date();
                            d.setHours(d.getHours() + 1, 0, 0, 0);
                            return d.toISOString().slice(0, 16);
                          });

                          // The session this order currently belongs to
                          const orderSession = () =>
                            openSessions().find((s) => s.id === order.session_id) ?? null;
                          const sessionIsOpen = () => orderSession()?.status === "Open";

                          // Other sessions the user could move to
                          const otherSessions = () =>
                            openSessions().filter((s) => s.id !== order.session_id);

                          const handleDelete = async () => {
                            const confirmed = await showConfirm({
                              title: "Delete order?",
                              message: `Remove this order (${totalItems()} item${totalItems() !== 1 ? "s" : ""}, €${formatPrice(order.total_price_cents)})?`,
                              confirmText: "Delete",
                              danger: true,
                            });
                            if (!confirmed) return;

                            setDeleting(true);
                            const ok = await deleteOrder(order.id);
                            setDeleting(false);

                            if (ok) {
                              await refreshMyOrders();
                              await refreshSession();
                            }
                          };

                          const handleMove = async () => {
                            const targetId = moveSessionId();
                            if (!targetId) return;
                            setMoving(true);
                            const moved = await moveOrderToSession(order.id, targetId);
                            setMoving(false);
                            if (moved) {
                              setShowMove(false);
                              await refreshMyOrders();
                              await refreshSession();
                            }
                          };

                          const handleMoveToNewSlot = async () => {
                            const endStr = newMoveSlotEnd();
                            if (!endStr || new Date(endStr).getTime() <= Date.now()) return;
                            setMoving(true);
                            try {
                              const now = new Date();
                              const newSession = await createSession({
                                restaurant_id: r().id,
                                start_date: now.toISOString(),
                                end_date: new Date(endStr).toISOString(),
                                pickup_time: null,
                                allow_late: false,
                              });
                              if (newSession) {
                                await refreshSession();
                                const moved = await moveOrderToSession(order.id, newSession.id);
                                if (moved) {
                                  setShowMove(false);
                                  await refreshMyOrders();
                                  await refreshSession();
                                }
                              }
                            } finally {
                              setMoving(false);
                            }
                          };

                          return (
                            <div class="box p-3 mb-2">
                              <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
                                <div>
                                  <span class="has-text-weight-semibold">
                                    {totalItems()} item{totalItems() !== 1 ? "s" : ""}
                                  </span>
                                  <Show when={orderSession()}>
                                    {(s) => {
                                      const displayTime = s().pickup_time ?? s().end_date;
                                      return (
                                        <span class="tag is-info is-light is-size-7 ml-2">
                                          {new Date(displayTime).toLocaleString(undefined, { timeStyle: "short", dateStyle: "short" })}
                                        </span>
                                      );
                                    }}
                                  </Show>
                                </div>
                                <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
                                  <span class="has-text-weight-bold">
                                    €{formatPrice(order.total_price_cents)}
                                  </span>
                                  <Show when={sessionIsOpen()}>
                                    <button
                                      class="button is-small is-info is-outlined"
                                      classList={{ "is-loading": moving() }}
                                      disabled={deleting() || moving() || orderLoading()}
                                      onClick={() => setShowMove(!showMove())}
                                      title="Change pickup time"
                                    >
                                      <span class="icon is-small"><span>🕐</span></span>
                                    </button>
                                    <button
                                      class="button is-small is-danger is-outlined"
                                      classList={{ "is-loading": deleting() }}
                                      disabled={deleting() || moving() || orderLoading()}
                                      onClick={handleDelete}
                                      title="Delete this order"
                                    >
                                      <span class="icon is-small"><span>🗑️</span></span>
                                    </button>
                                  </Show>
                                </div>
                              </div>

                              {/* Move to session UI */}
                              <Show when={showMove()}>
                                <div
                                  class="p-2 mb-2"
                                  style={{ background: "var(--bulma-scheme-main-bis)", "border-radius": "6px" }}
                                >
                                  <p class="is-size-7 has-text-weight-semibold mb-2">Move to a different pickup time</p>

                                  {/* Existing other sessions */}
                                  <Show when={otherSessions().length > 0}>
                                    <div class="is-flex is-flex-direction-column mb-2" style={{ gap: "0.3rem" }}>
                                      <For each={otherSessions()}>
                                        {(session) => {
                                          const displayTime = session.pickup_time ?? session.end_date;
                                          return (
                                            <label
                                              class="is-flex is-align-items-center"
                                              style={{ gap: "0.4rem", cursor: "pointer" }}
                                            >
                                              <input
                                                type="radio"
                                                name={`move-session-${order.id}`}
                                                value={session.id}
                                                checked={moveSessionId() === session.id}
                                                onChange={() => {
                                                  setMoveSessionId(session.id);
                                                  setShowNewMoveSlot(false);
                                                }}
                                              />
                                              <span class="is-size-7">
                                                {new Date(displayTime).toLocaleString(undefined, { timeStyle: "short", dateStyle: "short" })}
                                              </span>
                                            </label>
                                          );
                                        }}
                                      </For>
                                    </div>
                                  </Show>

                                  {/* New slot option */}
                                  <Show
                                    when={showNewMoveSlot()}
                                    fallback={
                                      <button
                                        class="button is-ghost is-small px-0 has-text-grey mb-2"
                                        style={{ height: "auto", "font-size": "0.72rem" }}
                                        onClick={() => { setShowNewMoveSlot(true); setMoveSessionId(null); }}
                                      >
                                        + New pickup time
                                      </button>
                                    }
                                  >
                                    <div class="field has-addons mb-2">
                                      <div class="control is-expanded">
                                        <input
                                          class="input is-small"
                                          type="datetime-local"
                                          value={newMoveSlotEnd()}
                                          onInput={(e) => setNewMoveSlotEnd(e.currentTarget.value)}
                                          disabled={moving()}
                                        />
                                      </div>
                                      <div class="control">
                                        <button
                                          class="button is-primary is-small"
                                          classList={{ "is-loading": moving() }}
                                          disabled={moving()}
                                          onClick={handleMoveToNewSlot}
                                        >
                                          Move
                                        </button>
                                      </div>
                                    </div>
                                  </Show>

                                  <div class="is-flex" style={{ gap: "0.5rem" }}>
                                    <Show when={moveSessionId()}>
                                      <button
                                        class="button is-primary is-small"
                                        classList={{ "is-loading": moving() }}
                                        disabled={moving() || !moveSessionId()}
                                        onClick={handleMove}
                                      >
                                        Move
                                      </button>
                                    </Show>
                                    <button
                                      class="button is-light is-small"
                                      disabled={moving()}
                                      onClick={() => { setShowMove(false); setMoveSessionId(null); setShowNewMoveSlot(false); }}
                                    >
                                      Cancel
                                    </button>
                                  </div>
                                </div>
                              </Show>

                              <div>
                                <For each={groupOrderItems(order.items)}>
                                  {(group) => (
                                    <div class="is-size-7 mb-1">
                                      <div class="is-flex is-align-items-center" style={{ gap: "0.4rem" }}>
                                        <span class="has-text-grey">•</span>
                                        <span class="has-text-weight-medium">
                                          {group.quantity > 1 ? `${group.quantity}× ` : ""}{group.itemName}
                                        </span>
                                        <span class="has-text-grey">
                                          €{formatPrice(group.itemPriceCents * group.quantity)}
                                        </span>
                                      </div>
                                      <Show when={group.notes.length > 0}>
                                        <div class="ml-4">
                                          <For each={group.notes}>
                                            {(note) => (
                                              <p class="has-text-grey is-italic">📝 {note}</p>
                                            )}
                                          </For>
                                        </div>
                                      </Show>
                                    </div>
                                  )}
                                </For>
                              </div>
                              <p class="has-text-grey is-size-7 mt-2">
                                Placed {new Date(order.created_at).toLocaleString(undefined, { timeStyle: "short", dateStyle: "short" })}
                              </p>
                            </div>
                          );
                        }}
                      </For>
                    </Show>
                  </div>
                </Card>
              </Show>

              {/* ── Global order error ─────────────────────────── */}
              <Show when={orderError()}>
                <div class="notification is-danger is-light mb-4">
                  <button class="delete" onClick={clearOrderError} />
                  {orderError()}
                </div>
              </Show>

              {/* ── Ordering mode: two-column layout ───────────── */}
              <Show when={orderingMode()}>
                <div class="columns is-variable is-6">
                  {/* ── Left: orderable menu ───────────────────── */}
                  <div class="column is-8">
                    {/* ── Offer Slot Picker (modal-like overlay) ── */}
                    <Show when={composingOffer()}>
                      {(offer) => (
                        <div class="mb-5">
                          <OfferSlotPicker
                            offer={offer()}
                            restaurantId={r().id}
                            onClose={() => setComposingOffer(null)}
                            onAdded={() => setComposingOffer(null)}
                          />
                        </div>
                      )}
                    </Show>

                    {/* ── Offer Banner (when not composing) ──────── */}
                    <Show when={!composingOffer()}>
                      <OfferBanner
                        restaurantId={r().id}
                        onComposeOffer={(offer) => setComposingOffer(offer)}
                      />
                    </Show>

                    <div class="mb-4">
                      <h2 class="title is-4 mb-2">📋 Pick your items</h2>
                      <Show when={!canOrder()}>
                        <div class="notification is-warning is-light">
                          <p class="is-size-7">
                            The current session is not accepting orders. You
                            can still browse, but ordering is disabled.
                          </p>
                        </div>
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

                    {/* Active menus — orderable */}
                    <Show when={activeMenus().length > 0}>
                      <For each={activeMenus()}>
                        {(menu) => (
                          <Card class="mb-5">
                            <header class="card-header">
                              <div class="card-header-title is-flex is-justify-content-space-between is-align-items-center">
                                <div>
                                  <span class="is-size-5 has-text-weight-bold">
                                    {menu.name}
                                  </span>
                                  <Show when={menu.permanent}>
                                    <span
                                      class="tag is-primary is-light ml-2"
                                      style={{ "vertical-align": "middle" }}
                                    >
                                      Permanent
                                    </span>
                                  </Show>
                                </div>
                              </div>
                            </header>
                            <div class="card-content">
                              <Show when={menu.description}>
                                <p class="has-text-grey mb-4">
                                  {menu.description}
                                </p>
                              </Show>
                              <Show
                                when={
                                  menu.sections.filter((s) => s.is_active).length > 0
                                }
                                fallback={
                                  <div class="has-text-centered py-4">
                                    <p class="has-text-grey-light is-italic">
                                      This menu has no active sections yet.
                                    </p>
                                  </div>
                                }
                              >
                                <For
                                  each={menu.sections
                                    .filter((s) => s.is_active)
                                    .sort((a, b) => a.position - b.position)}
                                >
                                  {(section) => (
                                    <OrderableMenuSectionView
                                      section={section}
                                      restaurantId={r().id}
                                      depth={0}
                                      hideUnavailable={!menu.permanent && !isEditor()}
                                    />
                                  )}
                                </For>
                              </Show>
                            </div>
                          </Card>
                        )}
                      </For>
                    </Show>
                  </div>

                  {/* ── Right: cart panel (sticky) ──────────────── */}
                  <div class="column is-4">
                    <div style={{ position: "sticky", top: "1rem" }}>
                      <CartPanel
                        restaurantId={r().id}
                        openSessions={orderableSessions()}
                        onOrderPlaced={() => {
                          refreshSession();
                          refreshMyOrders();
                        }}
                      />
                    </div>
                  </div>
                </div>
              </Show>

              {/* ── Normal (non-ordering) mode: standard menus ─── */}
              <Show when={!orderingMode()}>
                {/* ── Restaurant Settings (editor only) ────────── */}
                <Show when={isEditor()}>
                  <RestaurantSettingsPanel restaurantId={r().id} restaurant={r()} />
                </Show>

                {/* ── Offers Manager (editor only) ─────────────── */}
                <Show when={isEditor() && !menus.loading}>
                  <OffersManager
                    restaurantId={r().id}
                    menus={menus() ?? []}
                  />
                </Show>

                <div class="mb-4">
                  <div class="is-flex is-justify-content-space-between is-align-items-center mb-4">
                    <h2 class="title is-4 mb-0">📋 Menus</h2>
                    <Show when={isEditor()}>
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

                  {/* Active menus (unfiltered in non-ordering view) */}
                  <Show when={allActiveMenus().length > 0}>
                    <For each={allActiveMenus()}>
                      {(menu) => (
                        <div class="mb-5">
                          <MenuView menu={menu} hideUnavailable={!menu.permanent && !isEditor()} />
                          <Show when={isEditor() || (!menu.permanent && isAuthenticated())}>
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

                  {/* No active menus but has inactive ones (editor hint) */}
                  <Show
                    when={
                      isEditor() &&
                      allActiveMenus().length === 0 &&
                      inactiveMenus().length > 0 &&
                      !menus.loading
                    }
                  >
                    <div class="notification is-warning is-light has-text-centered mb-4">
                      <p>
                        No active menus. There{" "}
                        {inactiveMenus().length === 1 ? "is" : "are"}{" "}
                        {inactiveMenus().length} inactive menu
                        {inactiveMenus().length !== 1 ? "s" : ""}.
                      </p>
                    </div>
                  </Show>

                  {/* Toggle inactive menus (editor only) */}
                  <Show when={isEditor() && inactiveMenus().length > 0}>
                    <div class="has-text-centered mt-4 mb-4">
                      <button
                        class="button is-small is-light"
                        onClick={() => setShowInactive(!showInactive())}
                      >
                        <span class="mr-1">
                          {showInactive() ? "▼" : "▶"}
                        </span>
                        {showInactive() ? "Hide" : "Show"} inactive menus (
                        {inactiveMenus().length})
                      </button>
                    </div>

                    <Show when={showInactive()}>
                      <For each={inactiveMenus()}>
                        {(menu) => (
                          <div style={{ opacity: "0.7" }} class="mb-5">
                            <MenuView menu={menu} hideUnavailable={!menu.permanent && !isEditor()} />
                            <Show when={isEditor() || (!menu.permanent && isAuthenticated())}>
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

                {/* ── Floating cart pill (non-ordering mode) ────── */}
                <Show when={cartCount() > 0}>
                  <div
                    style={{
                      position: "fixed",
                      bottom: "1.5rem",
                      right: "1.5rem",
                      "z-index": "30",
                    }}
                  >
                    <button
                      class="button is-primary is-medium is-rounded"
                      style={{ "box-shadow": "0 4px 14px rgba(0,0,0,0.25)" }}
                      onClick={() => setOrderingMode(true)}
                    >
                      <span class="icon">
                        <span>🛒</span>
                      </span>
                      <span>
                        View Cart ({cartCount()})
                      </span>
                    </button>
                  </div>
                </Show>
              </Show>

              {/* ── Create Session Modal ───────────────────────── */}
              <CreateSessionModal
                isOpen={showCreateSession()}
                restaurantId={r().id}
                onClose={() => setShowCreateSession(false)}
                onCreated={refreshSession}
              />
            </>
          )}
        </Show>
      </div>
    </section>
  );
}
