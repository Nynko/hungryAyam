import { ClickableCard } from "@/components/Card";

interface CreateModeSelectorProps {
  onSelectManual: () => void;
  onSelectAutomatic: () => void;
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

        {/* Automatic mode */}
        <div class="column is-6">
          <ClickableCard
            onClick={() => props.onSelectAutomatic()}
            style={{ height: "100%" }}
          >
            <div class="card-content has-text-centered">
              <span style={{ "font-size": "3rem" }}>📸</span>
              <p class="title is-5 mt-3 mb-2">Automatic</p>
              <p class="has-text-grey is-size-6">
                Upload photos of a menu — AI will read it and build the
                structure for you automatically.
              </p>
              <div class="mt-4">
                <button class="button is-primary is-outlined">
                  Upload photos
                </button>
              </div>
            </div>
          </ClickableCard>
        </div>
      </div>
    </div>
  );
}