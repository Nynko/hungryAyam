import { Show, For, createSignal, createResource, onMount } from "solid-js";
import type { Offer } from "@bindings/Offer";
import {
  fetchActiveOffers,
  formatOfferPrice,
} from "@/stores/offerStore";

interface OfferBannerProps {
  restaurantId: string;
  /** Called when user clicks "Compose" on an offer. */
  onComposeOffer: (offer: Offer) => void;
}

export default function OfferBanner(props: OfferBannerProps) {
  const [offers, setOffers] = createSignal<Offer[]>([]);
  const [loading, setLoading] = createSignal(true);

  onMount(async () => {
    try {
      const active = await fetchActiveOffers(props.restaurantId);
      setOffers(active);
    } finally {
      setLoading(false);
    }
  });

  /** Summary text for an offer's slots. */
  const slotsSummary = (offer: Offer): string => {
    return offer.slots
      .map((slot) => {
        const label = slot.label;
        if (slot.min_items === 0) {
          return `${label} (optional)`;
        }
        if (slot.min_items === slot.max_items) {
          return slot.max_items === 1 ? label : `${slot.max_items}× ${label}`;
        }
        return `${slot.min_items}–${slot.max_items}× ${label}`;
      })
      .join(" + ");
  };

  /** Check if an offer has any supplements (slot-level or constraint-level). */
  const hasSupplements = (offer: Offer): boolean => {
    return offer.slots.some(
      (slot) =>
        slot.supplement_cents > 0 ||
        slot.constraints.some((c) => c.supplement_cents > 0),
    );
  };

  return (
    <Show when={!loading() && offers().length > 0}>
      <div class="mb-5">
        <h3 class="title is-5 mb-3">
          <span class="mr-2">🏷️</span>
          Available Offers
        </h3>

        <div class="columns is-multiline">
          <For each={offers()}>
            {(offer) => (
              <div class="column is-12">
                <div
                  class="box card-clickable"
                  style={{
                    "border-left": "4px solid var(--bulma-border)",
                  }}
                  onClick={() => props.onComposeOffer(offer)}
                >
                  <div class="is-flex is-justify-content-space-between is-align-items-center is-flex-wrap-wrap" style={{ gap: "0.75rem" }}>
                    {/* Left: offer info */}
                    <div style={{ flex: "1", "min-width": "200px" }}>
                      <div class="is-flex is-align-items-center mb-1" style={{ gap: "0.5rem" }}>
                        <span class="is-size-4">🍽️</span>
                        <span class="has-text-weight-bold is-size-5">
                          {offer.title}
                        </span>
                      </div>

                      <Show when={offer.description}>
                        <p class="has-text-grey is-size-6 mb-2">
                          {offer.description}
                        </p>
                      </Show>

                      <p class="is-size-7 has-text-grey-dark">
                        {slotsSummary(offer)}
                      </p>
                    </div>

                    {/* Right: price + action */}
                    <div class="has-text-right">
                      <p class="has-text-weight-bold is-size-4 has-text-primary mb-1">
                        €{formatOfferPrice(offer.base_price_cents)}
                        <Show when={hasSupplements(offer)}>
                          <span class="is-size-7 has-text-grey has-text-weight-normal ml-1">
                            +suppl.
                          </span>
                        </Show>
                      </p>

                      <button
                        class="button is-primary is-small is-rounded"
                        onClick={(e) => {
                          e.stopPropagation();
                          props.onComposeOffer(offer);
                        }}
                      >
                        <span class="icon is-small">
                          <span>✨</span>
                        </span>
                        <span>Compose</span>
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>

      {/* Loading state */}
      <Show when={loading()}>
        <div class="has-text-centered py-3">
          <progress class="progress is-primary is-small" max="100" />
          <p class="has-text-grey is-size-7 mt-1">Loading offers…</p>
        </div>
      </Show>
    </Show>
  );
}