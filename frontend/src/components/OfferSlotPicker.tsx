import { Show, For, createSignal, createEffect, createMemo, on, onMount } from "solid-js";
import type { Offer } from "@bindings/Offer";
import type { OfferSlot } from "@bindings/OfferSlot";
import type { Item } from "@bindings/Item";
import OfferSlotItemCard from "@/components/OfferSlotItemCard";
import {
  fetchAllowedItemsForSlot,
  validateOfferSelection,
  addOfferToCart,
  getSupplementForItem,
  formatOfferPrice,
  invalidateAllowedItemsCache,
  type OfferSlotSelection,
  type SlotAllowedItems,
} from "@/stores/offerStore";

interface OfferSlotPickerProps {
  offer: Offer;
  restaurantId: string;
  /** Called when the user finishes composing (or cancels). */
  onClose: () => void;
  /** Called after the offer is successfully added to the cart. */
  onAdded?: () => void;
}

/** Per-slot selection state: map of item_id → quantity. */
type SlotSelectionMap = Record<string, number>;

export default function OfferSlotPicker(props: OfferSlotPickerProps) {
  const slots = () =>
    [...props.offer.slots].sort((a, b) => a.label.localeCompare(b.label));

  // ── Navigation ─────────────────────────────────────────────────
  const [currentStepIndex, setCurrentStepIndex] = createSignal(0);
  const totalSteps = () => slots().length;
  const currentSlot = (): OfferSlot | undefined => slots()[currentStepIndex()];
  const isFirstStep = () => currentStepIndex() === 0;
  const isLastStep = () => currentStepIndex() === totalSteps() - 1;

  // ── Per-slot selections (index → SlotSelectionMap) ─────────────
  const [selectionsBySlotIndex, setSelectionsBySlotIndex] = createSignal<
    Record<number, SlotSelectionMap>
  >({});

  const currentSelections = (): SlotSelectionMap =>
    selectionsBySlotIndex()[currentStepIndex()] ?? {};

  const currentSelectionCount = (): number => {
    const sel = currentSelections();
    return Object.values(sel).reduce((sum, qty) => sum + qty, 0);
  };

  // ── Allowed items loading ──────────────────────────────────────
  const [slotAllowedItems, setSlotAllowedItems] = createSignal<
    Record<string, SlotAllowedItems>
  >({});
  const [slotLoading, setSlotLoading] = createSignal(false);

  // Preload allowed items for all slots on mount
  onMount(async () => {
    // Invalidate cache so we get fresh data
    invalidateAllowedItemsCache();

    for (const slot of slots()) {
      setSlotLoading(true);
      const result = await fetchAllowedItemsForSlot(
        slot.id,
        props.restaurantId,
      );
      if (result) {
        setSlotAllowedItems((prev) => ({ ...prev, [slot.id]: result }));
      }
    }
    setSlotLoading(false);
  });

  const currentAllowedItems = (): Item[] => {
    const slot = currentSlot();
    if (!slot) return [];
    const data = slotAllowedItems()[slot.id];
    return data?.items ?? [];
  };

  // ── Supplement resolution ──────────────────────────────────────
  const getItemSupplement = (item: Item): number => {
    const slot = currentSlot();
    if (!slot) return 0;
    return getSupplementForItem(slot, item);
  };

  // ── Validation state ───────────────────────────────────────────
  const [validating, setValidating] = createSignal(false);
  const [validationError, setValidationError] = createSignal<string | null>(
    null,
  );
  const [computedPrice, setComputedPrice] = createSignal<number | null>(null);

  // Clear validation error when step changes
  createEffect(
    on(currentStepIndex, () => {
      setValidationError(null);
    }),
  );

  // ── Slot completion check ──────────────────────────────────────
  const isSlotComplete = (slotIndex: number): boolean => {
    const slot = slots()[slotIndex];
    if (!slot) return false;
    const sel = selectionsBySlotIndex()[slotIndex] ?? {};
    const count = Object.values(sel).reduce((sum, qty) => sum + qty, 0);
    return count >= slot.min_items;
  };

  const isCurrentSlotComplete = () => isSlotComplete(currentStepIndex());

  const allSlotsComplete = createMemo(() => {
    for (let i = 0; i < totalSteps(); i++) {
      if (!isSlotComplete(i)) return false;
    }
    return true;
  });

  const currentSlotFull = (): boolean => {
    const slot = currentSlot();
    if (!slot) return true;
    return currentSelectionCount() >= slot.max_items;
  };

  // ── Selection actions ──────────────────────────────────────────
  const addItem = (itemId: string) => {
    if (currentSlotFull()) return;
    setSelectionsBySlotIndex((prev) => {
      const idx = currentStepIndex();
      const current = { ...(prev[idx] ?? {}) };
      current[itemId] = (current[itemId] ?? 0) + 1;
      return { ...prev, [idx]: current };
    });
  };

  const removeItem = (itemId: string) => {
    setSelectionsBySlotIndex((prev) => {
      const idx = currentStepIndex();
      const current = { ...(prev[idx] ?? {}) };
      const qty = current[itemId] ?? 0;
      if (qty <= 1) {
        delete current[itemId];
      } else {
        current[itemId] = qty - 1;
      }
      return { ...prev, [idx]: current };
    });
  };

  const getItemQuantity = (itemId: string): number =>
    currentSelections()[itemId] ?? 0;

  // ── Navigation actions ─────────────────────────────────────────
  const goNext = () => {
    if (!isLastStep()) {
      setCurrentStepIndex((i) => i + 1);
    }
  };

  const goPrev = () => {
    if (!isFirstStep()) {
      setCurrentStepIndex((i) => i - 1);
    }
  };

  const goToStep = (index: number) => {
    if (index >= 0 && index < totalSteps()) {
      setCurrentStepIndex(index);
    }
  };

  // ── Build selections for API ───────────────────────────────────
  const buildApiSelections = (): Array<{
    item_id: string;
    slot_id: string;
  }> => {
    const result: Array<{ item_id: string; slot_id: string }> = [];
    const allSlots = slots();

    for (let i = 0; i < allSlots.length; i++) {
      const slot = allSlots[i];
      const sel = selectionsBySlotIndex()[i] ?? {};
      for (const [itemId, qty] of Object.entries(sel)) {
        for (let q = 0; q < qty; q++) {
          result.push({ item_id: itemId, slot_id: slot.id });
        }
      }
    }

    return result;
  };

  // ── Build OfferSlotSelection[] for the cart ────────────────────
  const buildCartSelections = (): OfferSlotSelection[] => {
    const result: OfferSlotSelection[] = [];
    const allSlots = slots();

    for (let i = 0; i < allSlots.length; i++) {
      const slot = allSlots[i];
      const sel = selectionsBySlotIndex()[i] ?? {};
      const allowed = slotAllowedItems()[slot.id]?.items ?? [];
      const itemMap = new Map(allowed.map((item) => [item.id, item]));

      for (const [itemId, qty] of Object.entries(sel)) {
        const item = itemMap.get(itemId);
        if (!item) continue;
        const supplement = getSupplementForItem(slot, item);
        for (let q = 0; q < qty; q++) {
          result.push({
            slotId: slot.id,
            item,
            supplementCents: supplement,
          });
        }
      }
    }

    return result;
  };

  // ── Validate & add to cart ─────────────────────────────────────
  const handleAddToCart = async () => {
    setValidationError(null);

    if (!allSlotsComplete()) {
      setValidationError("Please complete all required slots before adding.");
      return;
    }

    setValidating(true);
    try {
      const selections = buildApiSelections();
      const result = await validateOfferSelection(
        props.offer.id,
        props.restaurantId,
        selections,
      );

      if (!result) {
        setValidationError(
          "Could not validate your selections. Please try again.",
        );
        return;
      }

      if (!result.valid) {
        setValidationError("Your selections are not valid for this offer.");
        return;
      }

      // Add to offer cart
      addOfferToCart(
        props.restaurantId,
        props.offer,
        buildCartSelections(),
        result.total_price_cents,
        result.base_price_cents,
      );

      props.onAdded?.();
      props.onClose();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setValidationError(msg);
    } finally {
      setValidating(false);
    }
  };

  // ── Live price preview (debounced) ─────────────────────────────
  // We recompute whenever selections change, but only if all slots are complete
  createEffect(
    on(
      () => JSON.stringify(selectionsBySlotIndex()),
      async () => {
        if (!allSlotsComplete()) {
          setComputedPrice(null);
          return;
        }

        const selections = buildApiSelections();
        if (selections.length === 0) {
          setComputedPrice(props.offer.base_price_cents);
          return;
        }

        const result = await validateOfferSelection(
          props.offer.id,
          props.restaurantId,
          selections,
        );

        if (result && result.valid) {
          setComputedPrice(result.total_price_cents);
        }
      },
    ),
  );

  // ── Running total approximation ────────────────────────────────
  const approximateTotal = createMemo(() => {
    let total = props.offer.base_price_cents;
    const allSlots = slots();

    for (let i = 0; i < allSlots.length; i++) {
      const slot = allSlots[i];
      const sel = selectionsBySlotIndex()[i] ?? {};
      const count = Object.values(sel).reduce((sum, qty) => sum + qty, 0);

      if (count > 0) {
        total += slot.supplement_cents;
      }

      // Add constraint supplements
      const allowed = slotAllowedItems()[slot.id]?.items ?? [];
      const itemMap = new Map(allowed.map((item) => [item.id, item]));

      for (const [itemId, qty] of Object.entries(sel)) {
        const item = itemMap.get(itemId);
        if (item) {
          total += getSupplementForItem(slot, item) * qty;
        }
      }
    }

    return total;
  });

  // Use server-computed price when available, otherwise approximation
  const displayTotal = () => computedPrice() ?? approximateTotal();

  // ── Render ─────────────────────────────────────────────────────
  return (
    <div class="box" style={{ "max-width": "700px", margin: "0 auto" }}>
      {/* Header */}
      <div class="is-flex is-justify-content-space-between is-align-items-center mb-4">
        <div>
          <h3 class="title is-4 mb-1">
            <span class="mr-2">🍽️</span>
            {props.offer.title}
          </h3>
          <Show when={props.offer.description}>
            <p class="has-text-grey is-size-6">{props.offer.description}</p>
          </Show>
        </div>
        <button
          class="delete is-medium"
          onClick={props.onClose}
          title="Close"
        />
      </div>

      {/* Step indicator / breadcrumb */}
      <div class="mb-4">
        <div
          class="is-flex is-align-items-center is-flex-wrap-wrap"
          style={{ gap: "0.25rem" }}
        >
          <For each={slots()}>
            {(slot, index) => {
              const isActive = () => currentStepIndex() === index();
              const isComplete = () => isSlotComplete(index());
              const isOptional = () => slot.min_items === 0;

              return (
                <>
                  <Show when={index() > 0}>
                    <span
                      class="has-text-grey mx-1"
                      style={{ "font-size": "0.7rem" }}
                    >
                      ›
                    </span>
                  </Show>
                  <button
                    class={`tag is-medium ${
                      isActive()
                        ? "is-primary"
                        : isComplete()
                          ? "is-success"
                          : ""
                    }`}
                    style={{ cursor: "pointer", transition: "all 0.15s ease" }}
                    onClick={() => goToStep(index())}
                  >
                    <Show when={isComplete() && !isActive()}>
                      <span class="mr-1">✓</span>
                    </Show>
                    {slot.label}
                    <Show when={isOptional()}>
                      <span
                        class="ml-1"
                        style={{ "font-size": "0.65rem" }}
                      >
                        opt.
                      </span>
                    </Show>
                  </button>
                </>
              );
            }}
          </For>
        </div>

        {/* Progress bar */}
        <progress
          class="progress is-primary is-small mt-2"
          value={currentStepIndex() + (isCurrentSlotComplete() ? 1 : 0)}
          max={totalSteps()}
        />
      </div>

      {/* Current slot */}
      <Show when={currentSlot()}>
        {(slot) => {
          const isOptional = () => slot().min_items === 0;

          return (
            <div>
              {/* Slot header */}
              <div class="mb-3">
                <div class="is-flex is-justify-content-space-between is-align-items-center">
                  <div>
                    <h4 class="title is-5 mb-0">{slot().label}</h4>
                    <p class="has-text-grey is-size-7 mt-1">
                      <Show
                        when={slot().min_items === slot().max_items}
                        fallback={
                          <>
                            Pick {slot().min_items}–{slot().max_items} item
                            {slot().max_items !== 1 ? "s" : ""}
                          </>
                        }
                      >
                        Pick {slot().max_items} item
                        {slot().max_items !== 1 ? "s" : ""}
                      </Show>

                      <Show when={isOptional()}>
                        <span class="ml-1">(optional)</span>
                      </Show>
                    </p>
                  </div>

                  <div class="has-text-right">
                    {/* Selection counter */}
                    <span
                      class={`tag ${
                        isCurrentSlotComplete()
                          ? "is-success"
                          : currentSelectionCount() > 0
                            ? "is-warning"
                            : ""
                      }`}
                    >
                      {currentSelectionCount()} / {slot().max_items}
                    </span>

                    {/* Slot supplement hint */}
                    <Show when={slot().supplement_cents > 0}>
                      <p class="is-size-7 has-text-grey mt-1">
                        Slot: +${formatOfferPrice(slot().supplement_cents)}
                      </p>
                    </Show>
                  </div>
                </div>
              </div>

              {/* Loading */}
              <Show when={slotLoading() && currentAllowedItems().length === 0}>
                <div class="has-text-centered py-4">
                  <progress
                    class="progress is-primary is-small"
                    max="100"
                  />
                  <p class="has-text-grey is-size-7 mt-1">
                    Loading available items…
                  </p>
                </div>
              </Show>

              {/* Items list */}
              <Show when={currentAllowedItems().length > 0}>
                <div
                  style={{
                    "max-height": "400px",
                    "overflow-y": "auto",
                    "padding-right": "4px",
                  }}
                >
                  <For each={currentAllowedItems()}>
                    {(item) => (
                      <OfferSlotItemCard
                        item={item}
                        supplementCents={getItemSupplement(item)}
                        quantity={getItemQuantity(item.id)}
                        slotFull={currentSlotFull()}
                        onAdd={() => addItem(item.id)}
                        onRemove={() => removeItem(item.id)}
                      />
                    )}
                  </For>
                </div>
              </Show>

              {/* No items */}
              <Show
                when={
                  !slotLoading() && currentAllowedItems().length === 0
                }
              >
                <div class="notification is-warning has-text-centered">
                  <p>No items available for this slot right now.</p>
                </div>
              </Show>
            </div>
          );
        }}
      </Show>

      {/* Validation error */}
      <Show when={validationError()}>
        <div class="notification is-danger mt-3">
          <button
            class="delete"
            onClick={() => setValidationError(null)}
          />
          {validationError()}
        </div>
      </Show>

      {/* Price preview */}
      <div
        class="is-flex is-justify-content-space-between is-align-items-center mt-4 pt-3"
        style={{ "border-top": "2px solid var(--bulma-border)" }}
      >
        <div>
          <span class="has-text-grey is-size-7">Estimated total</span>
          <p class="has-text-weight-bold is-size-5">
            ${formatOfferPrice(displayTotal())}
          </p>
          <Show when={displayTotal() > props.offer.base_price_cents}>
            <p class="is-size-7 has-text-grey">
              Base: ${formatOfferPrice(props.offer.base_price_cents)} +
              supplements
            </p>
          </Show>
        </div>
      </div>

      {/* Navigation buttons */}
      <div class="is-flex is-justify-content-space-between is-align-items-center mt-4">
        <div class="buttons">
          <button
            class="button"
            disabled={isFirstStep()}
            onClick={goPrev}
          >
            <span class="icon is-small">
              <span>←</span>
            </span>
            <span>Previous</span>
          </button>
        </div>

        <div class="buttons">
          {/* Skip (optional slots) */}
          <Show when={currentSlot()?.min_items === 0 && !isLastStep()}>
            <button class="button" onClick={goNext}>
              Skip
            </button>
          </Show>

          <Show
            when={isLastStep()}
            fallback={
              <button
                class="button is-primary"
                disabled={
                  !isCurrentSlotComplete() &&
                  (currentSlot()?.min_items ?? 0) > 0
                }
                onClick={goNext}
              >
                <span>Next</span>
                <span class="icon is-small">
                  <span>→</span>
                </span>
              </button>
            }
          >
            {/* Add to cart (last step) */}
            <button
              class="button is-primary is-medium"
              classList={{ "is-loading": validating() }}
              disabled={!allSlotsComplete() || validating()}
              onClick={handleAddToCart}
            >
              <span class="icon is-small">
                <span>🛒</span>
              </span>
              <span>
                Add to Cart — ${formatOfferPrice(displayTotal())}
              </span>
            </button>
          </Show>
        </div>
      </div>

      {/* Cancel link */}
      <div class="has-text-centered mt-3">
        <button
          class="button is-text is-small"
          onClick={props.onClose}
        >
          Cancel
        </button>
      </div>
    </div>
  );
}