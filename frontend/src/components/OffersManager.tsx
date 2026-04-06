import { Show, For, Index, createSignal, onMount, createMemo } from "solid-js";
import { createStore, produce } from "solid-js/store";
import type { Offer } from "@bindings/Offer";
import type { OfferSlot } from "@bindings/OfferSlot";
import type { SlotConstraintKind } from "@bindings/SlotConstraintKind";
import type { CreateOffer } from "@bindings/CreateOffer";
import type { CreateOfferSlot } from "@bindings/CreateOfferSlot";
import type { CreateOfferSlotConstraint } from "@bindings/CreateOfferSlotConstraint";
import type { UpdateOffer } from "@bindings/UpdateOffer";
import type { Menu } from "@bindings/Menu";
import type { MenuSection } from "@bindings/MenuSection";
import { Card } from "@/components/Card";
import AvailabilityRuleEditor from "@/components/AvailabilityRuleEditor";
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

// ══════════════════════════════════════════════════════════════════
// Draft types (local to this component)
// ══════════════════════════════════════════════════════════════════

interface DraftSlotConstraint {
  tempId: string;
  kind: SlotConstraintKind;
  /** Displayed/edited as dollars (string to allow typing "1.5"), stored as
   *  cents when sent to the backend. */
  supplementDisplay: string;
}

interface DraftSlot {
  tempId: string;
  label: string;
  minItems: number;
  maxItems: number;
  /** Displayed/edited as dollars string, converted to cents for backend. */
  supplementDisplay: string;
  constraints: DraftSlotConstraint[];
}

interface DraftOffer {
  title: string;
  description: string;
  /** Displayed/edited as dollars string, converted to cents for backend. */
  basePriceDisplay: string;
  isActive: boolean;
  menuId: string | null;
  slots: DraftSlot[];
}

// ══════════════════════════════════════════════════════════════════
// ID generators
// ══════════════════════════════════════════════════════════════════

let _cTempId = 0;
function nextConstraintTempId(): string {
  return `om-c-${++_cTempId}`;
}

let _sTempId = 0;
function nextSlotTempId(): string {
  return `om-s-${++_sTempId}`;
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

function emptyDraft(): DraftOffer {
  return {
    title: "",
    description: "",
    basePriceDisplay: "0.00",
    isActive: false,
    menuId: null,
    slots: [],
  };
}

function offerToDraft(offer: Offer): DraftOffer {
  return {
    title: offer.title,
    description: offer.description ?? "",
    basePriceDisplay: centsToDollars(offer.base_price_cents),
    isActive: offer.is_active,
    menuId: offer.menu_id,
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

/** Flatten nested sections into a flat list with depth + menu name context. */
function flattenAllSections(
  allMenus: Menu[],
): Array<{
  id: string;
  name: string;
  depth: number;
  menuName: string;
  menuId: string;
}> {
  const result: Array<{
    id: string;
    name: string;
    depth: number;
    menuName: string;
    menuId: string;
  }> = [];

  const recurse = (
    sections: MenuSection[],
    depth: number,
    menuName: string,
    menuId: string,
  ) => {
    for (const s of sections) {
      result.push({ id: s.id, name: s.name, depth, menuName, menuId });
      if (s.subsections && s.subsections.length > 0) {
        recurse(s.subsections, depth + 1, menuName, menuId);
      }
    }
  };

  for (const menu of allMenus) {
    recurse(menu.sections, 0, menu.name, menu.id);
  }

  return result;
}

function constraintKindKey(
  kind: SlotConstraintKind,
): "Item" | "Tag" | "Section" {
  if ("Item" in kind) return "Item";
  if ("Tag" in kind) return "Tag";
  return "Section";
}

function constraintKindValue(kind: SlotConstraintKind): string {
  if ("Item" in kind) return kind.Item;
  if ("Tag" in kind) return (kind as { Tag: string }).Tag;
  if ("Section" in kind) return (kind as { Section: string }).Section;
  return "";
}

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
  const [draft, setDraft] = createStore<DraftOffer>(emptyDraft());

  // ── Derived ────────────────────────────────────────────────────
  const offers = () => getOffers(props.restaurantId);
  const isEditing = () => editingOfferId() !== null || creating();

  const allSections = createMemo(() => flattenAllSections(props.menus));

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

  // ── Build API payloads ─────────────────────────────────────────

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

  // ── Actions ────────────────────────────────────────────────────

  const startCreating = () => {
    resetDraft(emptyDraft());
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
    resetDraft(emptyDraft());
    setEditingOfferId(null);
    setCreating(false);
    setLocalError(null);
  };

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
      if (creating()) {
        const request: CreateOffer = {
          restaurant_id: props.restaurantId,
          menu_id: draft.menuId,
          title: draft.title,
          description: draft.description || null,
          base_price_cents: dollarsToCents(draft.basePriceDisplay),
          is_active: draft.isActive,
          slots: buildCreateSlots(),
        };

        const result = await createOffer(request);
        if (result) {
          setCreating(false);
          resetDraft(emptyDraft());
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
          setEditingOfferId(null);
          resetDraft(emptyDraft());
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
      <div class="box mb-4 has-background-light">
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
            {(slot, slotIndex) => (
              <div
                class="box p-3 mb-3"
                style={{
                  "border-left": "3px solid var(--bulma-border)",
                }}
              >
                {/* Slot header */}
                <div class="is-flex is-justify-content-space-between is-align-items-center mb-2">
                  <span class="has-text-weight-semibold is-size-6">
                    Slot {slotIndex + 1}
                    <Show when={slot().label}>
                      <span class="has-text-grey has-text-weight-normal ml-1">
                        — {slot().label}
                      </span>
                    </Show>
                  </span>
                  <button
                    class="delete is-small"
                    title="Remove slot"
                    onClick={() => removeSlot(slotIndex)}
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
                          value={slot().label}
                          onInput={(e) =>
                            setDraft(
                              "slots",
                              slotIndex,
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
                          value={slot().minItems}
                          onInput={(e) =>
                            setDraft(
                              "slots",
                              slotIndex,
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
                          value={slot().maxItems}
                          onInput={(e) =>
                            setDraft(
                              "slots",
                              slotIndex,
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
                      <label class="label is-small">Slot Supplement (€)</label>
                      <div class="control">
                        <input
                          class="input is-small"
                          type="text"
                          inputmode="decimal"
                          placeholder="0.00"
                          value={slot().supplementDisplay}
                          onInput={(e) =>
                            setDraft(
                              "slots",
                              slotIndex,
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
                              slotIndex,
                              "supplementDisplay",
                              centsToDollars(cents),
                            );
                          }}
                        />
                      </div>
                      <p class="help">
                        {dollarsToCents(slot().supplementDisplay) === 0
                          ? "Included in base"
                          : `+€${centsToDollars(dollarsToCents(slot().supplementDisplay))}`}
                      </p>
                    </div>
                  </div>
                </div>

                {/* ── Constraints ──────────────────────────────── */}
                <div class="mt-2">
                  <div class="is-flex is-justify-content-space-between is-align-items-center mb-1">
                    <span class="is-size-7 has-text-weight-semibold has-text-grey-dark">
                      Constraints ({slot().constraints.length})
                    </span>
                    <button
                      class="button is-small"
                      onClick={() => addConstraint(slotIndex)}
                    >
                      <span
                        class="icon is-small"
                        style={{ "font-size": "0.7rem" }}
                      >
                        <span>➕</span>
                      </span>
                      <span class="is-size-7">Add</span>
                    </button>
                  </div>

                  <Show when={slot().constraints.length === 0}>
                    <p class="has-text-grey is-size-7 is-italic ml-2">
                      Add constraints to define which items are allowed.
                    </p>
                  </Show>

                  <Index each={slot().constraints}>
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
                          {/* Type selector */}
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
                                else newConstraintKind = { Section: "" };

                                setDraft(
                                  "slots",
                                  slotIndex,
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

                          {/* Section picker (with menu optgroups) */}
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
                                    slotIndex,
                                    "constraints",
                                    cIndex,
                                    "kind",
                                    { Section: e.currentTarget.value },
                                  );
                                }}
                              >
                                <option value="">— Select section —</option>
                                <For each={props.menus}>
                                  {(menu) => (
                                    <optgroup label={menu.name}>
                                      <For
                                        each={flattenAllSections([menu])}
                                      >
                                        {(section) => (
                                          <option value={section.id}>
                                            {"  ".repeat(section.depth)}
                                            {section.name}
                                          </option>
                                        )}
                                      </For>
                                    </optgroup>
                                  )}
                                </For>
                              </select>
                            </div>
                          </Show>

                          {/* Tag / Item: UUID input */}
                          <Show
                            when={
                              kindKey() === "Tag" || kindKey() === "Item"
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
                                    slotIndex,
                                    "constraints",
                                    cIndex,
                                    "kind",
                                    newKind,
                                  );
                                }}
                              />
                            </div>
                          </Show>

                          {/* Supplement (€) */}
                          <div class="control" style={{ width: "80px" }}>
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
                                  slotIndex,
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
                                  slotIndex,
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
                            {dollarsToCents(constraint().supplementDisplay) ===
                            0
                              ? "incl."
                              : `+€${centsToDollars(dollarsToCents(constraint().supplementDisplay))}`}
                          </span>

                          {/* Remove */}
                          <button
                            class="delete is-small"
                            title="Remove constraint"
                            onClick={() =>
                              removeConstraint(slotIndex, cIndex)
                            }
                          />
                        </div>
                      );
                    }}
                  </Index>
                </div>
              </div>
            )}
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