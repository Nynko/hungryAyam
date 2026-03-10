import { Show, For, createSignal, createEffect, createMemo, onMount, onCleanup } from "solid-js";
import type { DraftSection, DraftSectionItem } from "@/stores/menuEditorStore";
import {
  editorState,
  addSection,
  updateSection,
  removeSection,
  addItemToSection,
  updateSectionItem,
  moveSectionToIndex,
  moveItemToIndex,
} from "@/stores/menuEditorStore";
import SectionItemEditor from "./SectionItemEditor";
import { setupSortableItem, setupSortableMonitor } from "@/lib/dnd";
import type { SortableItemState } from "@/lib/dnd";
import DropIndicator from "./DropIndicator";

/**
 * Normalize a string for fuzzy matching: lowercase, trim, collapse whitespace.
 */
function normalize(s: string): string {
  return s.toLowerCase().trim().replace(/\s+/g, " ");
}

/**
 * Simple similarity check between two item names.
 * Returns true if the names are "close enough" to warn about duplicates.
 * Uses normalized prefix/substring matching + Levenshtein-like heuristic.
 */
function isSimilarName(a: string, b: string): boolean {
  const na = normalize(a);
  const nb = normalize(b);
  if (!na || !nb) return false;
  if (na === nb) return true;
  // One contains the other
  if (na.includes(nb) || nb.includes(na)) return true;
  // Levenshtein distance <= 2 for short names
  if (na.length <= 20 && nb.length <= 20) {
    const dist = levenshtein(na, nb);
    const maxLen = Math.max(na.length, nb.length);
    // Allow distance of ~15% of the longer string, min 2
    if (dist <= Math.max(2, Math.floor(maxLen * 0.15))) return true;
  }
  return false;
}

function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));
  for (let i = 0; i <= m; i++) dp[i][0] = i;
  for (let j = 0; j <= n; j++) dp[0][j] = j;
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      dp[i][j] = Math.min(dp[i - 1][j] + 1, dp[i][j - 1] + 1, dp[i - 1][j - 1] + cost);
    }
  }
  return dp[m][n];
}

/**
 * Collect all item names from all sections in the current menu draft (recursively).
 */
function collectAllItemNames(sections: DraftSection[]): string[] {
  const names: string[] = [];
  for (const section of sections) {
    for (const si of section.items) {
      if (si.item.name) names.push(si.item.name);
    }
    names.push(...collectAllItemNames(section.subsections));
  }
  return names;
}

interface SectionEditorProps {
  section: DraftSection;
  /** Current nesting depth (0 = top-level). */
  depth?: number;
  /** Total number of siblings (for move up/down bounds). */
  siblingCount: number;
  /** Index of this section among its sorted siblings. */
  sortedIndex: number;
  /** Callback when user requests moving this section up/down. */
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  /** Whether section-level drag-and-drop is enabled (default true). */
  draggable?: boolean;
  /** When true, only allow toggling item availability and adding items.
   *  Hides section name/desc editing, reorder, remove, active toggle,
   *  and add subsection controls. */
  availabilityOnly?: boolean;
}

export default function SectionEditor(props: SectionEditorProps) {
  const depth = () => props.depth ?? 0;
  const isDraggable = () => props.draggable !== false;

  const [collapsed, setCollapsed] = createSignal(false);
  const [editingName, setEditingName] = createSignal(false);
  const [nameValue, setNameValue] = createSignal(props.section.name);
  const [editingDesc, setEditingDesc] = createSignal(false);
  const [descValue, setDescValue] = createSignal(props.section.description ?? "");

  const [showAddItem, setShowAddItem] = createSignal(false);
  const [newItemName, setNewItemName] = createSignal("");
  const [newItemPrice, setNewItemPrice] = createSignal("");

  const [showAddSubsection, setShowAddSubsection] = createSignal(false);
  const [newSubsectionName, setNewSubsectionName] = createSignal("");

  const [confirmRemove, setConfirmRemove] = createSignal(false);

  // ── Search / filter state ──────────────────────────────────────
  const [searchQuery, setSearchQuery] = createSignal("");
  const [showFilter, setShowFilter] = createSignal(false);
  /** Filter: "all" | "available" | "unavailable" */
  const [availabilityFilter, setAvailabilityFilter] = createSignal<"all" | "available" | "unavailable">("all");

  // ── Duplicate detection state ──────────────────────────────────
  const [duplicateWarning, setDuplicateWarning] = createSignal<string | null>(null);

  // ── Drag-and-drop state ────────────────────────────────────────
  let sectionContainerRef!: HTMLDivElement;
  let sectionHandleRef!: HTMLSpanElement;

  const [sectionIsDragging, setSectionIsDragging] = createSignal(false);
  const [sectionClosestEdge, setSectionClosestEdge] = createSignal<ReturnType<SortableItemState["closestEdge"]>>(null);

  // Set up this section as a draggable + drop target inside createEffect
  // so that onCleanup properly tears down listeners when the component unmounts.
  createEffect(() => {
    const el = sectionContainerRef;
    const handle = sectionHandleRef;
    if (!el || !handle || !isDraggable()) return;

    const state = setupSortableItem({
      element: el,
      dragHandle: handle,
      getData: () => ({
        type: "section" as const,
        id: props.section.id,
        parentId: props.section.parent_id,
        index: props.sortedIndex,
      }),
      acceptType: "section",
      // Only allow drops from sections with the same parent
      canDrop: (src) => src.parentId === props.section.parent_id,
    });

    // Bridge the returned signals into our local ones
    createEffect(() => setSectionIsDragging(state.isDragging()));
    createEffect(() => setSectionClosestEdge(state.closestEdge()));

    onCleanup(state.cleanup);
  });

  // Set up item reorder monitor for this section's items
  onMount(() => {
    const cleanup = setupSortableMonitor({
      type: "item",
      canMonitor: (src) => src.sectionId === props.section.id,
      onReorder: (sourceId, _sourceIndex, destinationIndex) => {
        moveItemToIndex(props.section.id, sourceId, destinationIndex);
      },
    });
    onCleanup(cleanup);
  });

  // Set up subsection reorder monitor for this section's subsections
  onMount(() => {
    const cleanup = setupSortableMonitor({
      type: "section",
      // Only handle subsections whose parent is this section
      canMonitor: (src) => src.parentId === props.section.id,
      onReorder: (sourceId, _sourceIndex, destinationIndex) => {
        moveSectionToIndex(sourceId, destinationIndex);
      },
    });
    onCleanup(cleanup);
  });

  // ── Sorted & filtered children ─────────────────────────────────
  const allSortedItems = () =>
    [...props.section.items].sort((a, b) => a.position - b.position);

  const sortedItems = () => {
    let items = allSortedItems();
    const query = normalize(searchQuery());
    const filter = availabilityFilter();

    // Apply availability filter
    if (filter === "available") {
      items = items.filter((i) => i.is_available);
    } else if (filter === "unavailable") {
      items = items.filter((i) => !i.is_available);
    }

    // Apply search query
    if (query) {
      items = items.filter((i) => {
        const name = normalize(i.item.name);
        const desc = normalize(i.item.description ?? "");
        return name.includes(query) || desc.includes(query);
      });
    }

    return items;
  };

  // ── Availability stats ─────────────────────────────────────────
  const availableCount = createMemo(() => props.section.items.filter((i) => i.is_available).length);
  const unavailableCount = createMemo(() => props.section.items.filter((i) => !i.is_available).length);
  const totalCount = createMemo(() => props.section.items.length);
  const isFiltering = () => !!searchQuery() || availabilityFilter() !== "all";

  const sortedSubsections = () =>
    [...props.section.subsections].sort((a, b) => a.position - b.position);

  // ── Section name editing ───────────────────────────────────────
  const commitName = () => {
    const trimmed = nameValue().trim();
    if (trimmed && trimmed !== props.section.name) {
      updateSection(props.section.id, { name: trimmed });
    } else {
      setNameValue(props.section.name);
    }
    setEditingName(false);
  };

  // ── Section description editing ────────────────────────────────
  const commitDescription = () => {
    const trimmed = descValue().trim();
    const newDesc = trimmed || null;
    if (newDesc !== props.section.description) {
      updateSection(props.section.id, { description: newDesc });
    }
    setEditingDesc(false);
  };

  // ── Toggle active ──────────────────────────────────────────────
  const handleToggleActive = () => {
    updateSection(props.section.id, { is_active: !props.section.is_active });
  };

  // ── Add item (with duplicate detection) ────────────────────────
  const checkDuplicate = (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) {
      setDuplicateWarning(null);
      return;
    }
    const allNames = collectAllItemNames(editorState.draft.sections);
    const similar = allNames.find((existing) => isSimilarName(trimmed, existing));
    if (similar) {
      setDuplicateWarning(`Similar item exists: "${similar}"`);
    } else {
      setDuplicateWarning(null);
    }
  };

  const handleAddItem = () => {
    const name = newItemName().trim();
    const priceStr = newItemPrice().trim();
    if (!name) return;

    const priceCents = priceStr ? Math.round(parseFloat(priceStr) * 100) : 0;
    if (isNaN(priceCents) || priceCents < 0) return;

    addItemToSection(props.section.id, {
      name,
      base_price_cents: priceCents,
    });

    setNewItemName("");
    setNewItemPrice("");
    setShowAddItem(false);
    setDuplicateWarning(null);
  };

  // ── Bulk availability toggle ───────────────────────────────────
  const handleBulkActivate = () => {
    const items = sortedItems();
    for (const si of items) {
      if (!si.is_available) {
        updateSectionItem(props.section.id, si.id, { is_available: true });
      }
    }
  };

  const handleBulkDeactivate = () => {
    const items = sortedItems();
    for (const si of items) {
      if (si.is_available) {
        updateSectionItem(props.section.id, si.id, { is_available: false });
      }
    }
  };

  // ── Add subsection ─────────────────────────────────────────────
  const handleAddSubsection = () => {
    const name = newSubsectionName().trim();
    if (!name) return;
    addSection(props.section.id, name);
    setNewSubsectionName("");
    setShowAddSubsection(false);
  };

  // ── Remove section ─────────────────────────────────────────────
  const handleRemove = () => {
    if (confirmRemove()) {
      removeSection(props.section.id);
      setConfirmRemove(false);
    } else {
      setConfirmRemove(true);
      setTimeout(() => setConfirmRemove(false), 3000);
    }
  };

  // ── Heading size based on depth ────────────────────────────────
  const headingClass = () => {
    switch (depth()) {
      case 0:
        return "is-size-5";
      case 1:
        return "is-size-6";
      default:
        return "is-size-6";
    }
  };

  const borderColor = () => {
    if (props.section.isNew) return "hsl(204, 86%, 53%)"; // info blue
    switch (depth()) {
      case 0:
        return "hsl(171, 100%, 41%)"; // primary/turquoise
      case 1:
        return "hsl(48, 100%, 67%)"; // warning/yellow
      default:
        return "hsl(0, 0%, 86%)"; // grey-light
    }
  };

  return (
    <div
      ref={(el) => { sectionContainerRef = el; }}
      class="box mb-4 p-3"
      style={{
        position: "relative",
        "border-left": `4px solid ${borderColor()}`,
        opacity: sectionIsDragging() ? "0.4" : props.section.is_active ? "1" : "0.6",
        "margin-left": depth() > 0 ? "0.75rem" : "0",
      }}
    >
      <DropIndicator edge={sectionClosestEdge()} gap="1rem" />
      {/* ── Section header ──────────────────────────────────── */}
      <div class="is-flex is-justify-content-space-between is-align-items-flex-start">
        {/* Left: drag handle + name + badges */}
        <div class="is-flex is-align-items-center" style={{ flex: "1", "min-width": "0" }}>
          {/* Drag handle */}
          <span
            ref={(el) => { sectionHandleRef = el; }}
            class="drag-handle has-text-grey-light mr-2"
            style={{ "font-size": "1.1rem" }}
            title="Drag to reorder"
          >
            ⠿
          </span>

          {/* Name — click to edit (read-only in availabilityOnly mode) */}
          <Show
            when={!props.availabilityOnly}
            fallback={
              <span class={`has-text-weight-bold ${headingClass()}`}>
                {props.section.name || "(unnamed section)"}
              </span>
            }
          >
            <Show
              when={!editingName()}
              fallback={
                <div class="field has-addons mb-0" style={{ flex: "1" }}>
                  <div class="control is-expanded">
                    <input
                      class="input is-small"
                      type="text"
                      value={nameValue()}
                      onInput={(e) => setNameValue(e.currentTarget.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") commitName();
                        if (e.key === "Escape") {
                          setNameValue(props.section.name);
                          setEditingName(false);
                        }
                      }}
                      ref={(el) => setTimeout(() => el.focus(), 0)}
                    />
                  </div>
                  <div class="control">
                    <button class="button is-small is-primary" onClick={commitName}>
                      ✓
                    </button>
                  </div>
                  <div class="control">
                    <button
                      class="button is-small is-light"
                      onClick={() => {
                        setNameValue(props.section.name);
                        setEditingName(false);
                      }}
                    >
                      ✕
                    </button>
                  </div>
                </div>
              }
            >
              <span
                class={`has-text-weight-bold ${headingClass()} is-clickable`}
                style={{ cursor: "text" }}
                onClick={() => {
                  setNameValue(props.section.name);
                  setEditingName(true);
                }}
                title="Click to edit name"
              >
                {props.section.name || "(unnamed section)"}
              </span>
            </Show>
          </Show>

          {/* Badges */}
          <Show when={!props.section.is_active}>
            <span class="tag is-warning is-light is-small ml-2">Inactive</span>
          </Show>
          <Show when={props.section.isNew}>
            <span class="tag is-info is-light is-small ml-2">New</span>
          </Show>

          {/* Item/subsection counts */}
          <span class="has-text-grey is-size-7 ml-3" style={{ "white-space": "nowrap" }}>
            {props.section.items.length} item{props.section.items.length !== 1 ? "s" : ""}
            <Show when={props.section.subsections.length > 0}>
              {" · "}
              {props.section.subsections.length} sub
            </Show>
          </span>
        </div>

        {/* Right: controls */}
        <div class="is-flex is-align-items-center ml-3" style={{ "flex-shrink": "0" }}>
          {/* Move up/down (hidden in availabilityOnly) */}
          <Show when={!props.availabilityOnly && props.sortedIndex > 0}>
            <button
              class="button is-small is-light mr-1"
              title="Move up"
              onClick={() => props.onMoveUp?.()}
            >
              ↑
            </button>
          </Show>
          <Show when={!props.availabilityOnly && props.sortedIndex < props.siblingCount - 1}>
            <button
              class="button is-small is-light mr-1"
              title="Move down"
              onClick={() => props.onMoveDown?.()}
            >
              ↓
            </button>
          </Show>

          {/* Toggle active (hidden in availabilityOnly) */}
          <Show when={!props.availabilityOnly}>
            <button
              class={`button is-small mr-1 ${props.section.is_active ? "is-success is-light" : "is-warning is-light"}`}
              title={props.section.is_active ? "Deactivate" : "Activate"}
              onClick={handleToggleActive}
            >
              {props.section.is_active ? "👁" : "👁‍🗨"}
            </button>
          </Show>

          {/* Collapse/expand */}
          <button
            class="button is-small is-light mr-1"
            title={collapsed() ? "Expand" : "Collapse"}
            onClick={() => setCollapsed(!collapsed())}
          >
            {collapsed() ? "▶" : "▼"}
          </button>

          {/* Remove (hidden in availabilityOnly) */}
          <Show when={!props.availabilityOnly}>
            <button
              class={`button is-small ${confirmRemove() ? "is-danger" : "is-danger is-outlined"}`}
              title="Remove section"
              onClick={handleRemove}
            >
              {confirmRemove() ? "Confirm?" : "🗑"}
            </button>
          </Show>
        </div>
      </div>

      {/* ── Description ────────────────────────────────────── */}
      <Show when={!collapsed()}>
        {/* Description — editable or read-only depending on mode */}
        <Show
          when={!props.availabilityOnly}
          fallback={
            <Show when={props.section.description}>
              <p class="has-text-grey is-size-7 mt-1">
                {props.section.description}
              </p>
            </Show>
          }
        >
          <Show
            when={!editingDesc()}
            fallback={
              <div class="field has-addons mt-2 mb-0">
                <div class="control is-expanded">
                  <input
                    class="input is-small"
                    type="text"
                    placeholder="Section description (optional)"
                    value={descValue()}
                    onInput={(e) => setDescValue(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") commitDescription();
                      if (e.key === "Escape") {
                        setDescValue(props.section.description ?? "");
                        setEditingDesc(false);
                      }
                    }}
                    ref={(el) => setTimeout(() => el.focus(), 0)}
                  />
                </div>
                <div class="control">
                  <button class="button is-small is-primary" onClick={commitDescription}>
                    ✓
                  </button>
                </div>
                <div class="control">
                  <button
                    class="button is-small is-light"
                    onClick={() => {
                      setDescValue(props.section.description ?? "");
                      setEditingDesc(false);
                    }}
                  >
                    ✕
                  </button>
                </div>
              </div>
            }
          >
            <p
              class="has-text-grey is-size-7 mt-1 is-clickable"
              style={{ cursor: "text", "min-height": "1.2em" }}
              onClick={() => {
                setDescValue(props.section.description ?? "");
                setEditingDesc(true);
              }}
              title="Click to edit description"
            >
              {props.section.description || (
                <span class="has-text-grey-light is-italic">
                  Click to add a description…
                </span>
              )}
            </p>
          </Show>
        </Show>

        {/* ── Items toolbar (search, filter, bulk actions) ──── */}
        <Show when={totalCount() > 0}>
          <div class="mt-3 mb-2">
            {/* Stats bar */}
            <div class="is-flex is-justify-content-space-between is-align-items-center is-flex-wrap-wrap mb-2" style={{ gap: "0.5rem" }}>
              <div class="is-flex is-align-items-center" style={{ gap: "0.5rem" }}>
                <span class="is-size-7 has-text-grey">
                  {totalCount()} item{totalCount() !== 1 ? "s" : ""}
                </span>
                <Show when={availableCount() > 0}>
                  <span class="tag is-success is-light is-small">
                    {availableCount()} available
                  </span>
                </Show>
                <Show when={unavailableCount() > 0}>
                  <span class="tag is-warning is-light is-small">
                    {unavailableCount()} unavailable
                  </span>
                </Show>
              </div>
              <div class="buttons are-small">
                <button
                  class={`button is-small ${showFilter() ? "is-info" : "is-light"}`}
                  onClick={() => {
                    setShowFilter(!showFilter());
                    if (!showFilter()) {
                      setSearchQuery("");
                      setAvailabilityFilter("all");
                    }
                  }}
                  title="Search & filter items"
                >
                  <span class="icon is-small"><span>🔍</span></span>
                  <span>Filter</span>
                </button>
                <Show when={unavailableCount() > 0}>
                  <button
                    class="button is-small is-success is-outlined"
                    onClick={handleBulkActivate}
                    title={isFiltering() ? "Activate all filtered items" : "Activate all items in this section"}
                  >
                    <span class="icon is-small"><span>✅</span></span>
                    <span>Activate {isFiltering() ? "filtered" : "all"}</span>
                  </button>
                </Show>
                <Show when={availableCount() > 0}>
                  <button
                    class="button is-small is-warning is-outlined"
                    onClick={handleBulkDeactivate}
                    title={isFiltering() ? "Deactivate all filtered items" : "Deactivate all items in this section"}
                  >
                    <span class="icon is-small"><span>⏸</span></span>
                    <span>Deactivate {isFiltering() ? "filtered" : "all"}</span>
                  </button>
                </Show>
              </div>
            </div>

            {/* Search & filter bar */}
            <Show when={showFilter()}>
              <div class="box p-3 mb-2 has-background-light">
                <div class="columns is-mobile is-variable is-2 mb-0">
                  <div class="column">
                    <div class="control has-icons-left">
                      <input
                        class="input is-small"
                        type="text"
                        placeholder="Search items by name or description…"
                        value={searchQuery()}
                        onInput={(e) => setSearchQuery(e.currentTarget.value)}
                        ref={(el) => setTimeout(() => el.focus(), 0)}
                      />
                      <span class="icon is-left is-small">🔍</span>
                    </div>
                  </div>
                  <div class="column is-narrow">
                    <div class="select is-small">
                      <select
                        value={availabilityFilter()}
                        onChange={(e) => setAvailabilityFilter(e.currentTarget.value as "all" | "available" | "unavailable")}
                      >
                        <option value="all">All items</option>
                        <option value="available">✅ Available only</option>
                        <option value="unavailable">⏸ Unavailable only</option>
                      </select>
                    </div>
                  </div>
                  <Show when={isFiltering()}>
                    <div class="column is-narrow">
                      <button
                        class="button is-small is-light"
                        onClick={() => {
                          setSearchQuery("");
                          setAvailabilityFilter("all");
                        }}
                        title="Clear filters"
                      >
                        ✕ Clear
                      </button>
                    </div>
                  </Show>
                </div>
                <Show when={isFiltering()}>
                  <p class="is-size-7 has-text-grey mt-1">
                    Showing {sortedItems().length} of {totalCount()} item{totalCount() !== 1 ? "s" : ""}
                  </p>
                </Show>
              </div>
            </Show>
          </div>
        </Show>

        {/* ── Items list ────────────────────────────────────── */}
        <Show when={sortedItems().length > 0}>
          <div class="mt-1">
            <For each={sortedItems()}>
              {(sectionItem, index) => (
                <SectionItemEditor
                  sectionId={props.section.id}
                  sectionItem={sectionItem}
                  sortedIndex={index()}
                  availabilityOnly={props.availabilityOnly}
                />
              )}
            </For>
          </div>
        </Show>

        <Show when={sortedItems().length === 0 && totalCount() > 0 && isFiltering()}>
          <p class="has-text-grey is-size-7 is-italic mt-3 ml-2">
            No items match your filter. Try adjusting your search or filter.
          </p>
        </Show>

        <Show when={totalCount() === 0 && sortedSubsections().length === 0}>
          <p class="has-text-grey-light is-size-7 is-italic mt-3 ml-2">
            This section is empty — add items or subsections below.
          </p>
        </Show>

        {/* ── Add item form ─────────────────────────────────── */}
        <Show
          when={showAddItem()}
          fallback={
            <button
              class="button is-small is-info is-outlined mt-2 mr-2"
              onClick={() => setShowAddItem(true)}
            >
              <span class="icon is-small">
                <span>➕</span>
              </span>
              <span>Add item</span>
            </button>
          }
        >
          <div class="box p-3 mt-3 has-background-info-light">
            <p class="has-text-weight-semibold is-size-7 mb-2">New item</p>
            {/* Duplicate warning */}
            <Show when={duplicateWarning()}>
              <div class="notification is-warning is-light py-2 px-3 mb-2 is-size-7">
                ⚠️ {duplicateWarning()} — you can still add it if intended.
              </div>
            </Show>
            <div class="columns is-mobile is-variable is-2 mb-0">
              <div class="column">
                <div class="control">
                  <input
                    class="input is-small"
                    classList={{ "is-warning": !!duplicateWarning() }}
                    type="text"
                    placeholder="Item name"
                    value={newItemName()}
                    onInput={(e) => {
                      setNewItemName(e.currentTarget.value);
                      checkDuplicate(e.currentTarget.value);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleAddItem();
                      if (e.key === "Escape") {
                        setShowAddItem(false);
                        setDuplicateWarning(null);
                      }
                    }}
                    ref={(el) => setTimeout(() => el.focus(), 0)}
                  />
                </div>
              </div>
              <div class="column is-narrow">
                <div class="control">
                  <input
                    class="input is-small"
                    type="number"
                    step="0.01"
                    min="0"
                    placeholder="Price ($)"
                    value={newItemPrice()}
                    onInput={(e) => setNewItemPrice(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleAddItem();
                      if (e.key === "Escape") {
                        setShowAddItem(false);
                        setDuplicateWarning(null);
                      }
                    }}
                    style={{ width: "100px" }}
                  />
                </div>
              </div>
              <div class="column is-narrow">
                <div class="buttons">
                  <button
                    class="button is-small is-primary"
                    disabled={!newItemName().trim()}
                    onClick={handleAddItem}
                  >
                    Add
                  </button>
                  <button
                    class="button is-small is-light"
                    onClick={() => {
                      setShowAddItem(false);
                      setNewItemName("");
                      setNewItemPrice("");
                      setDuplicateWarning(null);
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Show>

        {/* ── Subsections ───────────────────────────────────── */}
        <Show when={sortedSubsections().length > 0}>
          <div class="mt-4">
            <For each={sortedSubsections()}>
              {(subsection, index) => (
                <SectionEditor
                  section={subsection}
                  depth={depth() + 1}
                  siblingCount={sortedSubsections().length}
                  sortedIndex={index()}
                  draggable={!props.availabilityOnly}
                  availabilityOnly={props.availabilityOnly}
                  onMoveUp={() => moveSectionToIndex(subsection.id, index() - 1)}
                  onMoveDown={() => moveSectionToIndex(subsection.id, index() + 1)}
                />
              )}
            </For>
          </div>
        </Show>

        {/* ── Add subsection form (hidden in availabilityOnly) ── */}
        <Show when={!props.availabilityOnly}>
          <Show
            when={showAddSubsection()}
            fallback={
              <button
                class="button is-small is-primary is-outlined mt-2"
                onClick={() => setShowAddSubsection(true)}
              >
                <span class="icon is-small">
                  <span>📁</span>
                </span>
                <span>Add subsection</span>
              </button>
            }
          >
            <div class="box p-3 mt-3 has-background-success-light">
              <p class="has-text-weight-semibold is-size-7 mb-2">New subsection</p>
              <div class="field has-addons">
                <div class="control is-expanded">
                  <input
                    class="input is-small"
                    type="text"
                    placeholder="Subsection name"
                    value={newSubsectionName()}
                    onInput={(e) => setNewSubsectionName(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleAddSubsection();
                      if (e.key === "Escape") setShowAddSubsection(false);
                    }}
                    ref={(el) => setTimeout(() => el.focus(), 0)}
                  />
                </div>
                <div class="control">
                  <button
                    class="button is-small is-primary"
                    disabled={!newSubsectionName().trim()}
                    onClick={handleAddSubsection}
                  >
                    Add
                  </button>
                </div>
                <div class="control">
                  <button
                    class="button is-small is-light"
                    onClick={() => {
                      setShowAddSubsection(false);
                      setNewSubsectionName("");
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          </Show>
        </Show>
      </Show>
    </div>
  );
}