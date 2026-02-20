import { Show } from "solid-js";
import { Card, ClickableCard } from "@/components/Card";

interface CreateModeSelectorProps {
  onSelectManual: () => void;
  onSelectAutomatic?: () => void;
}

export default function CreateModeSelector(props: CreateModeSelectorProps) {
  return (
    <div class="container" style={{ "max-width": "700px" }}>
      <div class="has-text-centered mb-5">
        <h2 class="title is-4">How would you like to create your menu?</h2>
        <p class="subtitle is-6 has-text-grey">
          Choose a method to get started.
        </p>
      </div>

      <div class="columns is-variable is-5">
        {/* Manual mode */}
        <div class="column is-6">
          <ClickableCard
            onClick={() => props.onSelectManual()}
            style={{ height: "100%" }}
          >
            <div class="card-content has-text-centered">
              <span style={{ "font-size": "3rem" }}>✏️</span>
              <p class="title is-5 mt-3 mb-2">Manual</p>
              <p class="has-text-grey is-size-6">
                Build your menu from scratch — add sections, items, prices, and
                organise them with drag & drop.
              </p>
              <div class="mt-4">
                <button class="button is-primary is-outlined">
                  Start building
                </button>
              </div>
            </div>
          </ClickableCard>
        </div>

        {/* Automatic mode (coming soon) */}
        <div class="column is-6">
          <ClickableCard
            onClick={() => {}}
            disabled
            style={{ height: "100%", position: "relative" }}
          >
            <div class="card-content has-text-centered">
              <span style={{ "font-size": "3rem" }}>📸</span>
              <p class="title is-5 mt-3 mb-2">Automatic</p>
              <p class="has-text-grey is-size-6">
                Upload a photo of a menu — OCR and AI will read it and build the
                structure for you automatically.
              </p>
              <div class="mt-4">
                <button class="button is-static" disabled>
                  <span class="icon is-small mr-1">🚧</span>
                  <span>Coming soon</span>
                </button>
              </div>
            </div>

            {/* "Coming soon" ribbon */}
            <Show when={true}>
              <span
                class="tag is-warning"
                style={{
                  position: "absolute",
                  top: "0.75rem",
                  right: "0.75rem",
                }}
              >
                Coming soon
              </span>
            </Show>
          </ClickableCard>
        </div>
      </div>
    </div>
  );
}