import { Show, For } from "solid-js";
import type { MenuSection } from "@bindings/MenuSection";
import MenuItemCard from "./MenuItemCard";

interface MenuSectionViewProps {
  section: MenuSection;
  /** Current nesting depth (0 = top-level). Used for visual indentation. */
  depth?: number;
}

export default function MenuSectionView(props: MenuSectionViewProps) {
  const depth = () => props.depth ?? 0;

  // Choose heading size based on depth: h4 → h5 → h6
  const headingClass = () => {
    switch (depth()) {
      case 0:
        return "is-size-4";
      case 1:
        return "is-size-5";
      default:
        return "is-size-6";
    }
  };

  const availableItems = () =>
    props.section.items.filter((si) => si.is_available && si.item.active);

  const unavailableItems = () =>
    props.section.items.filter((si) => !si.is_available || !si.item.active);

  const sortedItems = () =>
    [...availableItems(), ...unavailableItems()].sort(
      (a, b) => a.position - b.position
    );

  const activeSubsections = () =>
    props.section.subsections
      .filter((s) => s.is_active)
      .sort((a, b) => a.position - b.position);

  return (
    <div
      style={{
        "margin-left": depth() > 0 ? "1.25rem" : "0",
        "border-left": depth() > 0 ? "3px solid var(--bulma-border)" : "none",
        "padding-left": depth() > 0 ? "1rem" : "0",
      }}
    >
      {/* Section header */}
      <div class="mb-3 mt-4">
        <p class={`has-text-weight-bold ${headingClass()}`}>
          {props.section.name}
        </p>
        <Show when={props.section.description}>
          <p class="has-text-grey is-size-6 mt-1">{props.section.description}</p>
        </Show>
      </div>

      {/* Items in this section */}
      <Show when={sortedItems().length > 0}>
        <div class="mb-3">
          <For each={sortedItems()}>
            {(sectionItem) => (
              <div class="mb-2">
                <MenuItemCard sectionItem={sectionItem} />
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Recursive subsections */}
      <Show when={activeSubsections().length > 0}>
        <For each={activeSubsections()}>
          {(subsection) => (
            <MenuSectionView section={subsection} depth={depth() + 1} />
          )}
        </For>
      </Show>

      {/* Empty section */}
      <Show
        when={
          sortedItems().length === 0 && activeSubsections().length === 0
        }
      >
        <p class="has-text-grey-light is-size-7 is-italic ml-2 mb-3">
          No items in this section.
        </p>
      </Show>
    </div>
  );
}