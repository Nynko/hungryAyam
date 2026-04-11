import { Show, For, Index, createSignal, createEffect, onMount, onCleanup, createMemo } from "solid-js";
import { createStore, produce } from "solid-js/store";
import type { Offer } from "@bindings/Offer";
import type { CreateOffer } from "@bindings/CreateOffer";
import type { UpdateOffer } from "@bindings/UpdateOffer";
import type { Menu } from "@bindings/Menu";
import { Card } from "@/components/Card";
import AvailabilityRuleEditor from "@/components/AvailabilityRuleEditor";
import SlotFormFields from "@/components/SlotFormFields";
import {
  fetchOffers,
  getOffers,
  createOffer,
  updateOffer,
  deleteOffer as deleteOfferApi,
  activateOffer,
  deactivateOffer,
  formatOfferPrice,
  offerLoading,
  offerError,
  clearOfferError,
} from "@/stores/offerStore";
import { showConfirm } from "@/stores/confirmStore";
import { setupSortableItem, setupSortableMonitor, computeReorderIndex, extractClosestEdge } from "@/lib/dnd";
import type { SortableItemState } from "@/lib/dnd";
import DropIndicator from "@/components/menu-editor/DropIndicator";
import {
  type DraftOffer,
  emptyDraftOffer,
  offerToDraft,
  emptySlot,
  emptyConstraint,
  centsToDollars,
  dollarsToCents,
  constraintKindKey,
  constraintKindValue,
  flattenSectionsFromMenus,
  buildCreateSlots,
  validateDraft,
} from "@/lib/offerDraft";

// ══════════════════════════════════════════════════════════════════
// Component
// ══════════════════════════════════════════════════════════════════

interface OffersManagerProps {
  restaurantId: string;
  /** All menus for this restaurant (used for Section constraint picker). */
  menus: Menu[];
}

export default function OffersManager(props: OffersManagerProps) {
  // ── State ──────────────────────────────────────────────────────
  const [loaded, setLoaded] = createSignal(false);
  const [editingOfferId, setEditingOfferId] = createSignal<string | null>(null);
  const [creating, setCreating] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [localError, setLocalError] = createSignal<string | null>(null);
  const [localSuccess, setLocalSuccess] = createSignal<string | null>(null);
  const [collapsed, setCollapsed] = createSignal(false);

  // Use createStore for fine-grained reactivity — updating one slot's
  // label won't recreate sibling DOM nodes and lose input focus.
  const [draft, setDraft] = createStore<DraftOffer>(emptyDraftOffer());

  // ── Derived ────────────────────────────────────────────────────
  const offers = () => getOffers(props.restaurantId);
  const isEditing = () => editingOfferId() !== null || creating();

  const allSections = createMemo(() => flattenSectionsFromMenus(props.menus));

  const menuNameById = (menuId: string): string => {
    const menu = props.menus.find((m) => m.id === menuId);
    return menu?.name ?? "Unknown menu";
  };

  const sectionNameById = (sectionId: string): string => {
    const section = allSections().find((s) => s.id === sectionId);
    if (section) return `${section.name} (${section.menuName})`;
    return sectionId.slice(0, 8) + "…";
  };

  // ── Load offers on mount ───────────────────────────────────────
  onMount(async () => {
    await fetchOffers(props.restaurantId);
    setLoaded(true);
  });

  // ── Draft manipulation (via produce for fine-grained updates) ──

  const resetDraft = (d: DraftOffer) => {
    setDraft(d);
  };

  const addSlot = () => {
    setDraft(
      produce((d) => {
        d.slots.push(emptySlot());
      }),
    );
  };

  const removeSlot = (slotIndex: number) => {
    setDraft(
      produce((d) => {
        d.slots.splice(slotIndex, 1);
      }),
    );
  };

  const moveSlot = (from: number, to: number) => {
    if (to < 0 || to >= draft.slots.length) return;
    setDraft(
      produce((d) => {
        const [moved] = d.slots.splice(from, 1);
        d.slots.splice(to, 0, moved);
      }),
    );
  };

  // ── Slot DnD monitor ──────────────────────────────────────────
  onMount(() => {
    const cleanup = setupSortableMonitor({
      type: "slot",
      onReorder: (_sourceId, sourceIndex, destinationIndex) => {
        moveSlot(sourceIndex, destinationIndex);
      },
    });
    onCleanup(cleanup);
  });

  const addConstraint = (slotIndex: number) => {
    setDraft(
      produce((d) => {
        d.slots[slotIndex].constraints.push(emptyConstraint());
      }),
    );
  };

  const removeConstraint = (slotIndex: number, constraintIndex: number) => {
    setDraft(
      produce((d) => {
        d.slots[slotIndex].constraints.splice(constraintIndex, 1);
      }),
    );
  };

  // ── Actions ────────────────────────────────────────────────────

  const startCreating = () => {
    resetDraft(emptyDraftOffer());
    setCreating(true);
    setEditingOfferId(null);
    setLocalError(null);
    setLocalSuccess(null);
  };

  const startEditing = (offer: Offer) => {
    resetDraft(offerToDraft(offer));
    setEditingOfferId(offer.id);
    setCreating(false);
    setLocalError(null);
    setLocalSuccess(null);
  };

  const cancelEditing = () => {
    resetDraft(emptyDraftOffer());
    setEditingOfferId(null);
    setCreating(false);
    setLocalError(null);
  };

  const handleSave = async () => {
    setLocalError(null);
    setLocalSuccess(null);

    const err = validateDraft(draft);
    if (err) {
      setLocalError(err);
      return;
    }

    setSaving(true);
    try {
      if (creating()) {
        const request: CreateOffer = {
          restaurant_id: props.restaurantId,
          menu_id: draft.menuId,
          title: draft.title,
          description: draft.description || null,
          base_price_cents: dollarsToCents(draft.basePriceDisplay),
          is_active: draft.isActive,
          slots: buildCreateSlots(draft.slots),
        };

        const result = await createOffer(request);
        if (result) {
          setCreating(false);
          resetDraft(emptyDraftOffer());
          setLocalSuccess(`Offer "${result.title}" created.`);
        }
      } else if (editingOfferId()) {
        const request: UpdateOffer = {
          id: editingOfferId()!,
          menu_id: draft.menuId,
          title: draft.title,
          description: draft.description || null,
          base_price_cents: dollarsToCents(draft.basePriceDisplay),
          is_active: draft.isActive,
          slots: buildCreateSlots(draft.slots).map((s) => ({
            label: s.label,
            min_items: s.min_items,
            max_items: s.max_items,
            supplement_cents: s.supplement_cents,
            slot_group: s.slot_group,
            constraints: s.constraints.map((c) => ({
              kind: c.kind,
              supplement_cents: c.supplement_cents,
            })),
          })),
        };

        const result = await updateOffer(request);
        if (result) {
          setEditingOfferId(null);
          resetDraft(emptyDraftOffer());
          setLocalSuccess(`Offer "${result.title}" updated.`);
        }
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setLocalError(msg);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (offer: Offer) => {
    const confirmed = await showConfirm({
      title: "Delete offer?",
      message: `This will permanently delete "${offer.title}" and all its slots. This cannot be undone.`,
      confirmText: "Delete Offer",
      cancelText: "Cancel",
      danger: true,
    });
    if (!confirmed) return;

    setSaving(true);
    try {
      const ok = await deleteOfferApi(offer.id, props.restaurantId);
      if (ok) {
        if (editingOfferId() === offer.id) {
          cancelEditing();
        }
        setLocalSuccess(`Offer "${offer.title}" deleted.`);
      }
    } finally {
      setSaving(false);
    }
  };

  const handleToggleActive = async (offer: Offer) => {
    setSaving(true);
    try {
      const result = offer.is_active
        ? await deactivateOffer(offer.id)
        : await activateOffer(offer.id);

      if (result) {
        setLocalSuccess(
          result.is_active
            ? `"${result.title}" activated.`
            : `"${result.title}" deactivated.`,
        );
      }
    } finally {
      setSaving(false);
    }
  };

  // ── Offer form renderer (shared between create and edit) ───────
  const renderOfferForm = () => {
    return (
      <div class="box mb-4 editor-panel">
        <div class="is-flex is-justify-content-space-between is-align-items-center mb-3">
          <h4 class="title is-5 mb-0">
            {creating() ? "➕ New Offer" : "✏️ Edit Offer"}
          </h4>
          <button class="delete" title="Cancel" onClick={cancelEditing} />
        </div>

        {/* Title & Base price */}
        <div class="columns is-multiline">
          <div class="column is-5">
            <div class="field">
              <label class="label is-small">Title *</label>
              <div class="control">
                <input
                  class="input is-small"
                  type="text"
                  placeholder="e.g. Happy Hour Combo"
                  value={draft.title}
                  onInput={(e) =>
                    setDraft("title", e.currentTarget.value)
                  }
                />
              </div>
            </div>
          </div>

          <div class="column is-3">
            <div class="field">
              <label class="label is-small">Base Price (€) *</label>
              <div class="control">
                <input
                  class="input is-small"
                  type="text"
                  inputmode="decimal"
                  placeholder="0.00"
                  value={draft.basePriceDisplay}
                  onInput={(e) =>
                    setDraft("basePriceDisplay", e.currentTarget.value)
                  }
                  onBlur={(e) => {
                    // Normalize on blur: "12,5" → "12.50"
                    const cents = dollarsToCents(e.currentTarget.value);
                    setDraft("basePriceDisplay", centsToDollars(cents));
                  }}
                />
              </div>
              <p class="help">
                = {dollarsToCents(draft.basePriceDisplay)} cents
              </p>
            </div>
          </div>

          <div class="column is-2">
            <div class="field">
              <label class="label is-small">Status</label>
              <div class="control">
                <label class="checkbox">
                  <input
                    type="checkbox"
                    checked={draft.isActive}
                    onChange={(e) =>
                      setDraft("isActive", e.currentTarget.checked)
                    }
                  />{" "}
                  Active
                </label>
              </div>
            </div>
          </div>

          <div class="column is-2">
            <div class="field">
              <label class="label is-small">Linked Menu</label>
              <div class="select is-small is-fullwidth">
                <select
                  value={draft.menuId ?? ""}
                  onChange={(e) =>
                    setDraft("menuId", e.currentTarget.value || null)
                  }
                >
                  <option value="">— None (standalone) —</option>
                  <For each={props.menus}>
                    {(menu) => (
                      <option value={menu.id}>
                        {menu.name}
                        {!menu.permanent ? " (rotating)" : ""}
                      </option>
                    )}
                  </For>
                </select>
              </div>
              <p class="help has-text-grey">Optional</p>
            </div>
          </div>

          <div class="column is-12">
            <div class="field">
              <label class="label is-small">Description</label>
              <div class="control">
                <input
                  class="input is-small"
                  type="text"
                  placeholder="Optional description"
                  value={draft.description}
                  onInput={(e) =>
                    setDraft("description", e.currentTarget.value)
                  }
                />
              </div>
            </div>
          </div>
        </div>

        {/* ── Slots ──────────────────────────────────────────── */}
        <div class="mb-4">
          <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
            <h5 class="title is-6 mb-0">Slots ({draft.slots.length})</h5>
            <button
              class="button is-small is-primary is-outlined"
              onClick={addSlot}
            >
              <span class="icon is-small">
                <span>➕</span>
              </span>
              <span>Add Slot</span>
            </button>
          </div>

          <Show when={draft.slots.length === 0}>
            <div class="notification has-text-centered py-3">
              <p class="has-text-grey is-size-7">
                No slots yet. Add at least one (e.g. "Starter", "Main",
                "Drink").
              </p>
            </div>
          </Show>

          {/* Index keyed by position — stable DOM, no focus loss */}
          <Index each={draft.slots}>
            {(slot, slotIndex) => {
              let slotContainerRef!: HTMLDivElement;
              let slotHandleRef!: HTMLSpanElement;

              const [slotIsDragging, setSlotIsDragging] = createSignal(false);
              const [slotClosestEdge, setSlotClosestEdge] = createSignal<ReturnType<SortableItemState["closestEdge"]>>(null);

              createEffect(() => {
                const el = slotContainerRef;
                const handle = slotHandleRef;
                if (!el || !handle) return;

                const state = setupSortableItem({
                  element: el,
                  dragHandle: handle,
                  getData: () => ({
                    type: "slot" as const,
                    id: slot().tempId,
                    index: slotIndex,
                  }),
                  acceptType: "slot",
                });

                createEffect(() => setSlotIsDragging(state.isDragging()));
                createEffect(() => setSlotClosestEdge(state.closestEdge()));

                onCleanup(state.cleanup);
              });

              return (
              <div
                ref={(el) => { slotContainerRef = el; }}
                class="box p-3 mb-3"
                style={{
                  position: "relative",
                  "border-left": "3px solid var(--bulma-border)",
                  opacity: slotIsDragging() ? "0.4" : "1",
                }}
              >
                <DropIndicator edge={slotClosestEdge()} gap="0.75rem" />
                {/* Slot header */}
                <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
                  <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
                    <span
                      ref={(el) => { slotHandleRef = el; }}
                      class="drag-handle has-text-grey mr-1"
                      style={{ cursor: "grab", "font-size": "1.1rem", "user-select": "none", "line-height": "1" }}
                      title="Drag to reorder"
                    >
                      ☰
                    </span>
                    <span class="has-text-weight-semibold is-size-6">
                      Slot {slotIndex + 1}
                      <Show when={slot().label}>
                        <span class="has-text-grey has-text-weight-normal ml-1">
                          — {slot().label}
                        </span>
                      </Show>
                    </span>
                  </div>
                  <button
                    class="delete is-small"
                    title="Remove slot"
                    onClick={() => removeSlot(slotIndex)}
                  />
                </div>

                <SlotFormFields
                  slot={slot()}
                  slotIndex={slotIndex}
                  setDraft={setDraft}
                  menus={props.menus}
                  onAddConstraint={() => addConstraint(slotIndex)}
                  onRemoveConstraint={(cIndex) => removeConstraint(slotIndex, cIndex)}
                />
              </div>
              );
            }}
          </Index>
        </div>

        {/* ── Form actions ────────────────────────────────────── */}
        <hr class="my-3" />
        <div class="buttons">
          <button
            class="button is-primary is-small"
            classList={{ "is-loading": saving() }}
            disabled={saving()}
            onClick={handleSave}
          >
            <span class="icon is-small">
              <span>💾</span>
            </span>
            <span>{creating() ? "Create Offer" : "Save Changes"}</span>
          </button>

          <button
            class="button is-small"
            disabled={saving()}
            onClick={cancelEditing}
          >
            Cancel
          </button>
        </div>
      </div>
    );
  };

  // ── Render: offer list row ─────────────────────────────────────
  const renderOfferRow = (offer: Offer) => {
    const isBeingEdited = () => editingOfferId() === offer.id;
    const slotsSummary = offer.slots
      .map((slot) => {
        const label = slot.label;
        if (slot.min_items === 0) return `${label} (opt.)`;
        if (slot.min_items === slot.max_items) {
          return slot.max_items === 1 ? label : `${slot.max_items}× ${label}`;
        }
        return `${slot.min_items}–${slot.max_items}× ${label}`;
      })
      .join(" + ");

    return (
      <div class="box p-3 mb-3">
        <div
          class="is-flex is-justify-content-space-between is-align-items-flex-start is-flex-wrap-wrap"
          style={{ gap: "0.5rem" }}
        >
          {/* Left: info */}
          <div style={{ flex: "1", "min-width": "200px" }}>
            <div
              class="is-flex is-align-items-center"
              style={{ gap: "0.5rem" }}
            >
              <span class="has-text-weight-bold is-size-6">
                {offer.title}
              </span>
              <span
                class={`tag is-small ${offer.is_active ? "is-success" : "is-warning"}`}
              >
                {offer.is_active ? "Active" : "Inactive"}
              </span>
              <span class="tag is-info is-small">
                €{formatOfferPrice(offer.base_price_cents)}
              </span>
              <Show when={offer.menu_id}>
                <span
                  class="tag is-small"
                  title={`Linked to menu: ${menuNameById(offer.menu_id!)}`}
                >
                  📋 {menuNameById(offer.menu_id!)}
                </span>
              </Show>
              <Show when={!offer.menu_id}>
                <span class="tag is-small" title="Standalone offer">
                  🌐 Standalone
                </span>
              </Show>
            </div>

            <Show when={offer.description}>
              <p class="has-text-grey is-size-7 mt-1">{offer.description}</p>
            </Show>

            <p class="is-size-7 has-text-grey mt-1">
              {offer.slots.length} slot{offer.slots.length !== 1 ? "s" : ""}
              {slotsSummary ? ` — ${slotsSummary}` : ""}
            </p>

            {/* Constraints summary */}
            <div class="mt-1">
              <For each={offer.slots}>
                {(slot) => (
                  <div
                    class="is-size-7 has-text-grey"
                    style={{ "padding-left": "0.5rem" }}
                  >
                    <span class="has-text-weight-medium">{slot.label}:</span>{" "}
                    <For each={slot.constraints}>
                      {(c, idx) => {
                        const kk = constraintKindKey(c.kind);
                        const kv = constraintKindValue(c.kind);
                        const display =
                          kk === "Section"
                            ? sectionNameById(kv)
                            : `${kk}:${kv.slice(0, 8)}…`;
                        return (
                          <>
                            <Show when={idx() > 0}>
                              <span class="mx-1">|</span>
                            </Show>
                            <span
                              class="tag mr-1"
                              style={{
                                "font-size": "0.55rem",
                                "vertical-align": "middle",
                              }}
                            >
                              {kk}
                            </span>
                            <span>{display}</span>
                            <Show when={c.supplement_cents > 0}>
                              <span class="has-text-warning-dark ml-1">
                                (+€{formatOfferPrice(c.supplement_cents)})
                              </span>
                            </Show>
                          </>
                        );
                      }}
                    </For>
                    <Show when={slot.supplement_cents > 0}>
                      <span
                        class="tag is-warning ml-2"
                        style={{ "font-size": "0.55rem" }}
                      >
                        slot: +€{formatOfferPrice(slot.supplement_cents)}
                      </span>
                    </Show>
                  </div>
                )}
              </For>
            </div>
          </div>

          {/* Right: actions */}
          <div class="buttons are-small" style={{ "flex-shrink": "0" }}>
            <button
              class="button is-info is-small is-outlined"
              disabled={saving() || isEditing()}
              onClick={() => startEditing(offer)}
              title="Edit offer"
            >
              <span class="icon is-small">
                <span>✏️</span>
              </span>
            </button>

            <button
              class={`button is-small ${offer.is_active ? "is-warning is-outlined" : "is-success is-outlined"}`}
              classList={{ "is-loading": saving() }}
              disabled={saving() || isEditing()}
              onClick={() => handleToggleActive(offer)}
              title={offer.is_active ? "Deactivate" : "Activate"}
            >
              {offer.is_active ? "⏸" : "▶"}
            </button>

            <button
              class="button is-danger is-small is-outlined"
              classList={{ "is-loading": saving() }}
              disabled={saving() || isEditing()}
              onClick={() => handleDelete(offer)}
              title="Delete offer"
            >
              <span class="icon is-small">
                <span>🗑️</span>
              </span>
            </button>
          </div>
        </div>

        {/* ── Availability Rule ────────────────────────────────── */}
        <div class="mt-2 pt-2" style={{ "border-top": "1px solid var(--bulma-border-weak)" }}>
          <div class="is-flex is-align-items-center mb-1">
            <span class="is-size-7 has-text-weight-semibold has-text-grey-dark mr-1">🕐</span>
            <span class="is-size-7 has-text-weight-semibold has-text-grey-dark">Availability</span>
          </div>
          <AvailabilityRuleEditor
            rule={offer.availability_rule}
            entityType="offer"
            entityId={offer.id}
            onChanged={() => fetchOffers(props.restaurantId)}
          />
        </div>
      </div>
    );
  };

  // ── Main render ────────────────────────────────────────────────
  return (
    <div class="mb-5">
      {/* Header */}
      <div class="is-flex is-justify-content-space-between is-align-items-center mb-4">
        <div
          class="is-flex is-align-items-center"
          style={{ gap: "0.5rem" }}
        >
          <h2 class="title is-4 mb-0">🏷️ Offers</h2>
          <Show when={loaded() && offers().length > 0}>
            <span class="tag is-primary">{offers().length}</span>
          </Show>
          <button
            class="button is-small"
            onClick={() => setCollapsed((c) => !c)}
            title={collapsed() ? "Expand" : "Collapse"}
          >
            {collapsed() ? "▶" : "▼"}
          </button>
        </div>

        <Show when={!isEditing()}>
          <button
            class="button is-primary"
            onClick={startCreating}
            disabled={saving()}
          >
            <span class="icon is-small">
              <span>➕</span>
            </span>
            <span>Create Offer</span>
          </button>
        </Show>
      </div>

      <Show when={!collapsed()}>
        {/* Messages */}
        <Show when={localError()}>
          <div class="notification is-danger py-2 px-3 mb-3">
            <button
              class="delete is-small"
              onClick={() => setLocalError(null)}
            />
            {localError()}
          </div>
        </Show>
        <Show when={offerError()}>
          <div class="notification is-danger py-2 px-3 mb-3">
            <button class="delete is-small" onClick={clearOfferError} />
            {offerError()}
          </div>
        </Show>
        <Show when={localSuccess()}>
          <div class="notification is-success py-2 px-3 mb-3">
            <button
              class="delete is-small"
              onClick={() => setLocalSuccess(null)}
            />
            {localSuccess()}
          </div>
        </Show>

        {/* Loading */}
        <Show when={!loaded()}>
          <div class="has-text-centered py-4">
            <progress class="progress is-primary is-small" max="100" />
            <p class="has-text-grey is-size-7 mt-1">Loading offers…</p>
          </div>
        </Show>

        {/* Create form (shown above the list) */}
        <Show when={creating()}>{renderOfferForm()}</Show>

        {/* Edit form (shown above the list) */}
        <Show when={editingOfferId() !== null}>{renderOfferForm()}</Show>

        {/* Offers list */}
        <Show when={loaded()}>
          <Show
            when={offers().length > 0}
            fallback={
              <Show when={!isEditing()}>
                <div class="notification has-text-centered">
                  <p class="is-size-4 mb-2">🏷️</p>
                  <p class="has-text-grey">
                    No offers yet. Create one to set up combos or deals.
                  </p>
                </div>
              </Show>
            }
          >
            <For each={offers()}>{(offer) => renderOfferRow(offer)}</For>
          </Show>
        </Show>
      </Show>
    </div>
  );
}