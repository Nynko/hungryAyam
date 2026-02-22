import { Show, For, createSignal, createEffect, onMount, onCleanup } from "solid-js";
import type { DraftSection } from "@/stores/menuEditorStore";
import {
  addSection,
  updateSection,
  removeSection,
  addItemToSection,
  moveSectionToIndex,
  moveItemToIndex,
} from "@/stores/menuEditorStore";
import SectionItemEditor from "./SectionItemEditor";
import { setupSortableItem, setupSortableMonitor } from "@/lib/dnd";
import type { SortableItemState } from "@/lib/dnd";
import DropIndicator from "./DropIndicator";

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

  // ── Sorted children ────────────────────────────────────────────
  const sortedItems = () =>
    [...props.section.items].sort((a, b) => a.position - b.position);

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

  // ── Add item ───────────────────────────────────────────────────
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

          {/* Name — click to edit */}
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
          {/* Move up/down */}
          <Show when={props.sortedIndex > 0}>
            <button
              class="button is-small is-light mr-1"
              title="Move up"
              onClick={() => props.onMoveUp?.()}
            >
              ↑
            </button>
          </Show>
          <Show when={props.sortedIndex < props.siblingCount - 1}>
            <button
              class="button is-small is-light mr-1"
              title="Move down"
              onClick={() => props.onMoveDown?.()}
            >
              ↓
            </button>
          </Show>

          {/* Toggle active */}
          <button
            class={`button is-small mr-1 ${props.section.is_active ? "is-success is-light" : "is-warning is-light"}`}
            title={props.section.is_active ? "Deactivate" : "Activate"}
            onClick={handleToggleActive}
          >
            {props.section.is_active ? "👁" : "👁‍🗨"}
          </button>

          {/* Collapse/expand */}
          <button
            class="button is-small is-light mr-1"
            title={collapsed() ? "Expand" : "Collapse"}
            onClick={() => setCollapsed(!collapsed())}
          >
            {collapsed() ? "▶" : "▼"}
          </button>

          {/* Remove */}
          <button
            class={`button is-small ${confirmRemove() ? "is-danger" : "is-danger is-outlined"}`}
            title="Remove section"
            onClick={handleRemove}
          >
            {confirmRemove() ? "Confirm?" : "🗑"}
          </button>
        </div>
      </div>

      {/* ── Description ────────────────────────────────────── */}
      <Show when={!collapsed()}>
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

        {/* ── Items list ────────────────────────────────────── */}
        <Show when={sortedItems().length > 0}>
          <div class="mt-3">
            <For each={sortedItems()}>
              {(sectionItem, index) => (
                <SectionItemEditor
                  sectionId={props.section.id}
                  sectionItem={sectionItem}
                  sortedIndex={index()}
                />
              )}
            </For>
          </div>
        </Show>

        <Show when={sortedItems().length === 0 && sortedSubsections().length === 0}>
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
            <div class="columns is-mobile is-variable is-2 mb-0">
              <div class="column">
                <div class="control">
                  <input
                    class="input is-small"
                    type="text"
                    placeholder="Item name"
                    value={newItemName()}
                    onInput={(e) => setNewItemName(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleAddItem();
                      if (e.key === "Escape") setShowAddItem(false);
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
                      if (e.key === "Escape") setShowAddItem(false);
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
                  draggable={true}
                  onMoveUp={() => moveSectionToIndex(subsection.id, index() - 1)}
                  onMoveDown={() => moveSectionToIndex(subsection.id, index() + 1)}
                />
              )}
            </For>
          </div>
        </Show>

        {/* ── Add subsection form ───────────────────────────── */}
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
    </div>
  );
}