import { Show, createSignal } from "solid-js";
import type { DraftSectionItem } from "@/stores/menuEditorStore";
import { updateSectionItem, removeItem } from "@/stores/menuEditorStore";

interface SectionItemEditorProps {
  sectionId: string;
  sectionItem: DraftSectionItem;
}

/**
 * Format a price in cents to a display string (e.g. 1250 → "12.50").
 */
function formatPrice(cents: number): string {
  return (cents / 100).toFixed(2);
}

/**
 * Parse a price string (e.g. "12.50") to cents (1250).
 * Returns null if the input is not a valid price.
 */
function parsePriceCents(value: string): number | null {
  const trimmed = value.trim();
  if (trimmed === "") return null;
  const num = parseFloat(trimmed);
  if (isNaN(num) || num < 0) return null;
  return Math.round(num * 100);
}

export default function SectionItemEditor(props: SectionItemEditorProps) {
  const [expanded, setExpanded] = createSignal(false);
  const [confirmRemove, setConfirmRemove] = createSignal(false);

  const item = () => props.sectionItem.item;
  const displayPrice = () => {
    const override = props.sectionItem.price_override_cents;
    return override != null ? override : item().base_price_cents;
  };

  // ── Inline editing handlers ────────────────────────────────────

  const handleNameChange = (name: string) => {
    updateSectionItem(props.sectionId, props.sectionItem.id, {
      item: { name },
    });
  };

  const handleDescriptionChange = (description: string) => {
    updateSectionItem(props.sectionId, props.sectionItem.id, {
      item: { description: description || null },
    });
  };

  const handleBasePriceChange = (value: string) => {
    const cents = parsePriceCents(value);
    if (cents != null) {
      updateSectionItem(props.sectionId, props.sectionItem.id, {
        item: { base_price_cents: cents },
      });
    }
  };

  const handlePriceOverrideChange = (value: string) => {
    const trimmed = value.trim();
    if (trimmed === "") {
      // Clear override
      updateSectionItem(props.sectionId, props.sectionItem.id, {
        price_override_cents: null,
      });
    } else {
      const cents = parsePriceCents(value);
      if (cents != null) {
        updateSectionItem(props.sectionId, props.sectionItem.id, {
          price_override_cents: cents,
        });
      }
    }
  };

  const handleAvailabilityToggle = () => {
    updateSectionItem(props.sectionId, props.sectionItem.id, {
      is_available: !props.sectionItem.is_available,
    });
  };

  const handleImageUrlChange = (url: string) => {
    updateSectionItem(props.sectionId, props.sectionItem.id, {
      item: { image_url: url || null },
    });
  };

  const handleRemove = () => {
    if (confirmRemove()) {
      removeItem(props.sectionId, props.sectionItem.id);
      setConfirmRemove(false);
    } else {
      setConfirmRemove(true);
      // Auto-dismiss confirm after 3 seconds
      setTimeout(() => setConfirmRemove(false), 3000);
    }
  };

  return (
    <div
      class="box p-3 mb-2"
      style={{
        opacity: props.sectionItem.is_available ? "1" : "0.6",
        "border-left": props.sectionItem.isNew ? "3px solid hsl(204, 86%, 53%)" : "3px solid transparent",
      }}
    >
      {/* ── Collapsed row ──────────────────────────────────── */}
      <div class="is-flex is-justify-content-space-between is-align-items-center">
        <div
          class="is-flex is-align-items-center is-clickable"
          style={{ flex: "1", "min-width": "0", cursor: "pointer" }}
          onClick={() => setExpanded(!expanded())}
        >
          {/* Drag handle placeholder */}
          <span
            class="has-text-grey-light mr-2"
            style={{ cursor: "grab", "user-select": "none" }}
            title="Drag to reorder"
          >
            ⠿
          </span>

          {/* Thumbnail */}
          <Show when={item().image_url}>
            <figure
              class="image is-32x32 mr-2"
              style={{
                "border-radius": "4px",
                overflow: "hidden",
                "min-width": "32px",
                "flex-shrink": "0",
              }}
            >
              <img
                src={item().image_url!}
                alt={item().name}
                style={{ "object-fit": "cover", width: "100%", height: "100%" }}
              />
            </figure>
          </Show>

          {/* Name + availability */}
          <div style={{ "min-width": "0", flex: "1" }}>
            <span class="has-text-weight-semibold is-size-6">
              {item().name || "(unnamed)"}
            </span>
            <Show when={!props.sectionItem.is_available}>
              <span class="tag is-warning is-light is-small ml-2" style={{ "vertical-align": "middle" }}>
                Unavailable
              </span>
            </Show>
            <Show when={props.sectionItem.isNew}>
              <span class="tag is-info is-light is-small ml-2" style={{ "vertical-align": "middle" }}>
                New
              </span>
            </Show>
            <Show when={item().description}>
              <p
                class="has-text-grey is-size-7"
                style={{
                  overflow: "hidden",
                  "text-overflow": "ellipsis",
                  "white-space": "nowrap",
                }}
              >
                {item().description}
              </p>
            </Show>
          </div>
        </div>

        {/* Price + toggle */}
        <div class="is-flex is-align-items-center ml-3" style={{ "flex-shrink": "0" }}>
          <span class="has-text-weight-bold is-size-6 mr-3" style={{ "white-space": "nowrap" }}>
            ${formatPrice(displayPrice())}
            <Show when={props.sectionItem.price_override_cents != null}>
              <span
                class="has-text-grey is-size-7 ml-1"
                style={{ "text-decoration": "line-through" }}
              >
                ${formatPrice(item().base_price_cents)}
              </span>
            </Show>
          </span>

          <button
            class="button is-small is-light"
            title={expanded() ? "Collapse" : "Expand"}
            onClick={() => setExpanded(!expanded())}
          >
            {expanded() ? "▲" : "▼"}
          </button>
        </div>
      </div>

      {/* ── Expanded edit form ─────────────────────────────── */}
      <Show when={expanded()}>
        <hr class="my-3" />
        <div class="columns is-multiline">
          {/* Item name */}
          <div class="column is-6">
            <div class="field">
              <label class="label is-small">Name</label>
              <div class="control">
                <input
                  class="input is-small"
                  type="text"
                  placeholder="Item name"
                  value={item().name}
                  onInput={(e) => handleNameChange(e.currentTarget.value)}
                />
              </div>
            </div>
          </div>

          {/* Description */}
          <div class="column is-6">
            <div class="field">
              <label class="label is-small">Description</label>
              <div class="control">
                <input
                  class="input is-small"
                  type="text"
                  placeholder="Optional description"
                  value={item().description ?? ""}
                  onInput={(e) => handleDescriptionChange(e.currentTarget.value)}
                />
              </div>
            </div>
          </div>

          {/* Base price */}
          <div class="column is-4">
            <div class="field">
              <label class="label is-small">Base price ($)</label>
              <div class="control">
                <input
                  class="input is-small"
                  type="number"
                  step="0.01"
                  min="0"
                  placeholder="0.00"
                  value={formatPrice(item().base_price_cents)}
                  onChange={(e) => handleBasePriceChange(e.currentTarget.value)}
                />
              </div>
            </div>
          </div>

          {/* Price override */}
          <div class="column is-4">
            <div class="field">
              <label class="label is-small">
                Price override ($)
                <span class="has-text-grey-light has-text-weight-normal"> — optional</span>
              </label>
              <div class="control">
                <input
                  class="input is-small"
                  type="number"
                  step="0.01"
                  min="0"
                  placeholder="Leave blank for base price"
                  value={
                    props.sectionItem.price_override_cents != null
                      ? formatPrice(props.sectionItem.price_override_cents)
                      : ""
                  }
                  onChange={(e) => handlePriceOverrideChange(e.currentTarget.value)}
                />
              </div>
              <Show when={props.sectionItem.price_override_cents != null}>
                <p class="help has-text-info">
                  Overrides the base price for this menu only
                </p>
              </Show>
            </div>
          </div>

          {/* Image URL */}
          <div class="column is-4">
            <div class="field">
              <label class="label is-small">Image URL</label>
              <div class="control">
                <input
                  class="input is-small"
                  type="url"
                  placeholder="https://..."
                  value={item().image_url ?? ""}
                  onInput={(e) => handleImageUrlChange(e.currentTarget.value)}
                />
              </div>
            </div>
          </div>

          {/* Availability toggle + Remove */}
          <div class="column is-12">
            <div class="is-flex is-justify-content-space-between is-align-items-center">
              <div class="field">
                <label class="checkbox is-size-7">
                  <input
                    type="checkbox"
                    checked={props.sectionItem.is_available}
                    onChange={handleAvailabilityToggle}
                  />{" "}
                  Available
                </label>
              </div>

              <button
                class={`button is-small ${confirmRemove() ? "is-danger" : "is-danger is-outlined"}`}
                onClick={handleRemove}
              >
                <span class="icon is-small">
                  <span>{confirmRemove() ? "⚠" : "🗑"}</span>
                </span>
                <span>{confirmRemove() ? "Confirm remove" : "Remove"}</span>
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}