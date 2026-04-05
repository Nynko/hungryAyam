import { Show, For } from "solid-js";
import type { MenuSectionItem } from "@bindings/MenuSectionItem";

interface MenuItemCardProps {
  sectionItem: MenuSectionItem;
}

/**
 * Format a price in cents to a display string (e.g. 1250 → "12.50").
 */
function formatPrice(cents: number): string {
  return (cents / 100).toFixed(2);
}

export default function MenuItemCard(props: MenuItemCardProps) {
  const item = () => props.sectionItem.item;
  const displayPrice = () => {
    const override = props.sectionItem.price_override_cents;
    return override != null ? override : item().base_price_cents;
  };
  const hasOverride = () => props.sectionItem.price_override_cents != null;
  const isAvailable = () => props.sectionItem.is_available && item().active;

  return (
    <div
      class="box p-3"
      classList={{ "has-background-light": !isAvailable() }}
      style={{
        position: "relative",
      }}
    >
      <div class="columns is-mobile is-vcentered is-gapless">
        {/* Image (if present) */}
        <Show when={item().image_url}>
          <div class="column is-narrow mr-3">
            <figure
              class="image is-64x64"
              style={{
                "border-radius": "8px",
                overflow: "hidden",
                "min-width": "64px",
                filter: !isAvailable() ? "grayscale(0.25)" : undefined,
                opacity: !isAvailable() ? "0.7" : undefined,
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
            <div>
              <p class="has-text-weight-semibold is-size-6 mb-0">
                {item().name}
                <Show when={!isAvailable()}>
                  <span class="tag is-warning ml-2" style={{ "vertical-align": "middle" }}>
                    Unavailable
                  </span>
                </Show>
              </p>
              <Show when={item().description}>
                <p class="has-text-grey is-size-7 mt-1">{item().description}</p>
              </Show>
              {/* Tags */}
              <Show when={item().tags.length > 0}>
                <div class="tags mt-1 mb-0">
                  <For each={item().tags}>
                    {(tag) => (
                      <span class="tag is-info is-light is-small">{tag.name}</span>
                    )}
                  </For>
                </div>
              </Show>
            </div>

            {/* Price */}
            <div class="has-text-right ml-3" style={{ "white-space": "nowrap" }}>
              <p class="has-text-weight-bold is-size-6">
                ${formatPrice(displayPrice())}
              </p>
              <Show when={hasOverride()}>
                <p
                  class="has-text-grey is-size-7"
                  style={{ "text-decoration": "line-through" }}
                >
                  ${formatPrice(item().base_price_cents)}
                </p>
              </Show>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}