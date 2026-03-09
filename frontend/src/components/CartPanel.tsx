import { Show, For, createSignal, createMemo } from "solid-js";
import type { MenuSectionItem } from "@bindings/MenuSectionItem";
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
  getActiveSession,
  type CartItem,
} from "@/stores/orderStore";
import {
  getOfferCart,
  getOfferCartTotal,
  getOfferCartCount,
  removeOfferFromCart,
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

interface CartPanelProps {
  restaurantId: string;
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
  const [successMessage, setSuccessMessage] = createSignal<string | null>(null);
  const [placingOfferOrder, setPlacingOfferOrder] = createSignal(false);
  const [offerOrderError, setOfferOrderError] = createSignal<string | null>(null);

  const cart = () => getCart(props.restaurantId);
  const total = () => getCartTotal(props.restaurantId);
  const count = () => getCartCount(props.restaurantId);
  const activeSession = () => getActiveSession(props.restaurantId);

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

    const session = activeSession();
    const sessionId = session?.id ?? null;
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
            notes: null,
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
        `Order placed! Total: $${formatPrice(lastTotalCents)} (${totalPlaced} item${totalPlaced !== 1 ? "s" : ""})`,
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
              <div
                class="box p-3 mb-2"
                style={{
                  background:
                    "linear-gradient(135deg, hsl(141, 53%, 97%) 0%, hsl(204, 71%, 97%) 100%)",
                  border: "1px solid hsl(141, 53%, 88%)",
                }}
              >
                {/* Offer title row */}
                <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
                  <div>
                    <span class="has-text-weight-bold is-size-6">
                      🍽️ {entry.offer.title}
                    </span>
                  </div>
                  <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
                    <span class="has-text-weight-bold">
                      ${formatOfferPrice(entry.totalPriceCents)}
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
                              +${formatOfferPrice(slotGroup.slotSupplementCents)}
                            </span>
                          </Show>
                        </p>
                        <For each={slotGroup.items}>
                          {(item) => (
                            <p class="is-size-7 has-text-grey ml-3">
                              • {item.name}
                              <Show when={item.supplementCents > 0}>
                                <span class="has-text-warning-dark ml-1">
                                  (+${formatOfferPrice(item.supplementCents)})
                                </span>
                              </Show>
                            </p>
                          )}
                        </For>
                      </div>
                    )}
                  </For>
                </div>

                {/* Price breakdown */}
                <Show when={entry.totalPriceCents > entry.basePriceCents}>
                  <p class="is-size-7 has-text-grey mt-1" style={{ "border-top": "1px solid hsl(141, 53%, 88%)", "padding-top": "0.25rem" }}>
                    Base: ${formatOfferPrice(entry.basePriceCents)} + supplements: ${formatOfferPrice(entry.totalPriceCents - entry.basePriceCents)}
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
                    {/* Item name + image hint */}
                    <div
                      style={{
                        flex: "1",
                        "min-width": "0",
                        cursor: "pointer",
                      }}
                      onClick={() => toggleExpand(group.itemId)}
                      title="Click to expand notes"
                    >
                      <div class="is-flex is-align-items-center">
                        <Show when={item().image_url}>
                          <figure
                            class="image is-24x24 mr-2"
                            style={{
                              "border-radius": "4px",
                              overflow: "hidden",
                              "flex-shrink": "0",
                              "min-width": "24px",
                            }}
                          >
                            <img
                              src={item().image_url!}
                              alt=""
                              style={{
                                "object-fit": "cover",
                                width: "100%",
                                height: "100%",
                              }}
                            />
                          </figure>
                        </Show>
                        <span class="has-text-weight-semibold is-size-6" style={{ "overflow": "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }}>
                          {item().name}
                        </span>
                        <Show when={notesCount(group) > 0}>
                          <span class="tag is-light is-small ml-2" title={`${notesCount(group)} note(s)`}>
                            📝{notesCount(group)}
                          </span>
                        </Show>
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
                      ${formatPrice(group.lineTotalCents)}
                    </span>
                  </div>

                  {/* Unit price hint when qty > 1 */}
                  <Show when={group.quantity > 1}>
                    <p class="has-text-grey is-size-7 mt-1" style={{ "padding-left": item().image_url ? "32px" : "0" }}>
                      ${formatPrice(group.unitPriceCents)} each
                    </p>
                  </Show>

                  {/* Expanded: per-entry notes */}
                  <Show when={isExpanded()}>
                    <div
                      class="mt-3 pt-2"
                      style={{ "border-top": "1px solid var(--bulma-border-weak)" }}
                    >
                      <p class="is-size-7 has-text-grey mb-2">
                        Notes for each item (optional):
                      </p>
                      <For each={group.entries}>
                        {(entry, idx) => (
                          <div class="field mb-2">
                            <div class="control">
                              <input
                                class="input is-small"
                                type="text"
                                placeholder={`Item ${idx() + 1} note (e.g. no onions)…`}
                                value={entry.notes ?? ""}
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
                    </div>
                  </Show>
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
            ${formatPrice(combinedTotal())}
          </span>
        </div>

        {/* Breakdown when both types are present */}
        <Show when={count() > 0 && offerCount() > 0}>
          <div class="is-size-7 has-text-grey mb-3">
            <div class="is-flex is-justify-content-space-between">
              <span>À la carte items</span>
              <span>${formatPrice(total())}</span>
            </div>
            <div class="is-flex is-justify-content-space-between">
              <span>
                Offer{offerCount() > 1 ? "s" : ""} ({offerCount()})
              </span>
              <span>${formatPrice(offerTotal())}</span>
            </div>
          </div>
        </Show>

        {/* Active session info */}
        <Show when={activeSession()}>
          {(session) => (
            <div class="notification is-info is-light is-size-7 py-2 px-3 mb-3">
              <strong>Session:</strong> {session().status} — ends{" "}
              {new Date(session().end_date).toLocaleString()}
            </div>
          )}
        </Show>

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
            <span>Place Order — ${formatPrice(combinedTotal())}</span>
          </button>
        </Show>
      </Show>
    </div>
  );
}