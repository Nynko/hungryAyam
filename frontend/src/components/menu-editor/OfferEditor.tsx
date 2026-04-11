import { Show, For, createSignal, createEffect, onMount } from "solid-js";
import { createStore, produce } from "solid-js/store";
import type { Offer } from "@bindings/Offer";
import type { CreateOffer } from "@bindings/CreateOffer";
import type { UpdateOffer } from "@bindings/UpdateOffer";
import type { MenuSection } from "@bindings/MenuSection";
import {
  fetchOffers,
  getOffersForMenu,
  createOffer,
  updateOffer,
  deleteOffer as deleteOfferApi,
  activateOffer,
  deactivateOffer,
  formatOfferPrice,
  offerError,
  clearOfferError,
} from "@/stores/offerStore";
import { showConfirm } from "@/stores/confirmStore";
import SlotFormFields from "@/components/SlotFormFields";
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
// Types
// ══════════════════════════════════════════════════════════════════

interface OfferEditorProps {
  restaurantId: string;
  menuId: string | null;
  /** All sections from the current menu draft (for Section constraint picker). */
  menuSections: MenuSection[];
}

// ══════════════════════════════════════════════════════════════════
// Component
// ══════════════════════════════════════════════════════════════════

export default function OfferEditor(props: OfferEditorProps) {
  // ── State ──────────────────────────────────────────────────────
  const [existingOffer, setExistingOffer] = createSignal<Offer | null>(null);
  const [editing, setEditing] = createSignal(false);
  const [collapsed, setCollapsed] = createSignal(true);
  const [saving, setSaving] = createSignal(false);
  const [localError, setLocalError] = createSignal<string | null>(null);
  const [localSuccess, setLocalSuccess] = createSignal<string | null>(null);
  const [loadingOffers, setLoadingOffers] = createSignal(false);

  // Use createStore for fine-grained reactivity — updating one slot's
  // label won't recreate sibling DOM nodes and lose input focus.
  const [draft, setDraft] = createStore<DraftOffer>(emptyDraftOffer());

  const resetDraft = (d: DraftOffer) => {
    setDraft(d);
  };

  const hasOffer = () => existingOffer() !== null;
  const isNew = () => !hasOffer() && editing();
  const menuId = () => props.menuId;

  // ── Load existing offers for this menu ─────────────────────────
  const loadLinkedOffer = async () => {
    if (!menuId()) return;
    setLoadingOffers(true);
    try {
      await fetchOffers(props.restaurantId);
      const linked = getOffersForMenu(props.restaurantId, menuId()!);
      if (linked.length > 0) {
        setExistingOffer(linked[0]);
        resetDraft(offerToDraft(linked[0]));
      }
    } finally {
      setLoadingOffers(false);
    }
  };

  onMount(() => {
    if (menuId()) {
      loadLinkedOffer();
    }
  });

  // Reload when menuId changes (e.g. after first save of a new menu)
  createEffect(() => {
    const id = menuId();
    if (id) {
      loadLinkedOffer();
    }
  });

  // ── Draft manipulation (via createStore path setters) ──────────

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

  const moveSlot = (fromIndex: number, toIndex: number) => {
    setDraft(
      produce((d) => {
        const [slot] = d.slots.splice(fromIndex, 1);
        d.slots.splice(toIndex, 0, slot);
      }),
    );
  };

  // ── Save ───────────────────────────────────────────────────────

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
      if (hasOffer()) {
        // Update existing offer
        const request: UpdateOffer = {
          id: existingOffer()!.id,
          menu_id: menuId() ?? null,
          title: draft.title,
          description: draft.description || null,
          base_price_cents: dollarsToCents(draft.basePriceDisplay),
          is_active: draft.isActive,
          slots: buildCreateSlots(draft.slots).map((s) => ({
            label: s.label,
            min_items: s.min_items,
            max_items: s.max_items,
            supplement_cents: s.supplement_cents,
            constraints: s.constraints.map((c) => ({
              kind: c.kind,
              supplement_cents: c.supplement_cents,
            })),
          })),
        };

        const result = await updateOffer(request);
        if (result) {
          setExistingOffer(result);
          resetDraft(offerToDraft(result));
          setEditing(false);
          setLocalSuccess("Offer updated successfully.");
        }
      } else {
        // Create new offer
        if (!menuId()) {
          setLocalError("Please save the menu first before adding an offer.");
          return;
        }

        const request: CreateOffer = {
          restaurant_id: props.restaurantId,
          menu_id: menuId(),
          title: draft.title,
          description: draft.description || null,
          base_price_cents: dollarsToCents(draft.basePriceDisplay),
          is_active: draft.isActive,
          slots: buildCreateSlots(draft.slots),
        };

        const result = await createOffer(request);
        if (result) {
          setExistingOffer(result);
          resetDraft(offerToDraft(result));
          setEditing(false);
          setLocalSuccess("Offer created successfully.");
        }
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setLocalError(msg);
    } finally {
      setSaving(false);
    }
  };

  // ── Delete ─────────────────────────────────────────────────────
  const handleDelete = async () => {
    const offer = existingOffer();
    if (!offer) return;

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
        setExistingOffer(null);
        resetDraft(emptyDraftOffer());
        setEditing(false);
        setLocalSuccess("Offer deleted.");
      }
    } finally {
      setSaving(false);
    }
  };

  // ── Toggle active ──────────────────────────────────────────────
  const handleToggleActive = async () => {
    const offer = existingOffer();
    if (!offer) return;

    setSaving(true);
    try {
      const result = offer.is_active
        ? await deactivateOffer(offer.id)
        : await activateOffer(offer.id);

      if (result) {
        setExistingOffer(result);
        setDraft("isActive", result.is_active);
        setLocalSuccess(
          result.is_active ? "Offer activated." : "Offer deactivated.",
        );
      }
    } finally {
      setSaving(false);
    }
  };

  // ── Start editing / creating ───────────────────────────────────
  const startEditing = () => {
    const offer = existingOffer();
    if (offer) {
      resetDraft(offerToDraft(offer));
    }
    setEditing(true);
    setCollapsed(false);
    setLocalError(null);
    setLocalSuccess(null);
  };

  const startCreating = () => {
    resetDraft(emptyDraftOffer());
    setEditing(true);
    setCollapsed(false);
    setLocalError(null);
    setLocalSuccess(null);
  };

  const cancelEditing = () => {
    const offer = existingOffer();
    if (offer) {
      resetDraft(offerToDraft(offer));
    } else {
      resetDraft(emptyDraftOffer());
    }
    setEditing(false);
    setLocalError(null);
  };

  // ── Render ─────────────────────────────────────────────────────
  return (
    <div class="box mb-5 has-background-light editor-panel">
      {/* ── Header ──────────────────────────────────────────── */}
      <div
        class="is-flex is-justify-content-space-between is-align-items-center"
        style={{ cursor: "pointer" }}
        onClick={() => setCollapsed((c) => !c)}
      >
        <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
          <span class="is-size-5">🏷️</span>
          <h3 class="title is-5 mb-0">
            <Show when={hasOffer()} fallback="Menu Offer">
              {existingOffer()!.title}
            </Show>
          </h3>
          <Show when={hasOffer()}>
            <span
              class={`tag is-small ${existingOffer()!.is_active ? "is-success" : "is-warning"}`}
            >
              {existingOffer()!.is_active ? "Active" : "Inactive"}
            </span>
            <span class="tag is-info is-small">
              €{formatOfferPrice(existingOffer()!.base_price_cents)}
            </span>
            <span class="tag is-small">
              {existingOffer()!.slots.length} slot
              {existingOffer()!.slots.length !== 1 ? "s" : ""}
            </span>
          </Show>
          <Show when={!hasOffer() && !editing()}>
            <span class="has-text-grey is-size-7">No offer linked</span>
          </Show>
        </div>

        <span
          class="icon has-text-grey"
          style={{ transition: "transform 0.2s ease" }}
        >
          <span>{collapsed() ? "▶" : "▼"}</span>
        </span>
      </div>

      {/* ── Body (collapsible) ──────────────────────────────── */}
      <Show when={!collapsed()}>
        <div class="mt-4">
          {/* ── Messages ──────────────────────────────────── */}
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

          {/* ── Loading ───────────────────────────────────── */}
          <Show when={loadingOffers()}>
            <div class="has-text-centered py-3">
              <progress class="progress is-primary is-small" max="100" />
              <p class="has-text-grey is-size-7 mt-1">Loading offer…</p>
            </div>
          </Show>

          {/* ── No offer yet: prompt to create ────────────── */}
          <Show when={!hasOffer() && !editing() && !loadingOffers()}>
            <div class="has-text-centered py-4">
              <p class="has-text-grey mb-3">
                This menu doesn't have an associated offer yet.
              </p>
              <Show
                when={menuId()}
                fallback={
                  <p class="has-text-grey-light is-size-7 is-italic">
                    Save the menu first to enable offers.
                  </p>
                }
              >
                <button
                  class="button is-primary is-outlined"
                  onClick={startCreating}
                >
                  <span class="icon is-small">
                    <span>➕</span>
                  </span>
                  <span>Create Offer for this Menu</span>
                </button>
              </Show>
            </div>
          </Show>

          {/* ── Existing offer: read-only view ────────────── */}
          <Show when={hasOffer() && !editing()}>
            {(_) => {
              const offer = () => existingOffer()!;
              return (
                <div>
                  {/* Summary */}
                  <div class="columns is-multiline mb-3">
                    <div class="column is-6">
                      <p class="is-size-7 has-text-grey">Title</p>
                      <p class="has-text-weight-semibold">{offer().title}</p>
                    </div>
                    <div class="column is-3">
                      <p class="is-size-7 has-text-grey">Base Price</p>
                      <p class="has-text-weight-semibold">
                        €{formatOfferPrice(offer().base_price_cents)}
                      </p>
                    </div>
                    <div class="column is-3">
                      <p class="is-size-7 has-text-grey">Status</p>
                      <span
                        class={`tag ${offer().is_active ? "is-success" : "is-warning"}`}
                      >
                        {offer().is_active ? "Active" : "Inactive"}
                      </span>
                    </div>
                    <Show when={offer().description}>
                      <div class="column is-12">
                        <p class="is-size-7 has-text-grey">Description</p>
                        <p>{offer().description}</p>
                      </div>
                    </Show>
                  </div>

                  {/* Slots summary */}
                  <h4 class="title is-6 mb-2">
                    Slots ({offer().slots.length})
                  </h4>
                  <For each={offer().slots}>
                    {(slot) => (
                      <div class="box p-3 mb-2 has-background-light">
                        <div class="is-flex is-justify-content-space-between is-align-items-center">
                          <div>
                            <span class="has-text-weight-semibold">
                              {slot.label}
                            </span>
                            <span class="has-text-grey is-size-7 ml-2">
                              {slot.min_items === slot.max_items
                                ? `${slot.max_items} item${slot.max_items !== 1 ? "s" : ""}`
                                : `${slot.min_items}–${slot.max_items} items`}
                            </span>
                            <Show when={slot.min_items === 0}>
                              <span class="tag is-small ml-2">
                                optional
                              </span>
                            </Show>
                          </div>
                          <div
                            class="is-flex is-align-items-center"
                            style={{ gap: "0.5rem" }}
                          >
                            <Show when={slot.supplement_cents > 0}>
                              <span class="tag is-warning is-small">
                                +€{formatOfferPrice(slot.supplement_cents)}
                              </span>
                            </Show>
                            <span class="tag is-info is-small">
                              {slot.constraints.length} constraint
                              {slot.constraints.length !== 1 ? "s" : ""}
                            </span>
                          </div>
                        </div>

                        {/* Constraints */}
                        <div
                          class="mt-2"
                          style={{ "padding-left": "0.5rem" }}
                        >
                          <For each={slot.constraints}>
                            {(constraint) => {
                              const kindKey = constraintKindKey(constraint.kind);
                              const kindValue = constraintKindValue(
                                constraint.kind,
                              );
                              const sectionName = () => {
                                if (kindKey === "Section") {
                                  const section = flattenSectionsFromMenus(
                                    menuId() ? [{ id: menuId()!, name: "Current Menu", sections: props.menuSections }] : [],
                                  ).find(
                                    (s) => s.id === kindValue,
                                  );
                                  return (
                                    section?.name ?? kindValue.slice(0, 8) + "…"
                                  );
                                }
                                return kindValue.slice(0, 8) + "…";
                              };

                              return (
                                <p class="is-size-7 has-text-grey">
                                  <span
                                    class="tag mr-1"
                                    style={{ "font-size": "0.6rem" }}
                                  >
                                    {kindKey}
                                  </span>
                                  {kindKey === "Section"
                                    ? sectionName()
                                    : kindValue.slice(0, 12) + "…"}
                                  <Show when={constraint.supplement_cents > 0}>
                                    <span class="has-text-warning-dark ml-1">
                                      (+€
                                      {formatOfferPrice(
                                        constraint.supplement_cents,
                                      )}
                                      )
                                    </span>
                                  </Show>
                                </p>
                              );
                            }}
                          </For>
                        </div>
                      </div>
                    )}
                  </For>

                  {/* Action buttons */}
                  <hr class="my-3" />
                  <div class="buttons">
                    <button
                      class="button is-info is-small"
                      onClick={startEditing}
                      disabled={saving()}
                    >
                      <span class="icon is-small">
                        <span>✏️</span>
                      </span>
                      <span>Edit Offer</span>
                    </button>

                    <button
                      class={`button is-small ${offer().is_active ? "is-warning is-outlined" : "is-success is-outlined"}`}
                      classList={{ "is-loading": saving() }}
                      disabled={saving()}
                      onClick={handleToggleActive}
                    >
                      {offer().is_active ? "Deactivate" : "Activate"}
                    </button>

                    <button
                      class="button is-danger is-small is-outlined"
                      classList={{ "is-loading": saving() }}
                      disabled={saving()}
                      onClick={handleDelete}
                    >
                      <span class="icon is-small">
                        <span>🗑️</span>
                      </span>
                      <span>Delete</span>
                    </button>
                  </div>
                </div>
              );
            }}
          </Show>

          {/* ── Edit / Create form ────────────────────────── */}
          <Show when={editing()}>
            <div>
              <h4 class="title is-6 mb-3">
                {isNew() ? "Create Offer" : "Edit Offer"}
              </h4>

              {/* Title & Description */}
              <div class="columns is-multiline">
                <div class="column is-6">
                  <div class="field">
                    <label class="label is-small">Title *</label>
                    <div class="control">
                      <input
                        class="input is-small"
                        type="text"
                        placeholder="e.g. Menu du Jour"
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

                <div class="column is-3">
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

              {/* ── Slots ──────────────────────────────────── */}
              <div class="mb-4">
                <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
                  <h5 class="title is-6 mb-0">
                    Slots ({draft.slots.length})
                  </h5>
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
                      No slots yet. Add at least one slot (e.g. "Starter",
                      "Main", "Dessert").
                    </p>
                  </div>
                </Show>

                <For each={draft.slots}>
                  {(slot, slotIndex) => (
                    <div class="box p-3 mb-3 has-background-light editor-subpanel">
                      {/* Slot header with move buttons and remove button */}
                      <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
                        <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
                          <div class="buttons are-small has-addons mb-0">
                            <button
                              class="button is-small"
                              disabled={slotIndex() === 0}
                              onClick={() => moveSlot(slotIndex(), slotIndex() - 1)}
                              title="Move up"
                            >↑</button>
                            <button
                              class="button is-small"
                              disabled={slotIndex() === draft.slots.length - 1}
                              onClick={() => moveSlot(slotIndex(), slotIndex() + 1)}
                              title="Move down"
                            >↓</button>
                          </div>
                          <span class="has-text-weight-semibold is-size-6">
                            Slot {slotIndex() + 1}
                            <Show when={slot.label}>
                              <span class="has-text-grey has-text-weight-normal ml-1">
                                — {slot.label}
                              </span>
                            </Show>
                          </span>
                        </div>
                        <button
                          class="delete is-small"
                          title="Remove slot"
                          onClick={() => removeSlot(slotIndex())}
                        />
                      </div>

                      <SlotFormFields
                        slot={slot}
                        slotIndex={slotIndex()}
                        setDraft={setDraft}
                        menus={menuId() ? [{ id: menuId()!, name: "Current Menu", sections: props.menuSections }] : []}
                        onAddConstraint={() => addConstraint(slotIndex())}
                        onRemoveConstraint={(cIndex) => removeConstraint(slotIndex(), cIndex)}
                      />
                    </div>
                  )}
                </For>
              </div>

              {/* ── Form actions ───────────────────────────── */}
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
                  <span>{isNew() ? "Create Offer" : "Save Changes"}</span>
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
          </Show>
        </div>
      </Show>
    </div>
  );
}
