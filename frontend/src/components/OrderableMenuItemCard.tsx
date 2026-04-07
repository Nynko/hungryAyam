import { Show, For, createMemo, createSignal } from "solid-js";
import { isImageSrc } from "@/lib/imageUrl";
import type { MenuSectionItem } from "@bindings/MenuSectionItem";
import { addToCart, getCart, formatPrice } from "@/stores/orderStore";
import ImageLightbox from "@/components/ImageLightbox";

interface OrderableMenuItemCardProps {
  sectionItem: MenuSectionItem;
  restaurantId: string;
}

export default function OrderableMenuItemCard(props: OrderableMenuItemCardProps) {
  const item = () => props.sectionItem.item;
  const displayPrice = () => {
    const override = props.sectionItem.price_override_cents;
    return override != null ? override : item().base_price_cents;
  };
  const hasOverride = () => props.sectionItem.price_override_cents != null;
  const isAvailable = () => props.sectionItem.is_available && item().active;

  /** How many times this specific item is already in the cart. */
  const quantityInCart = createMemo(() => {
    const cart = getCart(props.restaurantId);
    return cart.filter((ci) => ci.sectionItem.item.id === item().id).length;
  });

  const [lightboxOpen, setLightboxOpen] = createSignal(false);

  const handleAdd = () => {
    if (!isAvailable()) return;
    addToCart(props.restaurantId, props.sectionItem);
  };

  return (
    <div
      class="box p-3"
      classList={{
        "has-background-light": !isAvailable(),
      }}
      style={{
        position: "relative",
      }}
    >
      <div class="columns is-mobile is-vcentered is-gapless">
        {/* Image / emoji (if present) */}
        <Show when={item().image_url}>
          <div class="column is-narrow mr-3">
            <div
              style={{
                width: "64px",
                height: "64px",
                "min-width": "64px",
                "border-radius": "8px",
                overflow: "hidden",
                display: "flex",
                "align-items": "center",
                "justify-content": "center",
                "background-color": "var(--bulma-scheme-main-bis)",
                filter: !isAvailable() ? "grayscale(0.25)" : undefined,
                opacity: !isAvailable() ? "0.7" : undefined,
              }}
            >
              <Show
                when={isImageSrc(item().image_url!)}
                fallback={
                  <span style={{ "font-size": "2rem", "line-height": "1" }}>
                    {item().image_url}
                  </span>
                }
              >
                <img
                  src={item().image_url!}
                  alt={item().name}
                  style={{ "object-fit": "cover", width: "100%", height: "100%", cursor: "zoom-in" }}
                  onClick={() => setLightboxOpen(true)}
                />
              </Show>
              <Show when={lightboxOpen()}>
                <ImageLightbox
                  src={item().image_url!}
                  alt={item().name}
                  onClose={() => setLightboxOpen(false)}
                />
              </Show>
            </div>
          </div>
        </Show>

        {/* Item details */}
        <div class="column">
          <div class="is-flex is-justify-content-space-between is-align-items-flex-start">
            <div style={{ flex: "1", "min-width": "0" }}>
              <p class="has-text-weight-semibold is-size-6 mb-0">
                {item().name}
                <Show when={!isAvailable()}>
                  <span
                    class="tag is-warning ml-2"
                    style={{ "vertical-align": "middle" }}
                  >
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
                      <span class="tag is-info is-light is-small">
                        {tag.name}
                      </span>
                    )}
                  </For>
                </div>
              </Show>
            </div>

            {/* Price + Add button */}
            <div
              class="has-text-right ml-3"
              style={{
                "white-space": "nowrap",
                display: "flex",
                "flex-direction": "column",
                "align-items": "flex-end",
                gap: "0.35rem",
              }}
            >
              <p class="has-text-weight-bold is-size-6 mb-0">
                €{formatPrice(displayPrice())}
              </p>
              <Show when={hasOverride()}>
                <p
                  class="has-text-grey is-size-7 mb-0"
                  style={{ "text-decoration": "line-through" }}
                >
                  €{formatPrice(item().base_price_cents)}
                </p>
              </Show>

              <Show
                when={isAvailable()}
                fallback={
                  <span class="tag is-warning is-small">N/A</span>
                }
              >
                <button
                  class="button is-small is-primary is-outlined"
                  style={{ position: "relative" }}
                  onClick={handleAdd}
                  title={`Add ${item().name} to cart`}
                >
                  <span class="icon is-small">
                    <span>＋</span>
                  </span>
                  <span>Add</span>

                  {/* Quantity badge */}
                  <Show when={quantityInCart() > 0}>
                    <span
                      class="tag is-primary is-rounded"
                      style={{
                        position: "absolute",
                        top: "-8px",
                        right: "-8px",
                        "min-width": "20px",
                        height: "20px",
                        "font-size": "0.65rem",
                        "padding-left": "5px",
                        "padding-right": "5px",
                        "pointer-events": "none",
                      }}
                    >
                      {quantityInCart()}
                    </span>
                  </Show>
                </button>
              </Show>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}