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
import { isAuthenticated } from "@/stores/authStore";
import AuthPanel from "@/components/AuthPanel";

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

  const cart = () => getCart(props.restaurantId);
  const total = () => getCartTotal(props.restaurantId);
  const count = () => getCartCount(props.restaurantId);
  const activeSession = () => getActiveSession(props.restaurantId);

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

  const handlePlaceOrder = async () => {
    clearOrderError();
    setSuccessMessage(null);

    const session = activeSession();
    const order = await placeOrder(props.restaurantId, session?.id ?? null);

    if (order) {
      setSuccessMessage(
        `Order placed! Total: $${formatPrice(order.total_price_cents)} (${order.items.length} item${order.items.length !== 1 ? "s" : ""})`,
      );
      props.onOrderPlaced?.();
    }
  };

  const handleClear = () => {
    clearCart(props.restaurantId);
    setSuccessMessage(null);
    clearOrderError();
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

  return (
    <div class="box">
      {/* Header */}
      <div class="is-flex is-justify-content-space-between is-align-items-center mb-3">
        <h3 class="title is-5 mb-0">
          🛒 Cart
          <Show when={count() > 0}>
            <span class="tag is-primary is-light ml-2">{count()}</span>
          </Show>
        </h3>
        <Show when={count() > 0}>
          <button
            class="button is-small is-light is-danger"
            onClick={handleClear}
            disabled={orderLoading()}
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

      {/* Error message */}
      <Show when={orderError()}>
        <div class="notification is-danger is-light">
          <button class="delete" type="button" onClick={clearOrderError} />
          {orderError()}
        </div>
      </Show>

      {/* Empty cart */}
      <Show when={count() === 0 && !successMessage()}>
        <div class="has-text-centered py-4">
          <p class="is-size-4 mb-2">🍽️</p>
          <p class="has-text-grey">
            Your cart is empty. Browse the menu and add items!
          </p>
        </div>
      </Show>

      {/* Grouped cart items */}
      <Show when={count() > 0}>
        <div class="mb-4">
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
                        disabled={orderLoading()}
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
                        disabled={orderLoading()}
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
            ${formatPrice(total())}
          </span>
        </div>

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
            classList={{ "is-loading": orderLoading() }}
            disabled={orderLoading() || count() === 0}
            onClick={handlePlaceOrder}
          >
            <span class="icon">
              <span>🛒</span>
            </span>
            <span>Place Order — ${formatPrice(total())}</span>
          </button>
        </Show>
      </Show>
    </div>
  );
}