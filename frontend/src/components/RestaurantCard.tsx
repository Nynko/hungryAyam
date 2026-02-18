import { Show } from "solid-js";
import { A } from "@solidjs/router";
import type { Restaurant } from "@bindings/Restaurant";

interface RestaurantCardProps {
  restaurant: Restaurant;
}

export default function RestaurantCard(props: RestaurantCardProps) {
  return (
    <A
      href={`/restaurants/${props.restaurant.id}`}
      style={{ "text-decoration": "none", color: "inherit" }}
    >
      <div class="card" style={{ cursor: "pointer", height: "100%" }}>
        <div class="card-image">
          <figure
            class="image is-4by3"
            style={{
              "background-color": "#f5f5f5",
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
                }}
              />
            </Show>
          </figure>
        </div>
        <div class="card-content">
          <p class="title is-5 has-text-centered mb-0">
            {props.restaurant.name}
          </p>
        </div>
      </div>
    </A>
  );
}