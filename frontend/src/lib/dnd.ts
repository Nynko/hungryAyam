/**
 * Drag-and-drop utilities for SolidJS + Atlassian Pragmatic Drag and Drop.
 *
 * Provides helpers to wire up draggable / drop-target / monitor behaviour
 * using SolidJS signals and lifecycle (`onMount` / `onCleanup`).
 *
 * Key design decisions:
 * - `getData` is a **getter function** (not a static object) so that
 *   `getInitialData` and the drop-target `getData` always return fresh
 *   values (e.g. current `sortedIndex`) even after reordering.
 * - Cleanup is returned to the caller instead of calling `onCleanup`
 *   internally, because setup may happen outside a reactive tracking
 *   scope (e.g. inside `queueMicrotask`). Callers should use
 *   `onCleanup(state.cleanup)` inside a `createEffect` or `onMount`.
 */

import { createSignal } from "solid-js";
import {
  draggable,
  dropTargetForElements,
  monitorForElements,
} from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { combine } from "@atlaskit/pragmatic-drag-and-drop/combine";
import {
  attachClosestEdge,
  extractClosestEdge,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/closest-edge";
import type { Edge } from "@atlaskit/pragmatic-drag-and-drop-hitbox/types";

// Re-export for convenience
export { extractClosestEdge };
export type { Edge };

// ─── Drag data types ──────────────────────────────────────────────

export interface SectionDragData {
  type: "section";
  id: string;
  parentId: string | null;
  index: number;
}

export interface ItemDragData {
  type: "item";
  id: string;
  sectionId: string;
  index: number;
}

export interface SlotDragData {
  type: "slot";
  id: string;
  index: number;
}

export type DragData = SectionDragData | ItemDragData | SlotDragData;

// ─── Type guards ──────────────────────────────────────────────────

export function isSectionDragData(data: Record<string, unknown>): boolean {
  return data.type === "section" && typeof data.id === "string";
}

export function isItemDragData(data: Record<string, unknown>): boolean {
  return data.type === "item" && typeof data.id === "string" && typeof data.sectionId === "string";
}

export function isSlotDragData(data: Record<string, unknown>): boolean {
  return data.type === "slot" && typeof data.id === "string";
}

// ─── Index computation ────────────────────────────────────────────

/**
 * Given a source index, destination index, and closest edge ("top"/"bottom"),
 * compute the final index the source should move to.
 *
 * This handles the shift that occurs when removing the source from its
 * original position before inserting at the destination.
 */
export function computeReorderIndex(
  sourceIndex: number,
  destIndex: number,
  edge: Edge | null,
): number {
  // If no edge detected, just use destIndex directly
  if (edge === null) return destIndex;

  // Target index based on edge
  let targetIndex = edge === "bottom" ? destIndex + 1 : destIndex;

  // If the source is before the target, removing it shifts everything down by 1
  if (sourceIndex < targetIndex) {
    targetIndex -= 1;
  }

  return Math.max(0, targetIndex);
}

// ─── SolidJS sortable item setup ──────────────────────────────────

export interface SortableItemConfig {
  /** The outer element that is both draggable and a drop target. */
  element: HTMLElement;
  /** Optional drag handle element. If omitted, the whole element is the handle. */
  dragHandle?: HTMLElement;
  /**
   * Getter that returns the current drag data for this item.
   * Called at drag-start and during drag-over so indices are always fresh.
   */
  getData: () => DragData;
  /** The drag type(s) this drop target accepts. */
  acceptType: DragData["type"];
  /**
   * Extra predicate for canDrop. Receives the source data.
   * Useful to restrict drops to the same parent/section.
   */
  canDrop?: (sourceData: Record<string, unknown>) => boolean;
}

export interface SortableItemState {
  /** Whether this item is currently being dragged. */
  isDragging: () => boolean;
  /** The closest edge when another item is dragged over this one. */
  closestEdge: () => Edge | null;
  /**
   * Cleanup function — tears down all draggable + drop-target listeners.
   * Callers should wire this into `onCleanup` in a proper tracking scope.
   */
  cleanup: () => void;
}

/**
 * Wire up a DOM element as both a draggable and a drop target for reordering.
 *
 * Call this inside `createEffect` or `onMount`, then register
 * `onCleanup(state.cleanup)` yourself to ensure proper teardown.
 *
 * Returns reactive signals for `isDragging` and `closestEdge`.
 */
export function setupSortableItem(config: SortableItemConfig): SortableItemState {
  const [isDragging, setIsDragging] = createSignal(false);
  const [closestEdge, setClosestEdge] = createSignal<Edge | null>(null);

  const cleanup = combine(
    draggable({
      element: config.element,
      dragHandle: config.dragHandle,
      getInitialData: () => ({ ...config.getData() }),
      onDragStart: () => setIsDragging(true),
      onDrop: () => {
        setIsDragging(false);
      },
    }),
    dropTargetForElements({
      element: config.element,
      canDrop: ({ source }) => {
        const data = config.getData();
        // Don't drop on self
        if (source.data.id === data.id && source.data.type === data.type) {
          return false;
        }
        // Must match the accepted type
        if (source.data.type !== config.acceptType) {
          return false;
        }
        // Custom predicate
        if (config.canDrop && !config.canDrop(source.data)) {
          return false;
        }
        return true;
      },
      getData: ({ input, element }) =>
        attachClosestEdge(
          { ...config.getData() },
          { input, element, allowedEdges: ["top", "bottom"] },
        ),
      getIsSticky: () => true,
      onDragEnter: ({ self }) => {
        setClosestEdge(extractClosestEdge(self.data));
      },
      onDrag: ({ self }) => {
        const edge = extractClosestEdge(self.data);
        // Only update if the edge actually changed to avoid unnecessary re-renders
        setClosestEdge((current) => (current === edge ? current : edge));
      },
      onDragLeave: () => {
        setClosestEdge(null);
      },
      onDrop: () => {
        setClosestEdge(null);
      },
    }),
  );

  // NOTE: We intentionally do NOT call `onCleanup(cleanup)` here.
  // The caller must wire cleanup into a proper SolidJS tracking scope.

  return { isDragging, closestEdge, cleanup };
}

// ─── SolidJS sortable monitor setup ───────────────────────────────

export interface SortableMonitorConfig {
  /** The drag type this monitor handles. */
  type: DragData["type"];
  /**
   * Called when a valid drop completes.
   * Receives the source ID, source index, destination index (already computed
   * from the closest edge), and raw source/dest data.
   */
  onReorder: (
    sourceId: string,
    sourceIndex: number,
    destinationIndex: number,
    sourceData: Record<string, unknown>,
    destData: Record<string, unknown>,
  ) => void;
  /**
   * Extra predicate for canMonitor. Receives the source data.
   */
  canMonitor?: (sourceData: Record<string, unknown>) => boolean;
}

/**
 * Set up a monitor that listens for drop events and calls `onReorder`.
 *
 * Returns a cleanup function. The caller should wire this into
 * `onCleanup` in a proper tracking scope.
 */
export function setupSortableMonitor(config: SortableMonitorConfig): () => void {
  const cleanup = monitorForElements({
    canMonitor: ({ source }) => {
      if (source.data.type !== config.type) return false;
      if (config.canMonitor && !config.canMonitor(source.data)) return false;
      return true;
    },
    onDrop: ({ source, location }) => {
      const destination = location.current.dropTargets[0];
      if (!destination) return;

      const sourceData = source.data;
      const destData = destination.data;

      const sourceIndex = sourceData.index as number;
      const destIndex = destData.index as number;
      const edge = extractClosestEdge(destData);

      const newIndex = computeReorderIndex(sourceIndex, destIndex, edge);

      // Don't fire if nothing changed
      if (newIndex === sourceIndex) return;

      config.onReorder(
        sourceData.id as string,
        sourceIndex,
        newIndex,
        sourceData,
        destData,
      );
    },
  });

  return cleanup;
}