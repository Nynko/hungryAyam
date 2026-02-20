import { createStore, produce, reconcile } from "solid-js/store";
import { createSignal, batch } from "solid-js";
import type { Menu } from "@bindings/Menu";
import type { MenuSection } from "@bindings/MenuSection";
import type { MenuSectionItem } from "@bindings/MenuSectionItem";
import type { CreateMenu } from "@bindings/CreateMenu";
import type { CreateMenuSection } from "@bindings/CreateMenuSection";
import type { CreateMenuSectionItem } from "@bindings/CreateMenuSectionItem";
import type { UpdateMenuAction } from "@bindings/UpdateMenuAction";
import type { UpdateMenuActionsRequest } from "@bindings/UpdateMenuActionsRequest";
import type { ApiResponse } from "@bindings/ApiResponse";

// ═══════════════════════════════════════════════════════════════════
// Position helpers
// ═══════════════════════════════════════════════════════════════════

/** Default gap between positions when creating or rebalancing. */
export const POSITION_GAP = 10_000;

/** Minimum gap before we consider rebalancing necessary. */
const MIN_GAP = 2;

/**
 * Compute a position for an item inserted between `before` and `after`.
 *
 * - Both null → first item, returns POSITION_GAP
 * - `before` is null → inserting at the start
 * - `after` is null → inserting at the end
 * - Both present → midpoint
 */
export function computePosition(
  before: number | null,
  after: number | null
): number {
  if (before == null && after == null) return POSITION_GAP;
  if (before == null) return Math.floor((after! - POSITION_GAP > 0 ? after! - POSITION_GAP : 0 + after!) / 2);
  if (after == null) return before + POSITION_GAP;
  return Math.floor((before + after) / 2);
}

/**
 * Compute position for inserting at the start of a sorted list.
 */
export function positionAtStart(items: { position: number }[]): number {
  if (items.length === 0) return POSITION_GAP;
  const first = Math.min(...items.map((i) => i.position));
  if (first > POSITION_GAP) return first - POSITION_GAP;
  if (first > MIN_GAP) return Math.floor(first / 2);
  // Gap exhausted at the front — use 0 and caller should rebalance
  return 0;
}

/**
 * Compute position for inserting at the end of a sorted list.
 */
export function positionAtEnd(items: { position: number }[]): number {
  if (items.length === 0) return POSITION_GAP;
  const last = Math.max(...items.map((i) => i.position));
  return last + POSITION_GAP;
}

/**
 * Rebalance positions in-place with even gaps.
 * Returns a new array of positions (same length as input).
 */
export function rebalancePositions(count: number): number[] {
  return Array.from({ length: count }, (_, i) => (i + 1) * POSITION_GAP);
}

/**
 * Check whether a list of sorted positions needs rebalancing.
 */
export function needsRebalance(sortedPositions: number[]): boolean {
  for (let i = 1; i < sortedPositions.length; i++) {
    if (sortedPositions[i] - sortedPositions[i - 1] < MIN_GAP) return true;
  }
  return false;
}

// ═══════════════════════════════════════════════════════════════════
// Temporary ID generation
// ═══════════════════════════════════════════════════════════════════

let _tempIdCounter = 0;

/** Generate a temporary ID for entities that don't yet exist in the DB. */
export function tempId(): string {
  _tempIdCounter += 1;
  return `__temp_${_tempIdCounter}`;
}

/** Check whether an ID is a temporary client-side ID. */
export function isTempId(id: string): boolean {
  return id.startsWith("__temp_");
}

// ═══════════════════════════════════════════════════════════════════
// Draft types — local working copy of the menu tree
// ═══════════════════════════════════════════════════════════════════

export interface DraftItem {
  id: string;
  isNew: boolean;
  restaurant_id: string;
  name: string;
  description: string | null;
  base_price_cents: number;
  image_url: string | null;
  active: boolean;
  tags: { id: string | null; name: string | null }[];
}

export interface DraftSectionItem {
  id: string;
  isNew: boolean;
  section_id: string;
  position: number;
  price_override_cents: number | null;
  is_available: boolean;
  item: DraftItem;
}

export interface DraftSection {
  id: string;
  isNew: boolean;
  menu_id: string;
  parent_id: string | null;
  name: string;
  description: string | null;
  position: number;
  is_active: boolean;
  items: DraftSectionItem[];
  subsections: DraftSection[];
}

export interface DraftMenu {
  id: string | null; // null for new menus
  restaurant_id: string;
  name: string;
  description: string | null;
  is_active: boolean;
  permanent: boolean;
  sections: DraftSection[];
}

// ═══════════════════════════════════════════════════════════════════
// Conversion helpers: API types ↔ Draft types
// ═══════════════════════════════════════════════════════════════════

function menuSectionItemToDraft(si: MenuSectionItem): DraftSectionItem {
  return {
    id: si.id,
    isNew: false,
    section_id: si.section_id,
    position: si.position,
    price_override_cents: si.price_override_cents,
    is_available: si.is_available,
    item: {
      id: si.item.id,
      isNew: false,
      restaurant_id: si.item.restaurant_id,
      name: si.item.name,
      description: si.item.description,
      base_price_cents: si.item.base_price_cents,
      image_url: si.item.image_url,
      active: si.item.active,
      tags: si.item.tags.map((t) => ({ id: t.id, name: t.name })),
    },
  };
}

function menuSectionToDraft(s: MenuSection): DraftSection {
  return {
    id: s.id,
    isNew: false,
    menu_id: s.menu_id,
    parent_id: s.parent_id,
    name: s.name,
    description: s.description,
    position: s.position,
    is_active: s.is_active,
    items: s.items.map(menuSectionItemToDraft),
    subsections: s.subsections.map(menuSectionToDraft),
  };
}

function menuToDraft(m: Menu): DraftMenu {
  return {
    id: m.id,
    restaurant_id: m.restaurant_id,
    name: m.name,
    description: m.description,
    is_active: m.is_active,
    permanent: m.permanent,
    sections: m.sections.map(menuSectionToDraft),
  };
}

function draftSectionItemToCreate(si: DraftSectionItem): CreateMenuSectionItem {
  return {
    position: si.position,
    price_override_cents: si.price_override_cents,
    is_available: si.is_available,
    item: {
      restaurant_id: si.item.restaurant_id,
      name: si.item.name,
      description: si.item.description,
      base_price_cents: si.item.base_price_cents,
      image_url: si.item.image_url,
      tags: si.item.tags,
    },
  };
}

function draftSectionToCreate(s: DraftSection): CreateMenuSection {
  return {
    name: s.name,
    description: s.description,
    position: s.position,
    is_active: s.is_active,
    items: s.items.map((si) => draftSectionItemToCreate(si)),
    subsections: s.subsections.map((sub) => draftSectionToCreate(sub)),
  };
}

function draftToCreateMenu(d: DraftMenu): CreateMenu {
  return {
    restaurant_id: d.restaurant_id,
    name: d.name,
    description: d.description,
    is_active: d.is_active,
    permanent: d.permanent,
    sections: d.sections.map((s) => draftSectionToCreate(s)),
  };
}

// ═══════════════════════════════════════════════════════════════════
// Action queue with merge logic
// ═══════════════════════════════════════════════════════════════════

/**
 * Get a merge key for an action. Actions with the same key can be merged.
 * Returns null if the action type doesn't support merging.
 */
function actionMergeKey(action: UpdateMenuAction): string | null {
  if ("UpdateMenu" in action) return "UpdateMenu";
  if ("UpdateMenuSection" in action) return `UpdateMenuSection:${action.UpdateMenuSection.section_id}`;
  if ("UpdateMenuSectionItem" in action) return `UpdateMenuSectionItem:${action.UpdateMenuSectionItem.item_id}`;
  if ("ChangePositionSection" in action) {
    const ref = action.ChangePositionSection.section_id;
    const refKey = "Existing" in ref ? ref.Existing : `created:${ref.CreatedByAction}`;
    return `ChangePositionSection:${refKey}`;
  }
  if ("ChangePositionItem" in action) {
    const ref = action.ChangePositionItem.item_id;
    const refKey = "Existing" in ref ? ref.Existing : `created:${ref.CreatedByAction}`;
    return `ChangePositionItem:${refKey}`;
  }
  if ("ChangeSectionForItem" in action) {
    const ref = action.ChangeSectionForItem.item_id;
    const refKey = "Existing" in ref ? ref.Existing : `created:${ref.CreatedByAction}`;
    return `ChangeSectionForItem:${refKey}`;
  }
  if ("ChangeSectionForSubSection" in action) {
    const ref = action.ChangeSectionForSubSection.subsection_id;
    const refKey = "Existing" in ref ? ref.Existing : `created:${ref.CreatedByAction}`;
    return `ChangeSectionForSubSection:${refKey}`;
  }
  // AddSection, AddItem — don't merge (each add is unique)
  return null;
}

/**
 * Merge two actions of the same type. Returns the merged action.
 * The `newer` action takes precedence for simple replacements.
 * For UpdateMenu/Section/Item, we merge fields (last-write-wins per field).
 */
function mergeActions(
  older: UpdateMenuAction,
  newer: UpdateMenuAction
): UpdateMenuAction {
  // UpdateMenu — merge optional fields
  if ("UpdateMenu" in older && "UpdateMenu" in newer) {
    return {
      UpdateMenu: {
        id: newer.UpdateMenu.id,
        name: newer.UpdateMenu.name ?? older.UpdateMenu.name,
        description: newer.UpdateMenu.description !== undefined ? newer.UpdateMenu.description : older.UpdateMenu.description,
        is_active: newer.UpdateMenu.is_active ?? older.UpdateMenu.is_active,
        permanent: newer.UpdateMenu.permanent ?? older.UpdateMenu.permanent,
      },
    };
  }

  // UpdateMenuSection — merge optional fields
  if ("UpdateMenuSection" in older && "UpdateMenuSection" in newer) {
    const o = older.UpdateMenuSection.update;
    const n = newer.UpdateMenuSection.update;
    return {
      UpdateMenuSection: {
        section_id: newer.UpdateMenuSection.section_id,
        update: {
          menu_id: n.menu_id ?? o.menu_id,
          parent_id: n.parent_id !== undefined ? n.parent_id : o.parent_id,
          name: n.name ?? o.name,
          description: n.description !== undefined ? n.description : o.description,
          position: n.position ?? o.position,
          is_active: n.is_active ?? o.is_active,
        },
      },
    };
  }

  // UpdateMenuSectionItem — merge optional fields
  if ("UpdateMenuSectionItem" in older && "UpdateMenuSectionItem" in newer) {
    const o = older.UpdateMenuSectionItem.update;
    const n = newer.UpdateMenuSectionItem.update;
    return {
      UpdateMenuSectionItem: {
        item_id: newer.UpdateMenuSectionItem.item_id,
        update: {
          section_id: n.section_id ?? o.section_id,
          position: n.position ?? o.position,
          price_override_cents: n.price_override_cents !== undefined ? n.price_override_cents : o.price_override_cents,
          is_available: n.is_available ?? o.is_available,
          item: n.item ?? o.item,
        },
      },
    };
  }

  // For position/move actions, just replace with newer
  return newer;
}

// ═══════════════════════════════════════════════════════════════════
// Store definition
// ═══════════════════════════════════════════════════════════════════

interface MenuEditorState {
  draft: DraftMenu;
  isNewMenu: boolean;
  /** Original menu snapshot (for edit mode — used to compute diff) */
  originalMenu: Menu | null;
}

const emptyDraft: DraftMenu = {
  id: null,
  restaurant_id: "",
  name: "",
  description: null,
  is_active: true,
  permanent: false,
  sections: [],
};

const [editorState, setEditorState] = createStore<MenuEditorState>({
  draft: { ...emptyDraft },
  isNewMenu: true,
  originalMenu: null,
});

const [actionQueue, setActionQueue] = createSignal<UpdateMenuAction[]>([]);
/** Maps temp IDs (sections and items) to the action queue index that created them. */
const tempIdToActionIndex = new Map<string, number>();
const [editorLoading, setEditorLoading] = createSignal(false);
const [editorError, setEditorError] = createSignal<string | null>(null);
const [editorSaving, setEditorSaving] = createSignal(false);
const [dirty, setDirty] = createSignal(false);

// ═══════════════════════════════════════════════════════════════════
// Initialization
// ═══════════════════════════════════════════════════════════════════

/**
 * Initialize the store for creating a new menu.
 */
function initNewMenu(restaurantId: string): void {
  batch(() => {
    setEditorState(
      reconcile({
        draft: {
          ...emptyDraft,
          restaurant_id: restaurantId,
          sections: [],
        },
        isNewMenu: true,
        originalMenu: null,
      })
    );
    setActionQueue([]);
    tempIdToActionIndex.clear();
    setEditorError(null);
    setDirty(false);
  });
}

/**
 * Load an existing menu into the editor for editing.
 */
function loadMenuForEditing(menu: Menu): void {
  batch(() => {
    setEditorState(
      reconcile({
        draft: menuToDraft(menu),
        isNewMenu: false,
        originalMenu: menu,
      })
    );
    setActionQueue([]);
    tempIdToActionIndex.clear();
    setEditorError(null);
    setDirty(false);
  });
}

/**
 * Fetch an existing menu from the API and load it.
 */
async function fetchAndLoadMenu(menuId: string): Promise<Menu | null> {
  try {
    setEditorLoading(true);
    setEditorError(null);

    const res = await fetch(`/api/menus/${menuId}`);
    if (!res.ok) {
      throw new Error(`GET /api/menus/${menuId} responded with ${res.status}`);
    }

    const json: ApiResponse<Menu> = await res.json();
    if (!json.success || json.data == null) {
      throw new Error(json.error ?? "Unexpected response");
    }

    loadMenuForEditing(json.data);
    return json.data;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setEditorError(msg);
    console.error("[menuEditorStore] fetchAndLoadMenu failed:", msg);
    return null;
  } finally {
    setEditorLoading(false);
  }
}

// ═══════════════════════════════════════════════════════════════════
// Action queue management
// ═══════════════════════════════════════════════════════════════════

/**
 * Push an action onto the queue with automatic merging.
 */
function pushAction(action: UpdateMenuAction): void {
  const key = actionMergeKey(action);

  if (key != null) {
    setActionQueue((prev) => {
      const existingIndex = prev.findIndex((a) => actionMergeKey(a) === key);
      if (existingIndex >= 0) {
        // Merge with existing
        const merged = mergeActions(prev[existingIndex], action);
        const next = [...prev];
        next[existingIndex] = merged;
        return next;
      }
      return [...prev, action];
    });
  } else {
    setActionQueue((prev) => [...prev, action]);
  }

  setDirty(true);
}

/**
 * Clear the entire action queue.
 */
function clearActions(): void {
  setActionQueue([]);
  tempIdToActionIndex.clear();
}

// ═══════════════════════════════════════════════════════════════════
// Tree path helpers
// ═══════════════════════════════════════════════════════════════════

/** A path to a section in the tree: array of indices into `sections`/`subsections`. */
type SectionPath = number[];

/**
 * Find the path to a section by ID (BFS through the tree).
 * Returns the path as an array of indices, or null if not found.
 */
function findSectionPath(sections: DraftSection[], targetId: string): SectionPath | null {
  for (let i = 0; i < sections.length; i++) {
    if (sections[i].id === targetId) return [i];
    const sub = findSectionPath(sections[i].subsections, targetId);
    if (sub != null) return [i, ...sub];
  }
  return null;
}

/**
 * Get a reference to a section by path.
 */
function getSectionByPath(sections: DraftSection[], path: SectionPath): DraftSection | null {
  if (path.length === 0) return null;
  let current = sections[path[0]];
  if (!current) return null;
  for (let i = 1; i < path.length; i++) {
    current = current.subsections[path[i]];
    if (!current) return null;
  }
  return current;
}

/**
 * Build the produce-path for setEditorState to reach a section.
 * E.g. path [1, 2] → draft.sections[1].subsections[2]
 */
function produceSectionUpdate(path: SectionPath, updater: (section: DraftSection) => void): void {
  setEditorState(
    produce((state) => {
      let current = state.draft.sections[path[0]];
      for (let i = 1; i < path.length; i++) {
        current = current.subsections[path[i]];
      }
      updater(current);
    })
  );
}

// ═══════════════════════════════════════════════════════════════════
// Draft mutations — Menu metadata
// ═══════════════════════════════════════════════════════════════════

function updateMenuName(name: string): void {
  setEditorState("draft", "name", name);
  setDirty(true);

  if (!editorState.isNewMenu && editorState.draft.id) {
    pushAction({
      UpdateMenu: {
        id: editorState.draft.id,
        name,
        description: null,
        is_active: null,
        permanent: null,
      },
    });
  }
}

function updateMenuDescription(description: string | null): void {
  setEditorState("draft", "description", description);
  setDirty(true);

  if (!editorState.isNewMenu && editorState.draft.id) {
    pushAction({
      UpdateMenu: {
        id: editorState.draft.id,
        name: null,
        description,
        is_active: null,
        permanent: null,
      },
    });
  }
}

function updateMenuIsActive(is_active: boolean): void {
  setEditorState("draft", "is_active", is_active);
  setDirty(true);

  if (!editorState.isNewMenu && editorState.draft.id) {
    pushAction({
      UpdateMenu: {
        id: editorState.draft.id,
        name: null,
        description: null,
        is_active,
        permanent: null,
      },
    });
  }
}

function updateMenuPermanent(permanent: boolean): void {
  setEditorState("draft", "permanent", permanent);
  setDirty(true);

  if (!editorState.isNewMenu && editorState.draft.id) {
    pushAction({
      UpdateMenu: {
        id: editorState.draft.id,
        name: null,
        description: null,
        is_active: null,
        permanent,
      },
    });
  }
}

// ═══════════════════════════════════════════════════════════════════
// Draft mutations — Sections
// ═══════════════════════════════════════════════════════════════════

/**
 * Add a new section to the menu (top-level) or as a subsection.
 * `parentId` is null for top-level sections.
 */
function addSection(parentId: string | null, name: string): DraftSection {
  const menuId = editorState.draft.id ?? "__draft__";
  const id = tempId();

  // Determine sibling list for position
  let siblings: DraftSection[];
  if (parentId == null) {
    siblings = editorState.draft.sections;
  } else {
    const path = findSectionPath(editorState.draft.sections, parentId);
    const parent = path ? getSectionByPath(editorState.draft.sections, path) : null;
    siblings = parent?.subsections ?? [];
  }

  const position = positionAtEnd(siblings);

  const newSection: DraftSection = {
    id,
    isNew: true,
    menu_id: menuId,
    parent_id: parentId,
    name,
    description: null,
    position,
    is_active: true,
    items: [],
    subsections: [],
  };

  if (parentId == null) {
    setEditorState(
      produce((state) => {
        state.draft.sections.push(newSection);
      })
    );
  } else {
    const path = findSectionPath(editorState.draft.sections, parentId);
    if (path) {
      produceSectionUpdate(path, (parent) => {
        parent.subsections.push(newSection);
      });
    }
  }

  setDirty(true);

  // In edit mode, push an AddSection action
  if (!editorState.isNewMenu) {
    const parentRef = parentId
      ? isTempId(parentId)
        ? { CreatedByAction: findActionIndexForTempSection(parentId) } as const
        : { Existing: parentId } as const
      : // Top-level section: parent is the menu itself
        editorState.draft.id
        ? { Existing: editorState.draft.id } as const
        : { Existing: "__draft__" } as const; // shouldn't happen in edit mode

    // Record the mapping BEFORE pushAction so we know the index.
    // AddSection never merges (actionMergeKey returns null), so it always appends.
    const actionIndex = actionQueue().length;
    tempIdToActionIndex.set(id, actionIndex);

    pushAction({
      AddSection: {
        parent_id: parentRef,
        section: {
          menu_id: menuId,
          parent_id: parentId,
          name,
          description: null,
          position,
          is_active: true,
        },
      },
    });
  }

  return newSection;
}

/**
 * Update a section's properties (name, description, is_active).
 */
function updateSection(
  sectionId: string,
  updates: { name?: string; description?: string | null; is_active?: boolean }
): void {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) return;

  produceSectionUpdate(path, (section) => {
    if (updates.name !== undefined) section.name = updates.name;
    if (updates.description !== undefined) section.description = updates.description;
    if (updates.is_active !== undefined) section.is_active = updates.is_active;
  });

  setDirty(true);

  if (!editorState.isNewMenu && !isTempId(sectionId)) {
    pushAction({
      UpdateMenuSection: {
        section_id: sectionId,
        update: {
          menu_id: null,
          parent_id: null,
          name: updates.name ?? null,
          description: updates.description !== undefined ? updates.description : null,
          position: null,
          is_active: updates.is_active ?? null,
        },
      },
    });
  }
}

/**
 * Remove a section (and all its children) from the draft.
 * In edit mode this is not directly an action — the backend doesn't have a
 * "delete section" action, so we'd set is_active=false for now.
 */
function removeSection(sectionId: string): void {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) return;

  if (path.length === 1) {
    // Top-level section
    setEditorState(
      produce((state) => {
        state.draft.sections.splice(path[0], 1);
      })
    );
  } else {
    // Subsection — remove from parent
    const parentPath = path.slice(0, -1);
    const indexInParent = path[path.length - 1];
    produceSectionUpdate(parentPath, (parent) => {
      parent.subsections.splice(indexInParent, 1);
    });
  }

  setDirty(true);

  // In edit mode, deactivate existing sections rather than delete
  if (!editorState.isNewMenu && !isTempId(sectionId)) {
    pushAction({
      UpdateMenuSection: {
        section_id: sectionId,
        update: {
          menu_id: null,
          parent_id: null,
          name: null,
          description: null,
          position: null,
          is_active: false,
        },
      },
    });
  }
}

// ═══════════════════════════════════════════════════════════════════
// Draft mutations — Items
// ═══════════════════════════════════════════════════════════════════

/**
 * Add a new item to a section.
 */
function addItemToSection(
  sectionId: string,
  itemData: {
    name: string;
    description?: string | null;
    base_price_cents: number;
    image_url?: string | null;
    tags?: { id: string | null; name: string | null }[];
  }
): DraftSectionItem {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) throw new Error(`Section ${sectionId} not found`);

  const section = getSectionByPath(editorState.draft.sections, path)!;
  const position = positionAtEnd(section.items);
  const id = tempId();

  const newItem: DraftSectionItem = {
    id,
    isNew: true,
    section_id: sectionId,
    position,
    price_override_cents: null,
    is_available: true,
    item: {
      id: tempId(),
      isNew: true,
      restaurant_id: editorState.draft.restaurant_id,
      name: itemData.name,
      description: itemData.description ?? null,
      base_price_cents: itemData.base_price_cents,
      image_url: itemData.image_url ?? null,
      active: true,
      tags: itemData.tags ?? [],
    },
  };

  produceSectionUpdate(path, (section) => {
    section.items.push(newItem);
  });

  setDirty(true);

  if (!editorState.isNewMenu) {
    const sectionRef = isTempId(sectionId)
      ? ({ CreatedByAction: findActionIndexForTempSection(sectionId) } as const)
      : ({ Existing: sectionId } as const);

    // Record the mapping BEFORE pushAction so we know the index.
    // AddItem never merges (actionMergeKey returns null), so it always appends.
    const actionIndex = actionQueue().length;
    tempIdToActionIndex.set(id, actionIndex);

    pushAction({
      AddItem: {
        section_id: sectionRef,
        item: {
          section_id: sectionId,
          position,
          price_override_cents: null,
          is_available: true,
          item: {
            restaurant_id: editorState.draft.restaurant_id,
            name: itemData.name,
            description: itemData.description ?? null,
            base_price_cents: itemData.base_price_cents,
            image_url: itemData.image_url ?? null,
            active: true,
            tags: itemData.tags ?? [],
          },
        },
      },
    });
  }

  return newItem;
}

/**
 * Update a section item's properties.
 */
function updateSectionItem(
  sectionId: string,
  itemId: string,
  updates: {
    price_override_cents?: number | null;
    is_available?: boolean;
    item?: Partial<{
      name: string;
      description: string | null;
      base_price_cents: number;
      image_url: string | null;
    }>;
  }
): void {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) return;

  produceSectionUpdate(path, (section) => {
    const idx = section.items.findIndex((i) => i.id === itemId);
    if (idx < 0) return;
    const si = section.items[idx];

    if (updates.price_override_cents !== undefined) si.price_override_cents = updates.price_override_cents;
    if (updates.is_available !== undefined) si.is_available = updates.is_available;
    if (updates.item) {
      if (updates.item.name !== undefined) si.item.name = updates.item.name;
      if (updates.item.description !== undefined) si.item.description = updates.item.description;
      if (updates.item.base_price_cents !== undefined) si.item.base_price_cents = updates.item.base_price_cents;
      if (updates.item.image_url !== undefined) si.item.image_url = updates.item.image_url;
    }
  });

  setDirty(true);

  if (!editorState.isNewMenu && !isTempId(itemId)) {
    pushAction({
      UpdateMenuSectionItem: {
        item_id: itemId,
        update: {
          section_id: null,
          position: null,
          price_override_cents: updates.price_override_cents !== undefined ? updates.price_override_cents : null,
          is_available: updates.is_available ?? null,
          item: updates.item
            ? {
                id: getSectionItemById(sectionId, itemId)?.item.id ?? "",
                name: updates.item.name ?? null,
                description: updates.item.description !== undefined ? updates.item.description : null,
                base_price_cents: updates.item.base_price_cents ?? null,
                image_url: updates.item.image_url !== undefined ? updates.item.image_url : null,
                active: null,
                tags: null,
              }
            : null,
        },
      },
    });
  }
}

/**
 * Remove an item from a section.
 */
function removeItem(sectionId: string, itemId: string): void {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) return;

  produceSectionUpdate(path, (section) => {
    const idx = section.items.findIndex((i) => i.id === itemId);
    if (idx >= 0) section.items.splice(idx, 1);
  });

  setDirty(true);

  // In edit mode, mark as unavailable
  if (!editorState.isNewMenu && !isTempId(itemId)) {
    pushAction({
      UpdateMenuSectionItem: {
        item_id: itemId,
        update: {
          section_id: null,
          position: null,
          price_override_cents: null,
          is_available: false,
          item: null,
        },
      },
    });
  }
}

// ═══════════════════════════════════════════════════════════════════
// Draft mutations — Reordering
// ═══════════════════════════════════════════════════════════════════

/**
 * Move a section to a new position within its sibling list.
 * `newIndex` is the desired index in the sorted sibling array.
 */
function moveSectionToIndex(sectionId: string, newIndex: number): void {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) return;

  // Get the sibling list
  let siblings: DraftSection[];
  if (path.length === 1) {
    siblings = editorState.draft.sections;
  } else {
    const parentPath = path.slice(0, -1);
    const parent = getSectionByPath(editorState.draft.sections, parentPath);
    siblings = parent?.subsections ?? [];
  }

  const sorted = [...siblings].sort((a, b) => a.position - b.position);
  const clampedIndex = Math.max(0, Math.min(newIndex, sorted.length - 1));

  // Compute new position
  const before = clampedIndex > 0 ? sorted[clampedIndex - 1].position : null;
  // Skip self when looking at "after" — if section is currently at clampedIndex
  const currentSortedIndex = sorted.findIndex((s) => s.id === sectionId);
  let afterIndex = clampedIndex;
  if (currentSortedIndex >= 0 && currentSortedIndex < clampedIndex) {
    afterIndex = clampedIndex; // After the item at clampedIndex
  }
  const afterItem = sorted[afterIndex + (afterIndex === clampedIndex && currentSortedIndex > clampedIndex ? 0 : 0)];
  const after = afterItem && afterItem.id !== sectionId ? afterItem.position : null;

  const newPosition = computePosition(before, after);

  // Update the section's position
  produceSectionUpdate(path, (section) => {
    section.position = newPosition;
  });

  setDirty(true);

  if (!editorState.isNewMenu) {
    const sectionRef = isTempId(sectionId)
      ? ({ CreatedByAction: findActionIndexForTempSection(sectionId) } as const)
      : ({ Existing: sectionId } as const);

    pushAction({
      ChangePositionSection: {
        section_id: sectionRef,
        position: newPosition,
      },
    });
  }
}

/**
 * Move an item to a new position within its section.
 */
function moveItemToIndex(sectionId: string, itemId: string, newIndex: number): void {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) return;

  const section = getSectionByPath(editorState.draft.sections, path);
  if (!section) return;

  const sorted = [...section.items].sort((a, b) => a.position - b.position);
  const clampedIndex = Math.max(0, Math.min(newIndex, sorted.length - 1));

  const before = clampedIndex > 0 ? sorted[clampedIndex - 1].position : null;
  const afterItem = sorted.find((_, i) => i === clampedIndex && sorted[i].id !== itemId)
    ?? sorted[clampedIndex];
  const after = afterItem && afterItem.id !== itemId ? afterItem.position : null;

  const newPosition = computePosition(before, after);

  produceSectionUpdate(path, (section) => {
    const idx = section.items.findIndex((i) => i.id === itemId);
    if (idx >= 0) section.items[idx].position = newPosition;
  });

  setDirty(true);

  if (!editorState.isNewMenu) {
    const itemRef = isTempId(itemId)
      ? ({ CreatedByAction: findActionIndexForTempItem(itemId) } as const)
      : ({ Existing: itemId } as const);

    pushAction({
      ChangePositionItem: {
        item_id: itemRef,
        position: newPosition,
      },
    });
  }
}

// ═══════════════════════════════════════════════════════════════════
// Lookup helpers
// ═══════════════════════════════════════════════════════════════════

function getSectionItemById(sectionId: string, itemId: string): DraftSectionItem | null {
  const path = findSectionPath(editorState.draft.sections, sectionId);
  if (!path) return null;
  const section = getSectionByPath(editorState.draft.sections, path);
  return section?.items.find((i) => i.id === itemId) ?? null;
}

/**
 * Find the action index that created a section with the given temp ID.
 * Used for building `CreatedByAction` references.
 */
function findActionIndexForTempSection(tempSectionId: string): number {
  const index = tempIdToActionIndex.get(tempSectionId);
  if (index !== undefined) return index;
  console.error(
    `[menuEditorStore] No action index found for temp section ID "${tempSectionId}". ` +
    `This means the section was never recorded via an AddSection action. ` +
    `Known temp IDs:`, [...tempIdToActionIndex.keys()]
  );
  throw new Error(`No action index found for temp section "${tempSectionId}"`);
}

function findActionIndexForTempItem(tempItemId: string): number {
  const index = tempIdToActionIndex.get(tempItemId);
  if (index !== undefined) return index;
  console.error(
    `[menuEditorStore] No action index found for temp item ID "${tempItemId}". ` +
    `This means the item was never recorded via an AddItem action. ` +
    `Known temp IDs:`, [...tempIdToActionIndex.keys()]
  );
  throw new Error(`No action index found for temp item "${tempItemId}"`);
}

// ═══════════════════════════════════════════════════════════════════
// Saving
// ═══════════════════════════════════════════════════════════════════

/**
 * Save the menu — create or update depending on mode.
 * Returns the saved `Menu` on success, or null on failure.
 */
async function saveMenu(): Promise<Menu | null> {
  try {
    setEditorSaving(true);
    setEditorError(null);

    if (editorState.isNewMenu) {
      return await saveNewMenu();
    } else {
      return await saveMenuUpdate();
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setEditorError(msg);
    console.error("[menuEditorStore] saveMenu failed:", msg);
    return null;
  } finally {
    setEditorSaving(false);
  }
}

async function saveNewMenu(): Promise<Menu> {
  const createPayload = draftToCreateMenu(editorState.draft);

  const res = await fetch("/api/menus", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(createPayload),
  });

  const json: ApiResponse<Menu> = await res.json();

  if (!res.ok || !json.success || json.data == null) {
    throw new Error(json.error ?? `Create menu failed with status ${res.status}`);
  }

  // Reload into editor with server-assigned IDs
  loadMenuForEditing(json.data);
  setDirty(false);
  return json.data;
}

async function saveMenuUpdate(): Promise<Menu> {
  const menuId = editorState.draft.id;
  if (!menuId) throw new Error("Cannot update a menu without an ID");

  const queue = actionQueue();
  if (queue.length === 0) {
    // Nothing to save
    setDirty(false);
    if (editorState.originalMenu) return editorState.originalMenu;
    throw new Error("No changes to save");
  }

  const payload: UpdateMenuActionsRequest = {
    menu_id: menuId,
    actions: queue,
  };

  const res = await fetch("/api/update-menu", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });

  const json: ApiResponse<Menu> = await res.json();

  if (!res.ok || !json.success || json.data == null) {
    throw new Error(json.error ?? `Update menu failed with status ${res.status}`);
  }

  // Reload with the server's response (canonical state)
  loadMenuForEditing(json.data);
  setDirty(false);
  return json.data;
}

// ═══════════════════════════════════════════════════════════════════
// Reset
// ═══════════════════════════════════════════════════════════════════

/**
 * Discard all changes and reload the original menu (edit mode)
 * or reset to empty (create mode).
 */
function discardChanges(): void {
  batch(() => {
    if (editorState.isNewMenu) {
      initNewMenu(editorState.draft.restaurant_id);
    } else if (editorState.originalMenu) {
      loadMenuForEditing(editorState.originalMenu);
    }
    setDirty(false);
  });
}

// ═══════════════════════════════════════════════════════════════════
// Exports
// ═══════════════════════════════════════════════════════════════════

export {
  // State (readonly)
  editorState,
  actionQueue,
  editorLoading,
  editorError,
  editorSaving,
  dirty,

  // Initialization
  initNewMenu,
  loadMenuForEditing,
  fetchAndLoadMenu,

  // Menu metadata
  updateMenuName,
  updateMenuDescription,
  updateMenuIsActive,
  updateMenuPermanent,

  // Sections
  addSection,
  updateSection,
  removeSection,

  // Items
  addItemToSection,
  updateSectionItem,
  removeItem,

  // Reordering
  moveSectionToIndex,
  moveItemToIndex,

  // Saving
  saveMenu,
  discardChanges,
  clearActions,

  // Helpers
  findSectionPath,
  getSectionByPath,
};
