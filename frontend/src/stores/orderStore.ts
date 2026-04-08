import { createSignal } from "solid-js";
import { createStore, produce } from "solid-js/store";
import type { ApiResponse } from "@bindings/ApiResponse";
import type { Order } from "@bindings/Order";
import type { OrderItem } from "@bindings/OrderItem";
import type { OrderSession } from "@bindings/OrderSession";
import type { OrderSessionStatus } from "@bindings/OrderSessionStatus";
import type { CreateOrderSession } from "@bindings/CreateOrderSession";
import type { UpdateOrderSession } from "@bindings/UpdateOrderSession";
import type { CreateOrder } from "@bindings/CreateOrder";
import type { CreateOrderItem } from "@bindings/CreateOrderItem";
import type { MoveOrderToSessionRequest } from "@bindings/MoveOrderToSessionRequest";
import type { RestaurantOrderSettings } from "@bindings/RestaurantOrderSettings";
import type { UpdateOrderSettingsRequest } from "@bindings/UpdateOrderSettingsRequest";
import type { OrderSessionStatusResponse } from "@bindings/OrderSessionStatusResponse";
import type { OrderSummary } from "@bindings/OrderSummary";
import type { MenuSectionItem } from "@bindings/MenuSectionItem";

// ══════════════════════════════════════════════════════════════════
// Cart types
// ══════════════════════════════════════════════════════════════════

export interface CartItem {
  /** The menu section item (includes item details, price override, etc.) */
  sectionItem: MenuSectionItem;
  /** Optional notes for this item */
  notes: string | null;
  /** Unique key for this cart entry (for removal) */
  key: number;
}

// ══════════════════════════════════════════════════════════════════
// State
// ══════════════════════════════════════════════════════════════════

/** Cart items keyed by restaurant ID */
const [cartsByRestaurant, setCartsByRestaurant] = createStore<
  Record<string, CartItem[]>
>({});

/** Cached open sessions per restaurant (all Open status sessions) */
const [openSessionsByRestaurant, setOpenSessionsByRestaurant] = createSignal<
  Record<string, OrderSession[]>
>({});

/** Loading states */
const [orderLoading, setOrderLoading] = createSignal(false);
const [sessionLoading, setSessionLoading] = createSignal(false);

/** Error state */
const [orderError, setOrderError] = createSignal<string | null>(null);

/** Counter for unique cart item keys */
let nextCartKey = 1;

// ══════════════════════════════════════════════════════════════════
// Cart actions
// ══════════════════════════════════════════════════════════════════

/**
 * Get the cart items for a specific restaurant.
 */
function getCart(restaurantId: string): CartItem[] {
  return cartsByRestaurant[restaurantId] ?? [];
}

/**
 * Add an item to the cart for a restaurant.
 */
function addToCart(restaurantId: string, sectionItem: MenuSectionItem): void {
  const existing = cartsByRestaurant[restaurantId] ?? [];
  setCartsByRestaurant(restaurantId, [
    ...existing,
    { sectionItem, notes: null, key: nextCartKey++ },
  ]);
}

/**
 * Remove an item from the cart by its unique key.
 */
function removeFromCart(restaurantId: string, key: number): void {
  setCartsByRestaurant(restaurantId, (prev) =>
    (prev ?? []).filter((item) => item.key !== key),
  );
}

/**
 * Update the notes for a cart item.
 * Fine-grained store mutation — preserves object identity so <For> does not
 * recreate DOM elements (and inputs do not lose focus while typing).
 */
function updateCartItemNotes(
  restaurantId: string,
  key: number,
  notes: string | null,
): void {
  const cart = cartsByRestaurant[restaurantId];
  if (!cart) return;
  const idx = cart.findIndex((item) => item.key === key);
  if (idx !== -1) {
    setCartsByRestaurant(restaurantId, idx, "notes", notes);
  }
}

/**
 * Clear the entire cart for a restaurant.
 */
function clearCart(restaurantId: string): void {
  setCartsByRestaurant(produce((draft) => { delete draft[restaurantId]; }));
}

/**
 * Get the total price of items in the cart (in cents).
 * Uses price_override_cents if set, otherwise base_price_cents.
 */
function getCartTotal(restaurantId: string): number {
  const items = getCart(restaurantId);
  return items.reduce((sum, ci) => {
    const price =
      ci.sectionItem.price_override_cents ?? ci.sectionItem.item.base_price_cents;
    return sum + price;
  }, 0);
}

/**
 * Get the number of items in the cart for a restaurant.
 */
function getCartCount(restaurantId: string): number {
  return getCart(restaurantId).length;
}

// ══════════════════════════════════════════════════════════════════
// Price formatting
// ══════════════════════════════════════════════════════════════════

function formatPrice(cents: number): string {
  return (cents / 100).toFixed(2);
}

// ══════════════════════════════════════════════════════════════════
// Order item grouping helpers
// ══════════════════════════════════════════════════════════════════

/** A group of identical items within an order. */
export interface OrderItemGroup {
  itemId: string;
  itemName: string;
  itemPriceCents: number;
  quantity: number;
  /** Individual notes from each occurrence (only non-null ones). */
  notes: string[];
}

/**
 * Group an order's items by item_id, aggregating quantities and notes.
 *
 * Example output: [{ itemName: "Nasi Goreng", quantity: 2, ... }, ...]
 */
function groupOrderItems(items: OrderItem[]): OrderItemGroup[] {
  const map = new Map<string, OrderItemGroup>();

  for (const oi of items) {
    const existing = map.get(oi.item_id);
    if (existing) {
      existing.quantity += 1;
      if (oi.notes) existing.notes.push(oi.notes);
    } else {
      map.set(oi.item_id, {
        itemId: oi.item_id,
        itemName: oi.item_name,
        itemPriceCents: oi.item_price_cents,
        quantity: 1,
        notes: oi.notes ? [oi.notes] : [],
      });
    }
  }

  return Array.from(map.values());
}

/**
 * Produce a compact one-liner summary of an order's items.
 *
 * Example: "2 Nasi Goreng - 1 Ayam Bakar - 3 Es Teh"
 */
function summarizeOrderItems(items: OrderItem[]): string {
  return groupOrderItems(items)
    .map((g) => `${g.quantity} ${g.itemName}`)
    .join(" – ");
}

// ══════════════════════════════════════════════════════════════════
// API — Order Sessions
// ══════════════════════════════════════════════════════════════════

/**
 * Fetch all open (Open status) sessions for a restaurant.
 * Caches the result in `openSessionsByRestaurant`.
 */
async function fetchOpenSessions(
  restaurantId: string,
): Promise<OrderSession[]> {
  try {
    setSessionLoading(true);
    setOrderError(null);

    const res = await fetch(
      `/api/restaurants/${restaurantId}/order-sessions/open`,
    );
    if (!res.ok) {
      throw new Error(`Failed to fetch open sessions (${res.status})`);
    }

    const json: ApiResponse<OrderSession[]> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    const sessions = json.data;
    setOpenSessionsByRestaurant((prev) => ({
      ...prev,
      [restaurantId]: sessions,
    }));
    return sessions;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchOpenSessions failed:", msg);
    return [];
  } finally {
    setSessionLoading(false);
  }
}

/**
 * Fetch the currently active (Open) session for a restaurant.
 * @deprecated Use fetchOpenSessions instead. Kept for backward compatibility.
 */
async function fetchActiveSession(
  restaurantId: string,
): Promise<OrderSession | null> {
  const sessions = await fetchOpenSessions(restaurantId);
  return sessions[0] ?? null;
}

/**
 * Fetch all sessions for a restaurant (most recent first).
 */
async function fetchSessionsForRestaurant(
  restaurantId: string,
): Promise<OrderSession[]> {
  try {
    setSessionLoading(true);
    setOrderError(null);

    const res = await fetch(
      `/api/restaurants/${restaurantId}/order-sessions`,
    );
    if (!res.ok) {
      throw new Error(`Failed to fetch sessions (${res.status})`);
    }

    const json: ApiResponse<OrderSession[]> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchSessionsForRestaurant failed:", msg);
    return [];
  } finally {
    setSessionLoading(false);
  }
}

/**
 * Fetch a single order session by ID (with orders and items).
 */
async function fetchSession(sessionId: string): Promise<OrderSession | null> {
  try {
    setSessionLoading(true);
    setOrderError(null);

    const res = await fetch(`/api/order-sessions/${sessionId}`);
    if (!res.ok) {
      if (res.status === 404) return null;
      throw new Error(`Failed to fetch session (${res.status})`);
    }

    const json: ApiResponse<OrderSession> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchSession failed:", msg);
    return null;
  } finally {
    setSessionLoading(false);
  }
}

/**
 * Create a new order session.
 */
async function createSession(
  request: CreateOrderSession,
): Promise<OrderSession | null> {
  try {
    setSessionLoading(true);
    setOrderError(null);

    const res = await fetch("/api/order-sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<OrderSession> = await res.json();
    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Create session failed (${res.status})`);
    }

    // Refresh open sessions cache
    await fetchOpenSessions(request.restaurant_id);

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] createSession failed:", msg);
    return null;
  } finally {
    setSessionLoading(false);
  }
}

/**
 * Update an order session (mutable fields: start_date, end_date, allow_late).
 */
async function updateSession(
  request: UpdateOrderSession,
): Promise<OrderSession | null> {
  try {
    setSessionLoading(true);
    setOrderError(null);

    const res = await fetch("/api/update-order-session", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<OrderSession> = await res.json();
    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Update session failed (${res.status})`);
    }

    const session = json.data;
    // Refresh open sessions cache to reflect changes
    await fetchOpenSessions(session.restaurant_id);

    return session;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] updateSession failed:", msg);
    return null;
  } finally {
    setSessionLoading(false);
  }
}

/**
 * Transition a session's status (cancel, close, send, reopen).
 */
async function transitionSession(
  sessionId: string,
  action: "cancel" | "close" | "send" | "reopen",
): Promise<OrderSession | null> {
  try {
    setSessionLoading(true);
    setOrderError(null);

    const res = await fetch(`/api/order-sessions/${sessionId}/${action}`, {
      method: "POST",
    });

    const json: ApiResponse<OrderSessionStatusResponse> = await res.json();
    if (!res.ok || !json.success || json.data == null) {
      throw new Error(
        json.error ?? `${action} session failed (${res.status})`,
      );
    }

    const session = json.data.session;

    // Refresh open sessions cache to reflect the status change
    await fetchOpenSessions(session.restaurant_id);

    return session;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error(`[orderStore] ${action}Session failed:`, msg);
    return null;
  } finally {
    setSessionLoading(false);
  }
}

async function cancelSession(sessionId: string): Promise<OrderSession | null> {
  return transitionSession(sessionId, "cancel");
}

async function closeSession(sessionId: string): Promise<OrderSession | null> {
  return transitionSession(sessionId, "close");
}

async function sendSession(sessionId: string): Promise<OrderSession | null> {
  return transitionSession(sessionId, "send");
}

async function reopenSession(sessionId: string): Promise<OrderSession | null> {
  return transitionSession(sessionId, "reopen");
}

// ══════════════════════════════════════════════════════════════════
// API — Orders
// ══════════════════════════════════════════════════════════════════

/**
 * Place an order. Converts current cart items into CreateOrderItem[]
 * and sends them to the backend. On success, clears the cart.
 *
 * If `sessionId` is null, the backend will resolve or auto-create a session.
 */
async function placeOrder(
  restaurantId: string,
  sessionId: string | null,
): Promise<Order | null> {
  const items = getCart(restaurantId);
  if (items.length === 0) {
    setOrderError("Your cart is empty.");
    return null;
  }

  const createItems: CreateOrderItem[] = items.map((ci) => ({
    item_id: ci.sectionItem.item.id,
    slot_id: null,
    notes: ci.notes,
  }));

  const request: CreateOrder = {
    restaurant_id: restaurantId,
    session_id: sessionId,
    offer_id: null,
    items: createItems,
  };

  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch("/api/orders", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<Order> = await res.json();
    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Place order failed (${res.status})`);
    }

    // Clear the cart on success
    clearCart(restaurantId);

    // Refresh open sessions (order may have auto-created one)
    await fetchOpenSessions(restaurantId);

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] placeOrder failed:", msg);
    return null;
  } finally {
    setOrderLoading(false);
  }
}

/**
 * Fetch a single order by ID (with items).
 */
async function fetchOrder(orderId: string): Promise<Order | null> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch(`/api/orders/${orderId}`);
    if (!res.ok) {
      if (res.status === 404) return null;
      throw new Error(`Failed to fetch order (${res.status})`);
    }

    const json: ApiResponse<Order> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchOrder failed:", msg);
    return null;
  } finally {
    setOrderLoading(false);
  }
}

/**
 * Delete an order (only while the parent session is Open).
 */
async function deleteOrder(orderId: string): Promise<boolean> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch(`/api/orders/${orderId}`, { method: "DELETE" });

    const json: ApiResponse<null> = await res.json();
    if (!res.ok || !json.success) {
      throw new Error(json.error ?? `Delete order failed (${res.status})`);
    }

    return true;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] deleteOrder failed:", msg);
    return false;
  } finally {
    setOrderLoading(false);
  }
}

/**
 * Fetch all orders for a session (with items).
 */
async function fetchOrdersForSession(
  sessionId: string,
): Promise<Order[]> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch(
      `/api/order-sessions/${sessionId}/orders`,
    );
    if (!res.ok) {
      throw new Error(`Failed to fetch orders (${res.status})`);
    }

    const json: ApiResponse<Order[]> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchOrdersForSession failed:", msg);
    return [];
  } finally {
    setOrderLoading(false);
  }
}

/**
 * Fetch lightweight order summaries for a session.
 */
async function fetchOrderSummaries(
  sessionId: string,
): Promise<OrderSummary[]> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch(
      `/api/order-sessions/${sessionId}/orders/summaries`,
    );
    if (!res.ok) {
      throw new Error(`Failed to fetch order summaries (${res.status})`);
    }

    const json: ApiResponse<OrderSummary[]> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchOrderSummaries failed:", msg);
    return [];
  } finally {
    setOrderLoading(false);
  }
}

/**
 * Move an order to a different session.
 */
async function moveOrderToSession(
  orderId: string,
  newSessionId: string,
): Promise<Order | null> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const request: MoveOrderToSessionRequest = { new_session_id: newSessionId };
    const res = await fetch(`/api/orders/${orderId}/move-to-session`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<Order> = await res.json();
    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Move order failed (${res.status})`);
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] moveOrderToSession failed:", msg);
    return null;
  } finally {
    setOrderLoading(false);
  }
}

/**
 * Fetch all orders placed by the current user across all open sessions for a restaurant.
 */
async function fetchMyOrdersInOpenSessions(
  restaurantId: string,
): Promise<Order[]> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch(
      `/api/restaurants/${restaurantId}/orders/mine`,
    );
    if (!res.ok) {
      throw new Error(`Failed to fetch your orders (${res.status})`);
    }

    const json: ApiResponse<Order[]> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchMyOrdersInOpenSessions failed:", msg);
    return [];
  } finally {
    setOrderLoading(false);
  }
}

/**
 * Fetch orders placed by the current user in a session.
 */
async function fetchMyOrdersInSession(
  sessionId: string,
): Promise<Order[]> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch(
      `/api/order-sessions/${sessionId}/orders/mine`,
    );
    if (!res.ok) {
      throw new Error(`Failed to fetch your orders (${res.status})`);
    }

    const json: ApiResponse<Order[]> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchMyOrdersInSession failed:", msg);
    return [];
  } finally {
    setOrderLoading(false);
  }
}

// ══════════════════════════════════════════════════════════════════
// API — Restaurant Order Settings
// ══════════════════════════════════════════════════════════════════

/**
 * Fetch the order settings for a restaurant.
 */
async function fetchOrderSettings(
  restaurantId: string,
): Promise<RestaurantOrderSettings | null> {
  try {
    setOrderError(null);

    const res = await fetch(
      `/api/restaurants/${restaurantId}/order-settings`,
    );
    if (!res.ok) {
      throw new Error(`Failed to fetch order settings (${res.status})`);
    }

    const json: ApiResponse<RestaurantOrderSettings> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] fetchOrderSettings failed:", msg);
    return null;
  }
}

/**
 * Update the order settings for a restaurant.
 */
async function updateOrderSettings(
  request: UpdateOrderSettingsRequest,
): Promise<RestaurantOrderSettings | null> {
  try {
    setOrderLoading(true);
    setOrderError(null);

    const res = await fetch(`/api/update-order-settings`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    if (!res.ok) {
      const text = await res.text();
      throw new Error(text || `Failed to update order settings (${res.status})`);
    }

    const json: ApiResponse<RestaurantOrderSettings> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOrderError(msg);
    console.error("[orderStore] updateOrderSettings failed:", msg);
    return null;
  } finally {
    setOrderLoading(false);
  }
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

function clearOrderError(): void {
  setOrderError(null);
}

/**
 * Get all cached open sessions for a restaurant (without fetching).
 */
function getOpenSessions(restaurantId: string): OrderSession[] {
  return openSessionsByRestaurant()[restaurantId] ?? [];
}

/**
 * Get the first cached open session for a restaurant (without fetching).
 * Used for backward compatibility with components expecting a single session.
 */
function getActiveSession(restaurantId: string): OrderSession | null {
  return getOpenSessions(restaurantId)[0] ?? null;
}

/**
 * Status badge color for Bulma tags.
 */
function sessionStatusColor(status: OrderSessionStatus): string {
  switch (status) {
    case "Open":
      return "is-success";
    case "Closed":
      return "is-warning";
    case "Sent":
      return "is-info";
    case "Cancelled":
      return "is-danger";
    default:
      return "is-light";
  }
}

export {
  // ── Cart state ──────────────────────────────────────────────
  cartsByRestaurant,
  getCart,
  getCartTotal,
  getCartCount,
  addToCart,
  removeFromCart,
  updateCartItemNotes,
  clearCart,
  formatPrice,

  // ── Session state ───────────────────────────────────────────
  openSessionsByRestaurant,
  getOpenSessions,
  getActiveSession,
  sessionLoading,

  // ── Order state ─────────────────────────────────────────────
  orderLoading,
  orderError,
  clearOrderError,

  // ── Session API ─────────────────────────────────────────────
  fetchOpenSessions,
  fetchActiveSession,
  fetchSessionsForRestaurant,
  fetchSession,
  createSession,
  updateSession,
  cancelSession,
  closeSession,
  sendSession,
  reopenSession,

  // ── Order API ───────────────────────────────────────────────
  placeOrder,
  fetchOrder,
  deleteOrder,
  moveOrderToSession,
  fetchOrdersForSession,
  fetchOrderSummaries,
  fetchMyOrdersInSession,
  fetchMyOrdersInOpenSessions,

  // ── Settings API ────────────────────────────────────────────
  fetchOrderSettings,
  updateOrderSettings,

  // ── Helpers ─────────────────────────────────────────────────
  sessionStatusColor,
  groupOrderItems,
  summarizeOrderItems,
};