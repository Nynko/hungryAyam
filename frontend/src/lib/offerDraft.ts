import type { Offer } from "@bindings/Offer";
import type { SlotConstraintKind } from "@bindings/SlotConstraintKind";
import type { CreateOfferSlot } from "@bindings/CreateOfferSlot";
import type { CreateOfferSlotConstraint } from "@bindings/CreateOfferSlotConstraint";
import type { MenuSection } from "@bindings/MenuSection";

// ══════════════════════════════════════════════════════════════════
// Draft types
// ══════════════════════════════════════════════════════════════════

export interface DraftSlotConstraint {
  tempId: string;
  kind: SlotConstraintKind;
  /** Displayed/edited as dollars (string to allow typing "1.5"), stored as
   *  cents when sent to the backend. */
  supplementDisplay: string;
}

export interface DraftSlot {
  tempId: string;
  /** Real slot UUID from the backend, if this slot already exists. Null for newly added slots. */
  slotId: string | null;
  label: string;
  minItems: number;
  maxItems: number;
  /** Displayed/edited as dollars string, converted to cents for backend. */
  supplementDisplay: string;
  slotGroup: string;
  constraints: DraftSlotConstraint[];
}

export interface DraftOffer {
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
export function nextConstraintTempId(): string {
  return `c-${++_cTempId}`;
}

let _sTempId = 0;
export function nextSlotTempId(): string {
  return `s-${++_sTempId}`;
}

// ══════════════════════════════════════════════════════════════════
// Price helpers
// ══════════════════════════════════════════════════════════════════

/** Convert cents (integer) to a display string like "12.50". */
export function centsToDollars(cents: number): string {
  return (cents / 100).toFixed(2);
}

/** Convert a display string like "12.50" or "12,50" to cents (integer).
 *  Returns 0 for invalid input. */
export function dollarsToCents(display: string): number {
  const normalized = display.replace(",", ".");
  const parsed = parseFloat(normalized);
  if (isNaN(parsed) || parsed < 0) return 0;
  return Math.round(parsed * 100);
}

// ══════════════════════════════════════════════════════════════════
// Factory helpers
// ══════════════════════════════════════════════════════════════════

export function emptyDraftOffer(): DraftOffer {
  return {
    title: "",
    description: "",
    basePriceDisplay: "0.00",
    isActive: false,
    menuId: null,
    slots: [],
  };
}

export function offerToDraft(offer: Offer): DraftOffer {
  return {
    title: offer.title,
    description: offer.description ?? "",
    basePriceDisplay: centsToDollars(offer.base_price_cents),
    isActive: offer.is_active,
    menuId: offer.menu_id,
    slots: offer.slots.map((slot) => ({
      tempId: nextSlotTempId(),
      slotId: slot.id,
      label: slot.label,
      minItems: slot.min_items,
      maxItems: slot.max_items,
      supplementDisplay: centsToDollars(slot.supplement_cents),
      slotGroup: slot.slot_group ?? "",
      constraints: slot.constraints.map((c) => ({
        tempId: nextConstraintTempId(),
        kind: c.kind,
        supplementDisplay: centsToDollars(c.supplement_cents),
      })),
    })),
  };
}

export function emptySlot(): DraftSlot {
  return {
    tempId: nextSlotTempId(),
    slotId: null,
    label: "",
    minItems: 1,
    maxItems: 1,
    supplementDisplay: "0.00",
    slotGroup: "",
    constraints: [],
  };
}

export function emptyConstraint(): DraftSlotConstraint {
  return {
    tempId: nextConstraintTempId(),
    kind: { Section: "" },
    supplementDisplay: "0.00",
  };
}

// ══════════════════════════════════════════════════════════════════
// Constraint helpers
// ══════════════════════════════════════════════════════════════════

export function constraintKindKey(
  kind: SlotConstraintKind,
): "Item" | "Tag" | "Section" {
  if ("Item" in kind) return "Item";
  if ("Tag" in kind) return "Tag";
  return "Section";
}

export function constraintKindValue(kind: SlotConstraintKind): string {
  if ("Item" in kind) return kind.Item;
  if ("Tag" in kind) return (kind as { Tag: string }).Tag;
  if ("Section" in kind) return (kind as { Section: string }).Section;
  return "";
}

// ══════════════════════════════════════════════════════════════════
// Section flattening
// ══════════════════════════════════════════════════════════════════

export interface FlatSection {
  id: string;
  name: string;
  depth: number;
  menuName: string;
  menuId: string;
}

/** Flatten nested sections across menus into a flat list with depth + menu context. */
export function flattenSectionsFromMenus(
  menus: Array<{ id: string; name: string; sections: MenuSection[] }>,
): FlatSection[] {
  const result: FlatSection[] = [];

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

  for (const menu of menus) {
    recurse(menu.sections, 0, menu.name, menu.id);
  }

  return result;
}

// ══════════════════════════════════════════════════════════════════
// API payload builders
// ══════════════════════════════════════════════════════════════════

export function buildCreateSlots(slots: DraftSlot[]): CreateOfferSlot[] {
  return slots.map((s) => ({
    label: s.label,
    min_items: s.minItems,
    max_items: s.maxItems,
    supplement_cents: dollarsToCents(s.supplementDisplay),
    slot_group: s.slotGroup.trim() || null,
    constraints: s.constraints.map(
      (c): CreateOfferSlotConstraint => ({
        kind: c.kind,
        supplement_cents: dollarsToCents(c.supplementDisplay),
      }),
    ),
  }));
}

// ══════════════════════════════════════════════════════════════════
// Validation
// ══════════════════════════════════════════════════════════════════

/** Validate offer draft fields. Returns error message or null if valid. */
export function validateDraft(draft: DraftOffer): string | null {
  if (!draft.title.trim()) return "Title is required.";
  if (dollarsToCents(draft.basePriceDisplay) < 0)
    return "Base price cannot be negative.";
  if (draft.slots.length === 0) return "Add at least one slot.";

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
}
