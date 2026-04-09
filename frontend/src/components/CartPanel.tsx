import { Show, For, createSignal, createMemo, createEffect } from "solid-js";
import { isImageSrc } from "@/lib/imageUrl";
import type { MenuSectionItem } from "@bindings/MenuSectionItem";
import type { OrderSession } from "@bindings/OrderSession";
import {
  getCart,
  getCartTotal,
  getCartCount,
  addToCart,
  removeFromCart,
  updateCartItemNotes,
  clearCart,
  placeOrder,
  formatPrice,
  orderLoading,
  orderError,
  clearOrderError,
  createSession,
  sessionLoading,
  type CartItem,
} from "@/stores/orderStore";
import {
  getOfferCart,
  getOfferCartTotal,
  getOfferCartCount,
  removeOfferFromCart,
  updateOfferCartNotes,
  clearOfferCart,
  formatOfferPrice,
  type OfferCartEntry,
} from "@/stores/offerStore";
import { isAuthenticated } from "@/stores/authStore";
import AuthPanel from "@/components/AuthPanel";
import type { ApiResponse } from "@bindings/ApiResponse";
import type { Order } from "@bindings/Order";
import type { CreateOrder } from "@bindings/CreateOrder";
import type { CreateOrderItem } from "@bindings/CreateOrderItem";
import type { CreateOrderSession } from "@bindings/CreateOrderSession";

/** Format a session's pickup time for display: uses pickup_time if set, else end_date. */
function sessionLabel(session: OrderSession): string {
  const dt = session.pickup_time ?? session.end_date;
  return new Date(dt).toLocaleString(undefined, { timeStyle: "short", dateStyle: "short" });
}

/** Round a Date up to the nearest 5 minutes and format as datetime-local string. */
function toDatetimeLocal(date: Date): string {
  const ms = 5 * 60 * 1000;
  const rounded = new Date(Math.ceil(date.getTime() / ms) * ms);
  const y = rounded.getFullYear();
  const M = String(rounded.getMonth() + 1).padStart(2, "0");
  const d = String(rounded.getDate()).padStart(2, "0");
  const h = String(rounded.getHours()).padStart(2, "0");
  const m = String(rounded.getMinutes()).padStart(2, "0");
  return `${y}-${M}-${d}T${h}:${m}`;
}

interface CartPanelProps {
  restaurantId: string;
  /** All currently open sessions for this restaurant. */
  openSessions: OrderSession[];
  /** Called after an order is successfully placed. */
  onOrderPlaced?: () => void;
}

/** A group of identical items in the cart (same item ID). */
interface CartGroup {
  /** The item ID shared by all entries in this group. */
  itemId: string;
  /** The section item (used for display info and re-adding). */
  sectionItem: MenuSectionItem;
  /** All individual cart entries belonging to this group. */
  entries: CartItem[];
  /** Total quantity. */
  quantity: number;
  /** Unit price (cents) for display. */
  unitPriceCents: number;
  /** Line total (cents). */
  lineTotalCents: number;
}

export default function CartPanel(props: CartPanelProps) {
  const [expandedGroupId, setExpandedGroupId] = createSignal<string | null>(null);
  const [expandedOfferKey, setExpandedOfferKey] = createSignal<number | null>(null);
  const [successMessage, setSuccessMessage] = createSignal<string | null>(null);
  const [placingOfferOrder, setPlacingOfferOrder] = createSignal(false);
  const [offerOrderError, setOfferOrderError] = createSignal<string | null>(null);

  // ── Session selection ────────────────────────────────────────
  // null = use backend default (auto-create); string = specific session id
  const [selectedSessionId, setSelectedSessionId] = createSignal<string | null>(null);
  // For "new slot" creation
  const [showNewSlot, setShowNewSlot] = createSignal(false);
  const [newSlotEnd, setNewSlotEnd] = createSignal(() => {
    const d = new Date();
    d.setHours(d.getHours() + 1, 0, 0, 0);
    return toDatetimeLocal(d);
  });
  const [newSlotPickup, setNewSlotPickup] = createSignal("");
  const [creatingSession, setCreatingSession] = createSignal(false);
  const [newSlotError, setNewSlotError] = createSignal<string | null>(null);

  // Auto-select the only session when there's exactly one
  createEffect(() => {
    const sessions = props.openSessions;
    if (sessions.length === 1) {
      setSelectedSessionId(sessions[0].id);
    } else if (sessions.length === 0) {
      setSelectedSessionId(null);
    }
    // If 2+ sessions and no selection, keep null (user must choose)
  });

  const cart = () => getCart(props.restaurantId);
  const total = () => getCartTotal(props.restaurantId);
  const count = () => getCartCount(props.restaurantId);

  // ── Offer cart ──────────────────────────────────────────────────
  const offerCart = () => getOfferCart(props.restaurantId);
  const offerTotal = () => getOfferCartTotal(props.restaurantId);
  const offerCount = () => getOfferCartCount(props.restaurantId);

  // ── Combined totals ─────────────────────────────────────────────
  const combinedTotal = () => total() + offerTotal();
  const combinedCount = () => count() + offerCount();
  const hasAnyItems = () => combinedCount() > 0;

  /**
   * Group cart items by item ID for a compact display.
   */
  const groups = createMemo((): CartGroup[] => {
    const items = cart();
    const map = new Map<string, CartGroup>();

    for (const ci of items) {
      const id = ci.sectionItem.item.id;
      const existing = map.get(id);
      const unitPrice =
        ci.sectionItem.price_override_cents ?? ci.sectionItem.item.base_price_cents;

      if (existing) {
        existing.entries.push(ci);
        existing.quantity += 1;
        existing.lineTotalCents += unitPrice;
      } else {
        map.set(id, {
          itemId: id,
          sectionItem: ci.sectionItem,
          entries: [ci],
          quantity: 1,
          unitPriceCents: unitPrice,
          lineTotalCents: unitPrice,
        });
      }
    }

    return Array.from(map.values());
  });

  // ── Actions ─────────────────────────────────────────────────────

  /**
   * Place all orders: regular items first, then each offer entry as a
   * separate order (since each offer order has its own offer_id + slot mapping).
   */
  const handlePlaceOrder = async () => {
    clearOrderError();
    setOfferOrderError(null);
    setSuccessMessage(null);

    // If multiple sessions exist but none is selected, prompt user to choose
    if (props.openSessions.length > 1 && !selectedSessionId()) {
      setOfferOrderError("Please select a pickup time before placing your order.");
      return;
    }

    const sessionId = selectedSessionId();
    let totalPlaced = 0;
    let lastTotalCents = 0;

    // 1. Place regular items order (if any)
    if (count() > 0) {
      const order = await placeOrder(props.restaurantId, sessionId);
      if (!order) return; // error is set in orderStore
      totalPlaced += order.items.length;
      lastTotalCents += order.total_price_cents;
    }

    // 2. Place each offer cart entry as a separate order
    const offerEntries = offerCart();
    if (offerEntries.length > 0) {
      setPlacingOfferOrder(true);
      try {
        for (const entry of offerEntries) {
          const createItems: CreateOrderItem[] = entry.selections.map((sel) => ({
            item_id: sel.item.id,
            slot_id: sel.slotId,
            notes: entry.notes,
          }));

          const request: CreateOrder = {
            restaurant_id: props.restaurantId,
            session_id: sessionId,
            offer_id: entry.offer.id,
            items: createItems,
          };

          const res = await fetch("/api/orders", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(request),
          });

          const json: ApiResponse<Order> = await res.json();
          if (!res.ok || !json.success || json.data == null) {
            throw new Error(
              json.error ?? `Failed to place offer order (${res.status})`,
            );
          }

          totalPlaced += json.data.items.length;
          lastTotalCents += json.data.total_price_cents;
        }

        // Clear offer cart on success
        clearOfferCart(props.restaurantId);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setOfferOrderError(msg);
        console.error("[CartPanel] offer order failed:", msg);
        setPlacingOfferOrder(false);
        return;
      } finally {
        setPlacingOfferOrder(false);
      }
    }

    if (totalPlaced > 0) {
      setSuccessMessage(
        `Order placed! Total: €${formatPrice(lastTotalCents)} (${totalPlaced} item${totalPlaced !== 1 ? "s" : ""})`,
      );
      props.onOrderPlaced?.();
    }
  };

  const handleClear = () => {
    clearCart(props.restaurantId);
    clearOfferCart(props.restaurantId);
    setSuccessMessage(null);
    clearOrderError();
    setOfferOrderError(null);
    setExpandedGroupId(null);
    setShowNewSlot(false);
    setNewSlotError(null);
  };

  /** Create a new pickup session and select it. */
  const handleCreateNewSlot = async () => {
    setNewSlotError(null);
    const endStr = newSlotEnd();
    if (!endStr) {
      setNewSlotError("Please enter a pickup time.");
      return;
    }
    const endMs = new Date(endStr).getTime();
    if (isNaN(endMs) || endMs <= Date.now()) {
      setNewSlotError("Pickup time must be in the future.");
      return;
    }

    const now = new Date();
    const pickupStr = newSlotPickup().trim();
    const request: CreateOrderSession = {
      restaurant_id: props.restaurantId,
      start_date: now.toISOString(),
      end_date: new Date(endStr).toISOString(),
      pickup_time: pickupStr ? new Date(pickupStr).toISOString() : null,
      allow_late: false,
    };

    setCreatingSession(true);
    try {
      const session = await createSession(request);
      if (session) {
        setSelectedSessionId(session.id);
        setShowNewSlot(false);
        props.onOrderPlaced?.(); // triggers session refresh in parent
      }
    } finally {
      setCreatingSession(false);
    }
  };

  /** Add one more of the same item. */
  const handleIncrement = (group: CartGroup) => {
    addToCart(props.restaurantId, group.sectionItem);
  };

  /** Remove the last-added entry of this item. */
  const handleDecrement = (group: CartGroup) => {
    const last = group.entries[group.entries.length - 1];
    if (last) {
      removeFromCart(props.restaurantId, last.key);
    }
  };

  const toggleExpand = (itemId: string) => {
    setExpandedGroupId((prev) => (prev === itemId ? null : itemId));
  };

  /** Number of entries in a group that have notes. */
  const notesCount = (group: CartGroup): number =>
    group.entries.filter((e) => e.notes).length;

  const isLoading = () => orderLoading() || placingOfferOrder();

  // ── Offer entry helpers ─────────────────────────────────────────

  /** Group offer selections by slot label for display. */
  const offerSelectionsBySlot = (
    entry: OfferCartEntry,
  ): Array<{ label: string; items: Array<{ name: string; supplementCents: number }>; slotSupplementCents: number }> => {
    const slotMap = new Map<
      string,
      { label: string; items: Array<{ name: string; supplementCents: number }>; slotSupplementCents: number }
    >();

    for (const sel of entry.selections) {
      const slot = entry.offer.slots.find((s) => s.id === sel.slotId);
      const label = slot?.label ?? "Unknown";
      const slotSupplement = slot?.supplement_cents ?? 0;

      const existing = slotMap.get(sel.slotId);
      if (existing) {
        existing.items.push({
          name: sel.item.name,
          supplementCents: sel.supplementCents,
        });
      } else {
        slotMap.set(sel.slotId, {
          label,
          items: [{ name: sel.item.name, supplementCents: sel.supplementCents }],
          slotSupplementCents: slotSupplement,
        });
      }
    }

    return Array.from(slotMap.values());
  };

  return (
    <div class="box">
      {/* Header */}
      <div class="is-flex is-justify-content-space-between is-align-items-center mb-3">
        <h3 class="title is-5 mb-0">
          🛒 Cart
          <Show when={combinedCount() > 0}>
            <span class="tag is-primary is-light ml-2">{combinedCount()}</span>
          </Show>
        </h3>
        <Show when={hasAnyItems()}>
          <button
            class="button is-small is-light is-danger"
            onClick={handleClear}
            disabled={isLoading()}
          >
            Clear
          </button>
        </Show>
      </div>

      {/* Success message */}
      <Show when={successMessage()}>
        <div class="notification is-success is-light">
          <button
            class="delete"
            type="button"
            onClick={() => setSuccessMessage(null)}
          />
          {successMessage()}
        </div>
      </Show>

      {/* Error message (regular orders) */}
      <Show when={orderError()}>
        <div class="notification is-danger is-light">
          <button class="delete" type="button" onClick={clearOrderError} />
          {orderError()}
        </div>
      </Show>

      {/* Error message (offer orders) */}
      <Show when={offerOrderError()}>
        <div class="notification is-danger is-light">
          <button class="delete" type="button" onClick={() => setOfferOrderError(null)} />
          {offerOrderError()}
        </div>
      </Show>

      {/* Empty cart */}
      <Show when={!hasAnyItems() && !successMessage()}>
        <div class="has-text-centered py-4">
          <p class="is-size-4 mb-2">🍽️</p>
          <p class="has-text-grey">
            Your cart is empty. Browse the menu and add items!
          </p>
        </div>
      </Show>

      {/* ── Offer cart entries ──────────────────────────────────── */}
      <Show when={offerCount() > 0}>
        <div class="mb-4">
          <p class="has-text-weight-semibold is-size-7 has-text-grey-dark mb-2">
            🏷️ OFFERS
          </p>
          <For each={offerCart()}>
            {(entry) => (
              <div class="box p-3 mb-2 has-background-light">
                {/* Offer title row */}
                <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
                  <div>
                    <span class="has-text-weight-bold is-size-6">
                      🍽️ {entry.offer.title}
                    </span>
                  </div>
                  <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
                    <span class="has-text-weight-bold">
                      €{formatOfferPrice(entry.totalPriceCents)}
                    </span>
                    <button
                      class="button is-small is-danger is-outlined"
                      disabled={isLoading()}
                      onClick={() =>
                        removeOfferFromCart(props.restaurantId, entry.key)
                      }
                      title="Remove this offer"
                    >
                      <span class="icon is-small">
                        <span>✕</span>
                      </span>
                    </button>
                  </div>
                </div>

                {/* Slot breakdown */}
                <div style={{ "padding-left": "0.25rem" }}>
                  <For each={offerSelectionsBySlot(entry)}>
                    {(slotGroup) => (
                      <div class="mb-1">
                        <p class="is-size-7 has-text-grey-dark has-text-weight-medium">
                          {slotGroup.label}
                          <Show when={slotGroup.slotSupplementCents > 0}>
                            <span class="tag is-warning is-light ml-1" style={{ "font-size": "0.6rem" }}>
                              +€{formatOfferPrice(slotGroup.slotSupplementCents)}
                            </span>
                          </Show>
                        </p>
                        <For each={slotGroup.items}>
                          {(item) => (
                            <p class="is-size-7 has-text-grey ml-3">
                              • {item.name}
                              <Show when={item.supplementCents > 0}>
                                <span class="has-text-warning-dark ml-1">
                                  (+€{formatOfferPrice(item.supplementCents)})
                                </span>
                              </Show>
                            </p>
                          )}
                        </For>
                      </div>
                    )}
                  </For>
                </div>

                {/* Notes */}
                <Show
                  when={expandedOfferKey() === entry.key}
                  fallback={
                    <Show
                      when={entry.notes}
                      fallback={
                        <button
                          class="button is-ghost is-small px-0 has-text-grey mt-1"
                          style={{ height: "auto", "font-size": "0.72rem", "text-decoration": "none" }}
                          onClick={() => setExpandedOfferKey(entry.key)}
                        >
                          ＋ Add a note
                        </button>
                      }
                    >
                      <div
                        class="is-flex is-align-items-center mt-2"
                        style={{ gap: "0.3rem", cursor: "pointer" }}
                        onClick={() => setExpandedOfferKey(entry.key)}
                        title="Click to edit note"
                      >
                        <span class="is-size-7 has-text-grey-dark is-italic">📝 {entry.notes}</span>
                        <span class="is-size-7 has-text-grey-light">✏️</span>
                      </div>
                    </Show>
                  }
                >
                  <div class="mt-2 pt-2" style={{ "border-top": "1px solid var(--bulma-border-weak)" }}>
                    <div class="field mb-1">
                      <div class="control">
                        <input
                          class="input is-small"
                          type="text"
                          placeholder="e.g. extra spicy, no sauce…"
                          ref={(el) => { el.value = entry.notes ?? ""; }}
                          onInput={(e) =>
                            updateOfferCartNotes(
                              props.restaurantId,
                              entry.key,
                              e.currentTarget.value || null,
                            )
                          }
                        />
                      </div>
                    </div>
                    <button
                      class="button is-ghost is-small px-0 has-text-grey"
                      style={{ height: "auto", "font-size": "0.72rem" }}
                      onClick={() => setExpandedOfferKey(null)}
                    >
                      Done
                    </button>
                  </div>
                </Show>

                {/* Price breakdown */}
                <Show when={entry.totalPriceCents > entry.basePriceCents}>
                  <p class="is-size-7 has-text-grey mt-1" style={{ "border-top": "1px solid var(--bulma-border-weak)", "padding-top": "0.25rem" }}>
                    Base: €{formatOfferPrice(entry.basePriceCents)} + supplements: €{formatOfferPrice(entry.totalPriceCents - entry.basePriceCents)}
                  </p>
                </Show>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* ── Regular cart items ──────────────────────────────────── */}
      <Show when={count() > 0}>
        <div class="mb-4">
          <Show when={offerCount() > 0}>
            <p class="has-text-weight-semibold is-size-7 has-text-grey-dark mb-2">
              📋 À LA CARTE
            </p>
          </Show>
          <For each={groups()}>
            {(group) => {
              const item = () => group.sectionItem.item;
              const isExpanded = () => expandedGroupId() === group.itemId;

              return (
                <div class="box p-3 mb-2">
                  {/* Main row: name, quantity controls, line total */}
                  <div class="is-flex is-justify-content-space-between is-align-items-center">
                    {/* Item name */}
                    <div style={{ flex: "1", "min-width": "0" }}>
                      <div class="is-flex is-align-items-center">
                        <Show when={item().image_url}>
                          <div
                            class="mr-2"
                            style={{
                              width: "24px",
                              height: "24px",
                              "min-width": "24px",
                              "border-radius": "4px",
                              overflow: "hidden",
                              "flex-shrink": "0",
                              display: "flex",
                              "align-items": "center",
                              "justify-content": "center",
                              "background-color": "var(--bulma-scheme-main-bis)",
                            }}
                          >
                            <Show
                              when={isImageSrc(item().image_url!)}
                              fallback={
                                <span style={{ "font-size": "0.9rem", "line-height": "1" }}>
                                  {item().image_url}
                                </span>
                              }
                            >
                              <img
                                src={item().image_url!}
                                alt=""
                                style={{ "object-fit": "cover", width: "100%", height: "100%" }}
                              />
                            </Show>
                          </div>
                        </Show>
                        <span class="has-text-weight-semibold is-size-6" style={{ overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }}>
                          {item().name}
                        </span>
                      </div>
                    </div>

                    {/* Quantity controls */}
                    <div
                      class="is-flex is-align-items-center ml-3"
                      style={{ gap: "0.35rem", "flex-shrink": "0" }}
                    >
                      <button
                        class="button is-small is-light"
                        onClick={() => handleDecrement(group)}
                        disabled={isLoading()}
                        title="Remove one"
                      >
                        −
                      </button>
                      <span
                        class="has-text-weight-bold"
                        style={{
                          "min-width": "1.6rem",
                          "text-align": "center",
                          "font-size": "0.95rem",
                        }}
                      >
                        {group.quantity}
                      </span>
                      <button
                        class="button is-small is-light"
                        onClick={() => handleIncrement(group)}
                        disabled={isLoading()}
                        title="Add one more"
                      >
                        +
                      </button>
                    </div>

                    {/* Line total */}
                    <span
                      class="has-text-weight-bold ml-3"
                      style={{ "white-space": "nowrap", "min-width": "3.5rem", "text-align": "right" }}
                    >
                      €{formatPrice(group.lineTotalCents)}
                    </span>
                  </div>

                  {/* Unit price hint when qty > 1 */}
                  <Show when={group.quantity > 1}>
                    <p class="has-text-grey is-size-7 mt-1" style={{ "padding-left": item().image_url ? "32px" : "0" }}>
                      €{formatPrice(group.unitPriceCents)} each
                    </p>
                  </Show>

                  {/* Notes affordance */}
                  <div style={{ "padding-left": item().image_url ? "32px" : "0" }}>
                    <Show
                      when={isExpanded()}
                      fallback={
                        <Show
                          when={notesCount(group) > 0}
                          fallback={
                            <button
                              class="button is-ghost is-small px-0 has-text-grey"
                              style={{ height: "auto", "font-size": "0.72rem", "text-decoration": "none" }}
                              onClick={() => toggleExpand(group.itemId)}
                            >
                              ＋ Add a note
                            </button>
                          }
                        >
                          {/* Notes summary — click to edit */}
                          <div
                            class="is-flex is-align-items-center mt-1"
                            style={{ gap: "0.3rem", cursor: "pointer" }}
                            onClick={() => toggleExpand(group.itemId)}
                            title="Click to edit notes"
                          >
                            <span class="is-size-7 has-text-grey is-italic" style={{ "line-height": "1.3" }}>
                              <For each={group.entries}>
                                {(entry, idx) => (
                                  <Show when={entry.notes}>
                                    <span>
                                      <Show when={group.quantity > 1}>
                                        <span class="has-text-grey-light">#{idx() + 1} </span>
                                      </Show>
                                      {entry.notes}
                                      <Show when={idx() < group.entries.length - 1 && group.entries[idx() + 1]?.notes}>
                                        <span class="has-text-grey-light"> · </span>
                                      </Show>
                                    </span>
                                  </Show>
                                )}
                              </For>
                            </span>
                            <span class="is-size-7 has-text-grey-light" title="Edit note">✏️</span>
                          </div>
                        </Show>
                      }
                    >
                      {/* Expanded: per-entry note inputs */}
                      <div
                        class="mt-2 pt-2"
                        style={{ "border-top": "1px solid var(--bulma-border-weak)" }}
                      >
                        <For each={group.entries}>
                          {(entry, idx) => (
                            <div class="field mb-2">
                              <div class="control">
                                <input
                                  class="input is-small"
                                  type="text"
                                  placeholder={group.quantity > 1 ? `Item ${idx() + 1} — e.g. not spicy` : "e.g. not spicy, extra sauce…"}
                                  ref={(el) => { el.value = entry.notes ?? ""; }}
                                  onInput={(e) =>
                                    updateCartItemNotes(
                                      props.restaurantId,
                                      entry.key,
                                      e.currentTarget.value || null,
                                    )
                                  }
                                />
                              </div>
                            </div>
                          )}
                        </For>
                        <button
                          class="button is-ghost is-small px-0 has-text-grey"
                          style={{ height: "auto", "font-size": "0.72rem" }}
                          onClick={() => toggleExpand(group.itemId)}
                        >
                          Done
                        </button>
                      </div>
                    </Show>
                  </div>
                </div>
              );
            }}
          </For>
        </div>
      </Show>

      {/* ── Totals & Place Order ───────────────────────────────── */}
      <Show when={hasAnyItems()}>
        {/* Total */}
        <div
          class="is-flex is-justify-content-space-between is-align-items-center py-3 mb-3"
          style={{
            "border-top": "2px solid var(--bulma-border)",
            "border-bottom": "2px solid var(--bulma-border)",
          }}
        >
          <span class="has-text-weight-bold is-size-5">Total</span>
          <span class="has-text-weight-bold is-size-5">
            €{formatPrice(combinedTotal())}
          </span>
        </div>

        {/* Breakdown when both types are present */}
        <Show when={count() > 0 && offerCount() > 0}>
          <div class="is-size-7 has-text-grey mb-3">
            <div class="is-flex is-justify-content-space-between">
              <span>À la carte items</span>
              <span>€{formatPrice(total())}</span>
            </div>
            <div class="is-flex is-justify-content-space-between">
              <span>
                Offer{offerCount() > 1 ? "s" : ""} ({offerCount()})
              </span>
              <span>€{formatPrice(offerTotal())}</span>
            </div>
          </div>
        </Show>

        {/* ── Pickup / Session picker ──────────────────────── */}
        <div class="mb-3">
          {/* No session at all */}
          <Show when={props.openSessions.length === 0}>
            <div class="notification is-info is-light is-size-7 py-2 px-3">
              No pickup session open yet — one will be created automatically.
            </div>
          </Show>

          {/* Exactly one session — show it, no choice needed */}
          <Show when={props.openSessions.length === 1}>
            {(_) => {
              const s = () => props.openSessions[0];
              return (
                <div class="notification is-info is-light is-size-7 py-2 px-3">
                  <strong>Pickup:</strong> {sessionLabel(s())}
                  <Show when={s().pickup_time}>
                    <span class="has-text-grey ml-1">(orders close {new Date(s().end_date).toLocaleString(undefined, { timeStyle: "short" })})</span>
                  </Show>
                </div>
              );
            }}
          </Show>

          {/* Multiple sessions — user must choose */}
          <Show when={props.openSessions.length > 1}>
            <div class="mb-2">
              <p class="is-size-7 has-text-weight-semibold mb-1">Pickup time</p>
              <div class="is-flex is-flex-direction-column" style={{ gap: "0.35rem" }}>
                <For each={props.openSessions}>
                  {(session) => (
                    <label
                      class="box p-2 is-flex is-align-items-center"
                      style={{
                        cursor: "pointer",
                        gap: "0.5rem",
                        border: selectedSessionId() === session.id
                          ? "2px solid var(--bulma-primary)"
                          : "2px solid var(--bulma-border)",
                        "border-radius": "6px",
                      }}
                    >
                      <input
                        type="radio"
                        name="pickup-session"
                        value={session.id}
                        checked={selectedSessionId() === session.id}
                        onChange={() => {
                          setSelectedSessionId(session.id);
                          setShowNewSlot(false);
                        }}
                      />
                      <span class="is-size-7">
                        {sessionLabel(session)}
                        <Show when={session.pickup_time}>
                          <span class="has-text-grey ml-1">(orders close {new Date(session.end_date).toLocaleString(undefined, { timeStyle: "short" })})</span>
                        </Show>
                      </span>
                    </label>
                  )}
                </For>
              </div>
            </div>
          </Show>

          {/* "Add a time slot" option — always available */}
          <Show
            when={showNewSlot()}
            fallback={
              <button
                class="button is-ghost is-small px-0 has-text-grey"
                style={{ height: "auto", "font-size": "0.75rem", "text-decoration": "none" }}
                onClick={() => setShowNewSlot(true)}
                disabled={isLoading() || creatingSession()}
              >
                + Different pickup time
              </button>
            }
          >
            <div class="box p-3" style={{ "border": "1px solid var(--bulma-border)" }}>
              <p class="is-size-7 has-text-weight-semibold mb-2">New pickup time slot</p>
              <Show when={newSlotError()}>
                <p class="help is-danger mb-1">{newSlotError()}</p>
              </Show>
              <div class="field mb-1">
                <label class="label is-size-7">Pickup time</label>
                <div class="control">
                  <input
                    class="input is-small"
                    type="datetime-local"
                    value={newSlotPickup()}
                    onInput={(e) => setNewSlotPickup(e.currentTarget.value)}
                    disabled={creatingSession()}
                    placeholder="When food is ready"
                  />
                </div>
              </div>
              <div class="field has-addons mb-2">
                <div class="control is-expanded">
                  <label class="label is-size-7">Orders close at</label>
                  <input
                    class="input is-small"
                    type="datetime-local"
                    value={newSlotEnd()}
                    onInput={(e) => setNewSlotEnd(e.currentTarget.value)}
                    disabled={creatingSession()}
                    placeholder="Ordering deadline"
                  />
                </div>
              </div>
              <div class="is-flex" style={{ gap: "0.5rem" }}>
                <button
                  class="button is-primary is-small"
                  classList={{ "is-loading": creatingSession() }}
                  disabled={creatingSession()}
                  onClick={handleCreateNewSlot}
                >
                  Create slot
                </button>
                <button
                  class="button is-light is-small"
                  onClick={() => { setShowNewSlot(false); setNewSlotError(null); }}
                  disabled={creatingSession()}
                >
                  Cancel
                </button>
              </div>
            </div>
          </Show>
        </div>

        {/* Place order — requires auth */}
        <Show
          when={isAuthenticated()}
          fallback={
            <div>
              <div class="notification is-warning is-light mb-3">
                <p class="is-size-7">
                  You need to be logged in to place an order.
                </p>
              </div>
              <AuthPanel />
            </div>
          }
        >
          <button
            class="button is-primary is-fullwidth is-medium"
            classList={{ "is-loading": isLoading() }}
            disabled={isLoading() || !hasAnyItems()}
            onClick={handlePlaceOrder}
          >
            <span class="icon">
              <span>🛒</span>
            </span>
            <span>Place Order — €{formatPrice(combinedTotal())}</span>
          </button>
        </Show>
      </Show>
    </div>
  );
}