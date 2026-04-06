import { Show, For, Index, createSignal, createEffect, onMount } from "solid-js";
import { createStore, produce } from "solid-js/store";
import type { Offer } from "@bindings/Offer";
import type { OfferSlot } from "@bindings/OfferSlot";
import type { OfferSlotConstraint } from "@bindings/OfferSlotConstraint";
import type { SlotConstraintKind } from "@bindings/SlotConstraintKind";
import type { CreateOffer } from "@bindings/CreateOffer";
import type { CreateOfferSlot } from "@bindings/CreateOfferSlot";
import type { CreateOfferSlotConstraint } from "@bindings/CreateOfferSlotConstraint";
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
  offerLoading,
  offerError,
  clearOfferError,
} from "@/stores/offerStore";
import { showConfirm } from "@/stores/confirmStore";
import { editorState } from "@/stores/menuEditorStore";

// ══════════════════════════════════════════════════════════════════
// Types
// ══════════════════════════════════════════════════════════════════

interface OfferEditorProps {
  restaurantId: string;
  menuId: string | null;
  /** All sections from the current menu draft (for Section constraint picker). */
  menuSections: MenuSection[];
}

interface DraftSlotConstraint {
  tempId: string;
  kind: SlotConstraintKind;
  /** Displayed/edited as dollars (string), converted to cents for backend. */
  supplementDisplay: string;
}

interface DraftSlot {
  tempId: string;
  label: string;
  minItems: number;
  maxItems: number;
  /** Displayed/edited as dollars (string), converted to cents for backend. */
  supplementDisplay: string;
  constraints: DraftSlotConstraint[];
}

interface DraftOffer {
  title: string;
  description: string;
  /** Displayed/edited as dollars (string), converted to cents for backend. */
  basePriceDisplay: string;
  isActive: boolean;
  slots: DraftSlot[];
}

let _constraintTempId = 0;
function nextConstraintTempId(): string {
  return `constraint-${++_constraintTempId}`;
}

let _slotTempId = 0;
function nextSlotTempId(): string {
  return `slot-${++_slotTempId}`;
}

// ══════════════════════════════════════════════════════════════════
// Price helpers
// ══════════════════════════════════════════════════════════════════

/** Convert cents (integer) to a display string like "12.50". */
function centsToDollars(cents: number): string {
  return (cents / 100).toFixed(2);
}

/** Convert a display string like "12.50" or "12,50" to cents (integer).
 *  Returns 0 for invalid input. */
function dollarsToCents(display: string): number {
  const normalized = display.replace(",", ".");
  const parsed = parseFloat(normalized);
  if (isNaN(parsed) || parsed < 0) return 0;
  return Math.round(parsed * 100);
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

function emptyDraftOffer(): DraftOffer {
  return {
    title: "",
    description: "",
    basePriceDisplay: "0.00",
    isActive: false,
    slots: [],
  };
}

function offerToDraft(offer: Offer): DraftOffer {
  return {
    title: offer.title,
    description: offer.description ?? "",
    basePriceDisplay: centsToDollars(offer.base_price_cents),
    isActive: offer.is_active,
    slots: offer.slots.map((slot) => ({
      tempId: nextSlotTempId(),
      label: slot.label,
      minItems: slot.min_items,
      maxItems: slot.max_items,
      supplementDisplay: centsToDollars(slot.supplement_cents),
      constraints: slot.constraints.map((c) => ({
        tempId: nextConstraintTempId(),
        kind: c.kind,
        supplementDisplay: centsToDollars(c.supplement_cents),
      })),
    })),
  };
}

function emptySlot(): DraftSlot {
  return {
    tempId: nextSlotTempId(),
    label: "",
    minItems: 1,
    maxItems: 1,
    supplementDisplay: "0.00",
    constraints: [],
  };
}

function emptyConstraint(): DraftSlotConstraint {
  return {
    tempId: nextConstraintTempId(),
    kind: { Section: "" },
    supplementDisplay: "0.00",
  };
}

/** Flatten nested sections into a flat list with depth info. */
function flattenSections(
  sections: MenuSection[],
  depth: number = 0,
): Array<{ id: string; name: string; depth: number }> {
  const result: Array<{ id: string; name: string; depth: number }> = [];
  for (const s of sections) {
    result.push({ id: s.id, name: s.name, depth });
    if (s.subsections && s.subsections.length > 0) {
      result.push(...flattenSections(s.subsections, depth + 1));
    }
  }
  return result;
}

/** Extract constraint kind key. */
function constraintKindKey(
  kind: SlotConstraintKind,
): "Item" | "Tag" | "Section" {
  if ("Item" in kind) return "Item";
  if ("Tag" in kind) return "Tag";
  return "Section";
}

/** Extract constraint kind value (the UUID). */
function constraintKindValue(kind: SlotConstraintKind): string {
  if ("Item" in kind) return kind.Item;
  if ("Tag" in kind) return (kind as { Tag: string }).Tag;
  if ("Section" in kind) return (kind as { Section: string }).Section;
  return "";
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

  // ── Flat sections for the constraint picker ────────────────────
  const flatSections = () => flattenSections(props.menuSections);

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

  // ── Validation ─────────────────────────────────────────────────
  const validate = (): string | null => {
    if (!draft.title.trim()) return "Offer title is required.";
    if (dollarsToCents(draft.basePriceDisplay) < 0)
      return "Base price cannot be negative.";
    if (draft.slots.length === 0) return "At least one slot is required.";

    for (const slot of draft.slots) {
      if (!slot.label.trim()) return "All slots must have a label.";
      if (slot.minItems < 0)
        return `Slot "${slot.label}": min items cannot be negative.`;
      if (slot.maxItems < slot.minItems)
        return `Slot "${slot.label}": max items must be ≥ min items.`;
      if (dollarsToCents(slot.supplementDisplay) < 0)
        return `Slot "${slot.label}": supplement cannot be negative.`;
      if (slot.constraints.length === 0)
        return `Slot "${slot.label}": at least one constraint is required.`;

      for (const c of slot.constraints) {
        const value = constraintKindValue(c.kind);
        if (!value)
          return `Slot "${slot.label}": a constraint is missing its target.`;
        if (dollarsToCents(c.supplementDisplay) < 0)
          return `Slot "${slot.label}": constraint supplement cannot be negative.`;
      }
    }

    return null;
  };

  // ── Save ───────────────────────────────────────────────────────

  const buildCreateSlots = (): CreateOfferSlot[] =>
    draft.slots.map((s) => ({
      label: s.label,
      min_items: s.minItems,
      max_items: s.maxItems,
      supplement_cents: dollarsToCents(s.supplementDisplay),
      constraints: s.constraints.map(
        (c): CreateOfferSlotConstraint => ({
          kind: c.kind,
          supplement_cents: dollarsToCents(c.supplementDisplay),
        }),
      ),
    }));

  const handleSave = async () => {
    setLocalError(null);
    setLocalSuccess(null);

    const err = validate();
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
          slots: buildCreateSlots().map((s) => ({
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
          slots: buildCreateSlots(),
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
                                  const section = flatSections().find(
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

                      {/* Slot fields */}
                      <div class="columns is-multiline">
                        <div class="column is-4">
                          <div class="field">
                            <label class="label is-small">Label *</label>
                            <div class="control">
                              <input
                                class="input is-small"
                                type="text"
                                placeholder="e.g. Starter"
                                value={slot.label}
                                onInput={(e) =>
                                  setDraft(
                                    "slots",
                                    slotIndex(),
                                    "label",
                                    e.currentTarget.value,
                                  )
                                }
                              />
                            </div>
                          </div>
                        </div>

                        <div class="column is-2">
                          <div class="field">
                            <label class="label is-small">Min</label>
                            <div class="control">
                              <input
                                class="input is-small"
                                type="number"
                                min="0"
                                value={slot.minItems}
                                onInput={(e) =>
                                  setDraft(
                                    "slots",
                                    slotIndex(),
                                    "minItems",
                                    parseInt(e.currentTarget.value) || 0,
                                  )
                                }
                              />
                            </div>
                          </div>
                        </div>

                        <div class="column is-2">
                          <div class="field">
                            <label class="label is-small">Max</label>
                            <div class="control">
                              <input
                                class="input is-small"
                                type="number"
                                min="0"
                                value={slot.maxItems}
                                onInput={(e) =>
                                  setDraft(
                                    "slots",
                                    slotIndex(),
                                    "maxItems",
                                    parseInt(e.currentTarget.value) || 0,
                                  )
                                }
                              />
                            </div>
                          </div>
                        </div>

                        <div class="column is-4">
                          <div class="field">
                            <label class="label is-small">
                              Slot Supplement (€)
                            </label>
                            <div class="control">
                              <input
                                class="input is-small"
                                type="text"
                                inputmode="decimal"
                                placeholder="0.00"
                                value={slot.supplementDisplay}
                                onInput={(e) =>
                                  setDraft(
                                    "slots",
                                    slotIndex(),
                                    "supplementDisplay",
                                    e.currentTarget.value,
                                  )
                                }
                                onBlur={(e) => {
                                  const cents = dollarsToCents(
                                    e.currentTarget.value,
                                  );
                                  setDraft(
                                    "slots",
                                    slotIndex(),
                                    "supplementDisplay",
                                    centsToDollars(cents),
                                  );
                                }}
                              />
                            </div>
                            <p class="help">
                              {dollarsToCents(slot.supplementDisplay) === 0
                                ? "Included in base"
                                : `+€${centsToDollars(dollarsToCents(slot.supplementDisplay))}`}
                            </p>
                          </div>
                        </div>
                      </div>

                      {/* ── Constraints ──────────────────────── */}
                      <div class="mt-2">
                        <div class="is-flex is-justify-content-space-between is-align-items-center mb-1">
                          <span class="is-size-7 has-text-weight-semibold has-text-grey-dark">
                            Constraints ({slot.constraints.length})
                          </span>
                          <button
                            class="button is-small"
                            onClick={() => addConstraint(slotIndex())}
                          >
                            <span
                              class="icon is-small"
                              style={{ "font-size": "0.7rem" }}
                            >
                              <span>➕</span>
                            </span>
                            <span class="is-size-7">Add Constraint</span>
                          </button>
                        </div>

                        <Show when={slot.constraints.length === 0}>
                          <p class="has-text-grey is-size-7 is-italic ml-2">
                            Add constraints to define which items are allowed.
                          </p>
                        </Show>

                        <Index each={slot.constraints}>
                          {(constraint, cIndex) => {
                            const kindKey = () =>
                              constraintKindKey(constraint().kind);
                            const kindValue = () =>
                              constraintKindValue(constraint().kind);

                            return (
                              <div
                                class="is-flex is-align-items-center mb-2"
                                style={{ gap: "0.5rem" }}
                              >
                                {/* Constraint type selector */}
                                <div class="select is-small">
                                  <select
                                    value={kindKey()}
                                    onChange={(e) => {
                                      const newKind = e.currentTarget.value as
                                        | "Item"
                                        | "Tag"
                                        | "Section";
                                      let newConstraintKind: SlotConstraintKind;
                                      if (newKind === "Item")
                                        newConstraintKind = { Item: "" };
                                      else if (newKind === "Tag")
                                        newConstraintKind = { Tag: "" };
                                      else
                                        newConstraintKind = { Section: "" };

                                      setDraft(
                                        "slots",
                                        slotIndex(),
                                        "constraints",
                                        cIndex,
                                        "kind",
                                        newConstraintKind,
                                      );
                                    }}
                                  >
                                    <option value="Section">Section</option>
                                    <option value="Tag">Tag</option>
                                    <option value="Item">Item</option>
                                  </select>
                                </div>

                                {/* Value selector/input */}
                                <Show when={kindKey() === "Section"}>
                                  <div
                                    class="select is-small"
                                    style={{ flex: "1" }}
                                  >
                                    <select
                                      value={kindValue()}
                                      onChange={(e) => {
                                        setDraft(
                                          "slots",
                                          slotIndex(),
                                          "constraints",
                                          cIndex,
                                          "kind",
                                          {
                                            Section: e.currentTarget.value,
                                          },
                                        );
                                      }}
                                    >
                                      <option value="">
                                        — Select section —
                                      </option>
                                      <For each={flatSections()}>
                                        {(section) => (
                                          <option value={section.id}>
                                            {"  ".repeat(section.depth)}
                                            {section.name}
                                          </option>
                                        )}
                                      </For>
                                    </select>
                                  </div>
                                </Show>

                                <Show
                                  when={
                                    kindKey() === "Tag" ||
                                    kindKey() === "Item"
                                  }
                                >
                                  <div
                                    class="control is-expanded"
                                    style={{ flex: "1" }}
                                  >
                                    <input
                                      class="input is-small"
                                      type="text"
                                      placeholder={`${kindKey()} UUID`}
                                      value={kindValue()}
                                      onInput={(e) => {
                                        const k = kindKey();
                                        let newKind: SlotConstraintKind;
                                        if (k === "Item")
                                          newKind = {
                                            Item: e.currentTarget.value,
                                          };
                                        else if (k === "Tag")
                                          newKind = {
                                            Tag: e.currentTarget.value,
                                          };
                                        else
                                          newKind = {
                                            Section: e.currentTarget.value,
                                          };

                                        setDraft(
                                          "slots",
                                          slotIndex(),
                                          "constraints",
                                          cIndex,
                                          "kind",
                                          newKind,
                                        );
                                      }}
                                    />
                                  </div>
                                </Show>

                                {/* Constraint supplement (€) */}
                                <div
                                  class="control"
                                  style={{ width: "80px" }}
                                >
                                  <input
                                    class="input is-small"
                                    type="text"
                                    inputmode="decimal"
                                    placeholder="0.00"
                                    title="Supplement (€)"
                                    value={constraint().supplementDisplay}
                                    onInput={(e) =>
                                      setDraft(
                                        "slots",
                                        slotIndex(),
                                        "constraints",
                                        cIndex,
                                        "supplementDisplay",
                                        e.currentTarget.value,
                                      )
                                    }
                                    onBlur={(e) => {
                                      const cents = dollarsToCents(
                                        e.currentTarget.value,
                                      );
                                      setDraft(
                                        "slots",
                                        slotIndex(),
                                        "constraints",
                                        cIndex,
                                        "supplementDisplay",
                                        centsToDollars(cents),
                                      );
                                    }}
                                  />
                                </div>
                                <span
                                  class="is-size-7 has-text-grey"
                                  style={{ "white-space": "nowrap" }}
                                >
                                  {dollarsToCents(
                                    constraint().supplementDisplay,
                                  ) === 0
                                    ? "incl."
                                    : `+€${centsToDollars(dollarsToCents(constraint().supplementDisplay))}`}
                                </span>

                                {/* Remove constraint */}
                                <button
                                  class="delete is-small"
                                  title="Remove constraint"
                                  onClick={() =>
                                    removeConstraint(slotIndex(), cIndex)
                                  }
                                />
                              </div>
                            );
                          }}
                        </Index>
                      </div>
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
