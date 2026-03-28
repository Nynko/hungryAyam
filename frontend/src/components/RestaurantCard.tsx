import { Show, createSignal, onCleanup } from "solid-js";
import { A } from "@solidjs/router";
import type { Restaurant } from "@bindings/Restaurant";
import { ClickableCard } from "@/components/Card";
import { availabilityStatus } from "@/lib/availability";

interface RestaurantCardProps {
  restaurant: Restaurant;
}

export default function RestaurantCard(props: RestaurantCardProps) {
  // Re-check availability every 60 seconds so the card updates live
  const [tick, setTick] = createSignal(0);
  const interval = setInterval(() => setTick((t) => t + 1), 60_000);
  onCleanup(() => clearInterval(interval));

  const status = () => {
    tick(); // subscribe to tick for periodic re-evaluation
    return availabilityStatus(props.restaurant.availability_rule);
  };

  return (
    <A
      href={`/restaurants/${props.restaurant.id}`}
      style={{ "text-decoration": "none", color: "inherit" }}
    >
      <ClickableCard onClick={() => {}} style={{ height: "100%" }}>
        <div class="card-image" style={{ position: "relative" }}>
          <figure
            class="image is-4by3"
            style={{
              "background-color": "var(--bulma-scheme-main-bis)",
              display: "flex",
              "align-items": "center",
              "justify-content": "center",
            }}
          >
            <Show
              when={props.restaurant.image_url}
              fallback={
                <span
                  style={{
                    "font-size": "4rem",
                    position: "absolute",
                    filter: !status().available ? "grayscale(1) opacity(0.5)" : undefined,
                  }}
                >
                  🍽️
                </span>
              }
            >
              <img
                src={props.restaurant.image_url!}
                alt={props.restaurant.name}
                style={{
                  "object-fit": "contain",
                  "max-width": "100%",
                  "max-height": "100%",
                  filter: !status().available ? "grayscale(1) opacity(0.5)" : undefined,
                  transition: "filter 0.3s ease",
                }}
              />
            </Show>
          </figure>

          {/* Unavailable overlay badge */}
          <Show when={!status().available}>
            <div
              style={{
                position: "absolute",
                top: "0",
                left: "0",
                right: "0",
                bottom: "0",
                display: "flex",
                "flex-direction": "column",
                "align-items": "center",
                "justify-content": "center",
                "background-color": "rgba(0, 0, 0, 0.15)",
                "pointer-events": "none",
              }}
            >
              <span
                class="tag is-dark"
                style={{
                  "font-size": "0.8rem",
                  "padding": "0.4em 0.8em",
                  "border-radius": "4px",
                }}
              >
                Currently unavailable
              </span>
            </div>
          </Show>
        </div>
        <div class="card-content">
          <p
            class="title is-5 has-text-centered mb-0"
            style={{
              opacity: !status().available ? "0.5" : "1",
              transition: "opacity 0.3s ease",
            }}
          >
            {props.restaurant.name}
          </p>
          <Show when={!status().available && status().reason}>
            <p
              class="has-text-grey has-text-centered is-size-7 mt-1"
              style={{ "font-style": "italic" }}
            >
              {status().reason}
            </p>
          </Show>
        </div>
      </ClickableCard>
    </A>
  );
}