import { Show, For, createMemo } from "solid-js";
import type { Item } from "@bindings/Item";
import { formatOfferPrice } from "@/stores/offerStore";

interface OfferSlotItemCardProps {
  item: Item;
  /** Constraint-level supplement for this item in the current slot (cents). */
  supplementCents: number;
  /** How many times this item is currently selected in the slot. */
  quantity: number;
  /** Max selectable for the whole slot (used to disable add when full). */
  slotFull: boolean;
  /** Called when the user wants to add one of this item. */
  onAdd: () => void;
  /** Called when the user wants to remove one of this item. */
  onRemove: () => void;
}

export default function OfferSlotItemCard(props: OfferSlotItemCardProps) {
  const item = () => props.item;
  const isActive = () => item().active;
  const hasSupplement = () => props.supplementCents > 0;

  const canAdd = () => isActive() && !props.slotFull;

  return (
    <div
      class="box p-3 mb-2"
      style={{
        opacity: isActive() ? "1" : "0.45",
        position: "relative",
        border: props.quantity > 0 ? "2px solid hsl(141, 53%, 53%)" : "1px solid transparent",
        "background-color": props.quantity > 0 ? "hsl(141, 53%, 97%)" : undefined,
        transition: "border-color 0.15s ease, background-color 0.15s ease",
      }}
    >
      <div class="columns is-mobile is-vcentered is-gapless">
        {/* Image (if present) */}
        <Show when={item().image_url}>
          <div class="column is-narrow mr-3">
            <figure
              class="image is-48x48"
              style={{
                "border-radius": "8px",
                overflow: "hidden",
                "min-width": "48px",
              }}
            >
              <img
                src={item().image_url!}
                alt={item().name}
                style={{
                  "object-fit": "cover",
                  width: "100%",
                  height: "100%",
                }}
              />
            </figure>
          </div>
        </Show>

        {/* Item details */}
        <div class="column">
          <div class="is-flex is-justify-content-space-between is-align-items-flex-start">
            <div style={{ flex: "1", "min-width": "0" }}>
              <p class="has-text-weight-semibold is-size-6 mb-0">
                {item().name}
                <Show when={!isActive()}>
                  <span
                    class="tag is-warning is-light ml-2"
                    style={{ "vertical-align": "middle", "font-size": "0.65rem" }}
                  >
                    Unavailable
                  </span>
                </Show>
              </p>

              <Show when={item().description}>
                <p
                  class="has-text-grey is-size-7 mt-1"
                  style={{
                    overflow: "hidden",
                    "text-overflow": "ellipsis",
                    "white-space": "nowrap",
                    "max-width": "300px",
                  }}
                >
                  {item().description}
                </p>
              </Show>

              {/* Tags */}
              <Show when={item().tags.length > 0}>
                <div class="tags mt-1 mb-0">
                  <For each={item().tags}>
                    {(tag) => (
                      <span class="tag is-info is-light is-small" style={{ "font-size": "0.6rem" }}>
                        {tag.name}
                      </span>
                    )}
                  </For>
                </div>
              </Show>
            </div>

            {/* Right: supplement badge + quantity controls */}
            <div
              class="has-text-right ml-3"
              style={{
                "white-space": "nowrap",
                display: "flex",
                "flex-direction": "column",
                "align-items": "flex-end",
                gap: "0.3rem",
                "flex-shrink": "0",
              }}
            >
              {/* Supplement badge */}
              <Show
                when={hasSupplement()}
                fallback={
                  <span class="tag is-success is-light is-small">
                    Included
                  </span>
                }
              >
                <span class="tag is-warning is-light is-small">
                  +${formatOfferPrice(props.supplementCents)}
                </span>
              </Show>

              {/* Quantity controls */}
              <Show when={isActive()}>
                <div
                  class="is-flex is-align-items-center"
                  style={{ gap: "0.3rem" }}
                >
                  <Show
                    when={props.quantity > 0}
                    fallback={
                      <button
                        class="button is-small is-primary is-outlined"
                        disabled={!canAdd()}
                        onClick={(e) => {
                          e.stopPropagation();
                          props.onAdd();
                        }}
                        title={
                          props.slotFull
                            ? "Slot is full — remove an item first"
                            : `Add ${item().name}`
                        }
                      >
                        <span class="icon is-small">
                          <span>＋</span>
                        </span>
                        <span>Add</span>
                      </button>
                    }
                  >
                    {/* Decrement */}
                    <button
                      class="button is-small is-light"
                      onClick={(e) => {
                        e.stopPropagation();
                        props.onRemove();
                      }}
                      title="Remove one"
                    >
                      −
                    </button>

                    {/* Current quantity */}
                    <span
                      class="has-text-weight-bold"
                      style={{
                        "min-width": "1.4rem",
                        "text-align": "center",
                        "font-size": "0.9rem",
                      }}
                    >
                      {props.quantity}
                    </span>

                    {/* Increment */}
                    <button
                      class="button is-small is-light"
                      disabled={!canAdd()}
                      onClick={(e) => {
                        e.stopPropagation();
                        props.onAdd();
                      }}
                      title={
                        props.slotFull
                          ? "Slot is full — remove an item first"
                          : "Add one more"
                      }
                    >
                      +
                    </button>
                  </Show>
                </div>
              </Show>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}