import { createSignal } from "solid-js";
import type { ApiResponse } from "@bindings/ApiResponse";
import type { Offer } from "@bindings/Offer";
import type { OfferSlot } from "@bindings/OfferSlot";
import type { CreateOffer } from "@bindings/CreateOffer";
import type { UpdateOffer } from "@bindings/UpdateOffer";
import type { ValidateOfferSelectionRequest } from "@bindings/ValidateOfferSelectionRequest";
import type { ValidateOfferSelectionResponse } from "@bindings/ValidateOfferSelectionResponse";
import type { Item } from "@bindings/Item";

// ══════════════════════════════════════════════════════════════════
// Types
// ══════════════════════════════════════════════════════════════════

/** A single item selected for a slot in the offer composer. */
export interface OfferSlotSelection {
  slotId: string;
  item: Item;
  /** The constraint-level supplement for this item in this slot (cents). */
  supplementCents: number;
}

/** A fully composed offer ready to be added to the cart. */
export interface OfferCartEntry {
  /** Unique key for this cart entry (for removal). */
  key: number;
  offer: Offer;
  selections: OfferSlotSelection[];
  /** Computed total from the validate-selection endpoint. */
  totalPriceCents: number;
  /** The offer base price (for display in breakdown). */
  basePriceCents: number;
  /** Optional note for this offer entry (e.g. "Extra spicy"). */
  notes: string | null;
}

/** Resolved items for a slot, with per-item supplement info. */
export interface SlotAllowedItems {
  slotId: string;
  items: Item[];
  /** Map of item_id → supplement_cents (from constraint resolution). */
  supplements: Record<string, number>;
}

// ══════════════════════════════════════════════════════════════════
// State
// ══════════════════════════════════════════════════════════════════

/** Cached offers per restaurant (all offers, not just active). */
const [offersByRestaurant, setOffersByRestaurant] = createSignal<
  Record<string, Offer[]>
>({});

/** Offer cart entries keyed by restaurant ID. */
const [offerCartsByRestaurant, setOfferCartsByRestaurant] = createSignal<
  Record<string, OfferCartEntry[]>
>({});

/** Cache of resolved allowed items per slot ID. */
const [allowedItemsBySlot, setAllowedItemsBySlot] = createSignal<
  Record<string, SlotAllowedItems>
>({});

const [offerLoading, setOfferLoading] = createSignal(false);
const [offerError, setOfferError] = createSignal<string | null>(null);

/** Counter for unique offer cart entry keys. */
let nextOfferCartKey = 1;

// ══════════════════════════════════════════════════════════════════
// API — Offer CRUD
// ══════════════════════════════════════════════════════════════════

/**
 * Fetch all offers for a restaurant and cache them.
 */
async function fetchOffers(restaurantId: string): Promise<Offer[]> {
  try {
    setOfferLoading(true);
    setOfferError(null);

    const res = await fetch(`/api/restaurants/${restaurantId}/offers`);
    const json: ApiResponse<Offer[]> = await res.json();

    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Failed to load offers (${res.status})`);
    }

    setOffersByRestaurant((prev) => ({
      ...prev,
      [restaurantId]: json.data!,
    }));

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] fetchOffers failed:", msg);
    return [];
  } finally {
    setOfferLoading(false);
  }
}

/**
 * Fetch only active offers for a restaurant.
 */
async function fetchActiveOffers(restaurantId: string): Promise<Offer[]> {
  try {
    setOfferLoading(true);
    setOfferError(null);

    const res = await fetch(
      `/api/restaurants/${restaurantId}/offers/active`,
    );
    const json: ApiResponse<Offer[]> = await res.json();

    if (!res.ok || !json.success || json.data == null) {
      throw new Error(
        json.error ?? `Failed to load active offers (${res.status})`,
      );
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] fetchActiveOffers failed:", msg);
    return [];
  } finally {
    setOfferLoading(false);
  }
}

/**
 * Fetch a single offer by ID.
 */
async function fetchOffer(offerId: string): Promise<Offer | null> {
  try {
    setOfferLoading(true);
    setOfferError(null);

    const res = await fetch(`/api/offers/${offerId}`);
    const json: ApiResponse<Offer> = await res.json();

    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Failed to load offer (${res.status})`);
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] fetchOffer failed:", msg);
    return null;
  } finally {
    setOfferLoading(false);
  }
}

/**
 * Create a new offer.
 */
async function createOffer(request: CreateOffer): Promise<Offer | null> {
  try {
    setOfferLoading(true);
    setOfferError(null);

    const res = await fetch("/api/offers", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<Offer> = await res.json();

    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Failed to create offer (${res.status})`);
    }

    // Update local cache
    const offer = json.data;
    setOffersByRestaurant((prev) => {
      const existing = prev[offer.restaurant_id] ?? [];
      return { ...prev, [offer.restaurant_id]: [...existing, offer] };
    });

    return offer;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] createOffer failed:", msg);
    return null;
  } finally {
    setOfferLoading(false);
  }
}

/**
 * Update an existing offer.
 */
async function updateOffer(request: UpdateOffer): Promise<Offer | null> {
  try {
    setOfferLoading(true);
    setOfferError(null);

    const res = await fetch("/api/update-offer", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<Offer> = await res.json();

    if (!res.ok || !json.success || json.data == null) {
      throw new Error(json.error ?? `Failed to update offer (${res.status})`);
    }

    const updated = json.data;
    setOffersByRestaurant((prev) => {
      const existing = prev[updated.restaurant_id] ?? [];
      return {
        ...prev,
        [updated.restaurant_id]: existing.map((o) =>
          o.id === updated.id ? updated : o,
        ),
      };
    });

    return updated;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] updateOffer failed:", msg);
    return null;
  } finally {
    setOfferLoading(false);
  }
}

/**
 * Delete an offer by ID.
 */
async function deleteOffer(
  offerId: string,
  restaurantId: string,
): Promise<boolean> {
  try {
    setOfferLoading(true);
    setOfferError(null);

    const res = await fetch(`/api/offers/${offerId}`, { method: "DELETE" });
    const json: ApiResponse<null> = await res.json();

    if (!res.ok || !json.success) {
      throw new Error(json.error ?? `Failed to delete offer (${res.status})`);
    }

    setOffersByRestaurant((prev) => {
      const existing = prev[restaurantId] ?? [];
      return {
        ...prev,
        [restaurantId]: existing.filter((o) => o.id !== offerId),
      };
    });

    return true;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] deleteOffer failed:", msg);
    return false;
  } finally {
    setOfferLoading(false);
  }
}

/**
 * Activate an offer.
 */
async function activateOffer(offerId: string): Promise<Offer | null> {
  return toggleOfferActive(offerId, true);
}

/**
 * Deactivate an offer.
 */
async function deactivateOffer(offerId: string): Promise<Offer | null> {
  return toggleOfferActive(offerId, false);
}

async function toggleOfferActive(
  offerId: string,
  activate: boolean,
): Promise<Offer | null> {
  try {
    setOfferLoading(true);
    setOfferError(null);

    const action = activate ? "activate" : "deactivate";
    const res = await fetch(`/api/offers/${offerId}/${action}`, {
      method: "POST",
    });
    const json: ApiResponse<Offer> = await res.json();

    if (!res.ok || !json.success || json.data == null) {
      throw new Error(
        json.error ?? `Failed to ${action} offer (${res.status})`,
      );
    }

    const updated = json.data;
    setOffersByRestaurant((prev) => {
      const existing = prev[updated.restaurant_id] ?? [];
      return {
        ...prev,
        [updated.restaurant_id]: existing.map((o) =>
          o.id === updated.id ? updated : o,
        ),
      };
    });

    return updated;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] toggleOfferActive failed:", msg);
    return null;
  } finally {
    setOfferLoading(false);
  }
}

// ══════════════════════════════════════════════════════════════════
// API — Slot Helpers
// ══════════════════════════════════════════════════════════════════

/**
 * Fetch the resolved allowed item IDs for a slot, then fetch the
 * full Item objects and constraint supplements. Results are cached.
 */
async function fetchAllowedItemsForSlot(
  slotId: string,
  restaurantId: string,
): Promise<SlotAllowedItems | null> {
  // Return from cache if available
  const cached = allowedItemsBySlot()[slotId];
  if (cached) return cached;

  try {
    // 1. Get allowed item IDs from the slot endpoint
    const idsRes = await fetch(`/api/offer-slots/${slotId}/allowed-items`);
    const idsJson: ApiResponse<string[]> = await idsRes.json();

    if (!idsRes.ok || !idsJson.success || idsJson.data == null) {
      throw new Error(
        idsJson.error ?? `Failed to load allowed items (${idsRes.status})`,
      );
    }

    const itemIds: string[] = idsJson.data;

    if (itemIds.length === 0) {
      const result: SlotAllowedItems = {
        slotId,
        items: [],
        supplements: {},
      };
      setAllowedItemsBySlot((prev) => ({ ...prev, [slotId]: result }));
      return result;
    }

    // 2. Fetch all items for the restaurant (we need the full Item objects).
    //    We use the restaurant items endpoint and filter by the allowed IDs.
    const itemsRes = await fetch(
      `/api/restaurants/${restaurantId}/items`,
    );
    const itemsJson: ApiResponse<Item[]> = await itemsRes.json();

    if (!itemsRes.ok || !itemsJson.success || itemsJson.data == null) {
      throw new Error(
        itemsJson.error ?? `Failed to load items (${itemsRes.status})`,
      );
    }

    const allowedSet = new Set(itemIds);
    const items = itemsJson.data.filter((item) => allowedSet.has(item.id));

    // 3. Build supplements map.
    //    We don't have a dedicated endpoint for per-item supplements in bulk,
    //    so we use the validate-selection endpoint with individual items to
    //    infer supplements. However, that's expensive. Instead, we'll compute
    //    supplements client-side from the offer slot constraints when possible,
    //    or just store 0 and let the validate-selection endpoint compute the
    //    real total at checkout time.
    //
    //    For now, we compute a basic supplements map: items that match
    //    constraints with non-zero supplements. The slot's constraint data
    //    is available on the Offer object, so callers can use
    //    `getSupplementForItem()` to look it up.
    const supplements: Record<string, number> = {};
    for (const id of itemIds) {
      supplements[id] = 0; // default; will be enriched by the UI
    }

    const result: SlotAllowedItems = { slotId, items, supplements };
    setAllowedItemsBySlot((prev) => ({ ...prev, [slotId]: result }));
    return result;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    console.error("[offerStore] fetchAllowedItemsForSlot failed:", msg);
    return null;
  }
}

/**
 * Invalidate the allowed-items cache for a slot (or all slots).
 */
function invalidateAllowedItemsCache(slotId?: string): void {
  if (slotId) {
    setAllowedItemsBySlot((prev) => {
      const next = { ...prev };
      delete next[slotId];
      return next;
    });
  } else {
    setAllowedItemsBySlot({});
  }
}

// ══════════════════════════════════════════════════════════════════
// API — Offer Validation
// ══════════════════════════════════════════════════════════════════

/**
 * Validate a set of offer selections and get the computed price.
 */
async function validateOfferSelection(
  offerId: string,
  restaurantId: string,
  selections: Array<{ item_id: string; slot_id: string }>,
): Promise<ValidateOfferSelectionResponse | null> {
  try {
    setOfferError(null);

    const request: ValidateOfferSelectionRequest = {
      restaurant_id: restaurantId,
      selections: selections.map((s) => ({
        item_id: s.item_id,
        slot_id: s.slot_id,
      })),
    };

    const res = await fetch(`/api/offers/${offerId}/validate-selection`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<ValidateOfferSelectionResponse> = await res.json();

    if (!res.ok || !json.success || json.data == null) {
      throw new Error(
        json.error ?? `Validation failed (${res.status})`,
      );
    }

    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setOfferError(msg);
    console.error("[offerStore] validateOfferSelection failed:", msg);
    return null;
  }
}

// ══════════════════════════════════════════════════════════════════
// Offer Cart
// ══════════════════════════════════════════════════════════════════

/**
 * Get the offer cart entries for a restaurant.
 */
function getOfferCart(restaurantId: string): OfferCartEntry[] {
  return offerCartsByRestaurant()[restaurantId] ?? [];
}

/**
 * Add a fully composed offer to the cart.
 */
function addOfferToCart(
  restaurantId: string,
  offer: Offer,
  selections: OfferSlotSelection[],
  totalPriceCents: number,
  basePriceCents: number,
  notes: string | null = null,
): void {
  const entry: OfferCartEntry = {
    key: nextOfferCartKey++,
    offer,
    selections,
    totalPriceCents,
    basePriceCents,
    notes,
  };

  setOfferCartsByRestaurant((prev) => {
    const existing = prev[restaurantId] ?? [];
    return { ...prev, [restaurantId]: [...existing, entry] };
  });
}

/**
 * Remove an offer entry from the cart by its key.
 */
function removeOfferFromCart(restaurantId: string, key: number): void {
  setOfferCartsByRestaurant((prev) => {
    const existing = prev[restaurantId] ?? [];
    return {
      ...prev,
      [restaurantId]: existing.filter((e) => e.key !== key),
    };
  });
}

/**
 * Clear all offer cart entries for a restaurant.
 */
function clearOfferCart(restaurantId: string): void {
  setOfferCartsByRestaurant((prev) => {
    const next = { ...prev };
    delete next[restaurantId];
    return next;
  });
}

/**
 * Get total price of all offer cart entries for a restaurant (cents).
 */
function getOfferCartTotal(restaurantId: string): number {
  const entries = getOfferCart(restaurantId);
  return entries.reduce((sum, e) => sum + e.totalPriceCents, 0);
}

/**
 * Get the count of offer entries in the cart.
 */
function getOfferCartCount(restaurantId: string): number {
  return (offerCartsByRestaurant()[restaurantId] ?? []).length;
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

/**
 * Get cached offers for a restaurant.
 */
function getOffers(restaurantId: string): Offer[] {
  return offersByRestaurant()[restaurantId] ?? [];
}

/**
 * Get the active offers for a restaurant from the cached list.
 */
function getActiveOffers(restaurantId: string): Offer[] {
  return (offersByRestaurant()[restaurantId] ?? []).filter((o) => o.is_active);
}

/**
 * Find offers linked to a specific menu.
 */
function getOffersForMenu(restaurantId: string, menuId: string): Offer[] {
  return (offersByRestaurant()[restaurantId] ?? []).filter(
    (o) => o.menu_id === menuId,
  );
}

/**
 * Determine the best supplement for an item in a slot, given the offer's
 * constraint data. When an item matches multiple constraints, the lowest
 * supplement is used (most favorable to the customer).
 *
 * This is a client-side approximation. The backend computes the authoritative
 * price via validate-selection / compute_offer_price.
 *
 * For Section constraints we can't resolve membership client-side (we don't
 * know which section an item belongs to without extra data), so we return
 * the constraint supplement only for Item and Tag matches.
 */
function getSupplementForItem(
  slot: OfferSlot,
  item: Item,
): number {
  let best: number | null = null;

  for (const constraint of slot.constraints) {
    let matches = false;
    const kind = constraint.kind;

    if ("Item" in kind) {
      matches = kind.Item === item.id;
    } else if ("Tag" in kind) {
      matches = item.tags.some((t) => t.id === (kind as { Tag: string }).Tag);
    } else if ("Section" in kind) {
      // We can't fully resolve Section membership client-side without
      // knowing which section the item is in. We'll optimistically match
      // all items here (the backend allowed-items endpoint already filtered
      // to only valid items). This means for Section constraints we just
      // assume the item matches and use the supplement.
      matches = true;
    }

    if (matches) {
      if (best === null || constraint.supplement_cents < best) {
        best = constraint.supplement_cents;
      }
    }
  }

  return best ?? 0;
}

/**
 * Format a price in cents to a display string (e.g. 1250 → "12.50").
 */
function formatOfferPrice(cents: number): string {
  return (cents / 100).toFixed(2);
}

/**
 * Clear the offer error.
 */
function clearOfferError(): void {
  setOfferError(null);
}

// ══════════════════════════════════════════════════════════════════
// Exports
// ══════════════════════════════════════════════════════════════════

export {
  // State (read-only signals)
  offerLoading,
  offerError,
  offersByRestaurant,
  offerCartsByRestaurant,
  allowedItemsBySlot,

  // API — CRUD
  fetchOffers,
  fetchActiveOffers,
  fetchOffer,
  createOffer,
  updateOffer,
  deleteOffer,
  activateOffer,
  deactivateOffer,

  // API — Slot helpers
  fetchAllowedItemsForSlot,
  invalidateAllowedItemsCache,

  // API — Validation
  validateOfferSelection,

  // Offer Cart
  getOfferCart,
  addOfferToCart,
  removeOfferFromCart,
  clearOfferCart,
  getOfferCartTotal,
  getOfferCartCount,

  // Helpers
  getOffers,
  getActiveOffers,
  getOffersForMenu,
  getSupplementForItem,
  formatOfferPrice,
  clearOfferError,
};