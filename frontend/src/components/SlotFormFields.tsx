import { Show, For, Index } from "solid-js";
import type { SetStoreFunction } from "solid-js/store";
import type { SlotConstraintKind } from "@bindings/SlotConstraintKind";
import type { MenuSection } from "@bindings/MenuSection";
import {
  type DraftOffer,
  type DraftSlot,
  centsToDollars,
  dollarsToCents,
  constraintKindKey,
  constraintKindValue,
  emptyConstraint,
  flattenSectionsFromMenus,
} from "@/lib/offerDraft";

/** A menu-like object with sections, used for the constraint section picker. */
export interface MenuForPicker {
  id: string;
  name: string;
  sections: MenuSection[];
}

interface SlotFormFieldsProps {
  slot: DraftSlot;
  slotIndex: number;
  setDraft: SetStoreFunction<DraftOffer>;
  /** Menus to display in the section constraint picker. */
  menus: MenuForPicker[];
  /** Callback to add a constraint to this slot. */
  onAddConstraint: () => void;
  /** Callback to remove a constraint from this slot. */
  onRemoveConstraint: (constraintIndex: number) => void;
}

export default function SlotFormFields(props: SlotFormFieldsProps) {
  const slot = () => props.slot;
  const idx = () => props.slotIndex;

  return (
    <>
      {/* Slot fields */}
      <div class="columns is-multiline">
        <div class="column is-4">
          <div class="field">
            <label class="label is-small">Label *</label>
            <div class="control">
              <input
                class="input is-small"
                type="text"
                placeholder="e.g. Starter"
                value={slot().label}
                onInput={(e) =>
                  props.setDraft("slots", idx(), "label", e.currentTarget.value)
                }
              />
            </div>
          </div>
        </div>

        <div class="column is-2">
          <div class="field">
            <label class="label is-small">Min</label>
            <div class="control">
              <input
                class="input is-small"
                type="number"
                min="0"
                value={slot().minItems}
                onInput={(e) =>
                  props.setDraft(
                    "slots",
                    idx(),
                    "minItems",
                    parseInt(e.currentTarget.value) || 0,
                  )
                }
              />
            </div>
          </div>
        </div>

        <div class="column is-2">
          <div class="field">
            <label class="label is-small">Max</label>
            <div class="control">
              <input
                class="input is-small"
                type="number"
                min="0"
                value={slot().maxItems}
                onInput={(e) =>
                  props.setDraft(
                    "slots",
                    idx(),
                    "maxItems",
                    parseInt(e.currentTarget.value) || 0,
                  )
                }
              />
            </div>
          </div>
        </div>

        <div class="column is-4">
          <div class="field">
            <label class="label is-small">Group</label>
            <div class="control">
              <input
                class="input is-small"
                type="text"
                placeholder="e.g. main_course"
                value={slot().slotGroup}
                onInput={(e) =>
                  props.setDraft(
                    "slots",
                    idx(),
                    "slotGroup",
                    e.currentTarget.value,
                  )
                }
              />
            </div>
            <p class="help">Slots sharing a group are treated as a unit</p>
          </div>
        </div>

        <div class="column is-4">
          <div class="field">
            <label class="label is-small">Slot Supplement (€)</label>
            <div class="control">
              <input
                class="input is-small"
                type="text"
                inputmode="decimal"
                placeholder="0.00"
                value={slot().supplementDisplay}
                onInput={(e) =>
                  props.setDraft(
                    "slots",
                    idx(),
                    "supplementDisplay",
                    e.currentTarget.value,
                  )
                }
                onBlur={(e) => {
                  const cents = dollarsToCents(e.currentTarget.value);
                  props.setDraft(
                    "slots",
                    idx(),
                    "supplementDisplay",
                    centsToDollars(cents),
                  );
                }}
              />
            </div>
            <p class="help">
              {dollarsToCents(slot().supplementDisplay) === 0
                ? "Included in base"
                : `+€${centsToDollars(dollarsToCents(slot().supplementDisplay))}`}
            </p>
          </div>
        </div>
      </div>

      {/* ── Constraints ──────────────────────────────── */}
      <div class="mt-2">
        <div class="is-flex is-justify-content-space-between is-align-items-center mb-1">
          <span class="is-size-7 has-text-weight-semibold has-text-grey-dark">
            Constraints ({slot().constraints.length})
          </span>
          <button class="button is-small" onClick={props.onAddConstraint}>
            <span class="icon is-small" style={{ "font-size": "0.7rem" }}>
              <span>➕</span>
            </span>
            <span class="is-size-7">Add</span>
          </button>
        </div>

        <Show when={slot().constraints.length === 0}>
          <p class="has-text-grey is-size-7 is-italic ml-2">
            Add constraints to define which items are allowed.
          </p>
        </Show>

        <Index each={slot().constraints}>
          {(constraint, cIndex) => {
            const kindKey = () => constraintKindKey(constraint().kind);
            const kindValue = () => constraintKindValue(constraint().kind);

            return (
              <div
                class="is-flex is-align-items-center mb-2"
                style={{ gap: "0.5rem" }}
              >
                {/* Type selector */}
                <div class="select is-small">
                  <select
                    value={kindKey()}
                    onChange={(e) => {
                      const newKind = e.currentTarget.value as
                        | "Item"
                        | "Tag"
                        | "Section";
                      let newConstraintKind: SlotConstraintKind;
                      if (newKind === "Item") newConstraintKind = { Item: "" };
                      else if (newKind === "Tag")
                        newConstraintKind = { Tag: "" };
                      else newConstraintKind = { Section: "" };

                      props.setDraft(
                        "slots",
                        idx(),
                        "constraints",
                        cIndex,
                        "kind",
                        newConstraintKind,
                      );
                    }}
                  >
                    <option value="Section">Section</option>
                    <option value="Tag">Tag</option>
                    <option value="Item">Item</option>
                  </select>
                </div>

                {/* Section picker (with menu optgroups) */}
                <Show when={kindKey() === "Section"}>
                  <div class="select is-small" style={{ flex: "1" }}>
                    <select
                      value={kindValue()}
                      onChange={(e) => {
                        props.setDraft(
                          "slots",
                          idx(),
                          "constraints",
                          cIndex,
                          "kind",
                          { Section: e.currentTarget.value },
                        );
                      }}
                    >
                      <option value="">— Select section —</option>
                      <For each={props.menus}>
                        {(menu) => (
                          <optgroup label={menu.name}>
                            <For
                              each={flattenSectionsFromMenus([menu])}
                            >
                              {(section) => (
                                <option value={section.id}>
                                  {"  ".repeat(section.depth)}
                                  {section.name}
                                </option>
                              )}
                            </For>
                          </optgroup>
                        )}
                      </For>
                    </select>
                  </div>
                </Show>

                {/* Tag / Item: UUID input */}
                <Show when={kindKey() === "Tag" || kindKey() === "Item"}>
                  <div class="control is-expanded" style={{ flex: "1" }}>
                    <input
                      class="input is-small"
                      type="text"
                      placeholder={`${kindKey()} UUID`}
                      value={kindValue()}
                      onInput={(e) => {
                        const k = kindKey();
                        let newKind: SlotConstraintKind;
                        if (k === "Item")
                          newKind = { Item: e.currentTarget.value };
                        else if (k === "Tag")
                          newKind = { Tag: e.currentTarget.value };
                        else
                          newKind = { Section: e.currentTarget.value };

                        props.setDraft(
                          "slots",
                          idx(),
                          "constraints",
                          cIndex,
                          "kind",
                          newKind,
                        );
                      }}
                    />
                  </div>
                </Show>

                {/* Supplement (€) */}
                <div class="control" style={{ width: "80px" }}>
                  <input
                    class="input is-small"
                    type="text"
                    inputmode="decimal"
                    placeholder="0.00"
                    title="Supplement (€)"
                    value={constraint().supplementDisplay}
                    onInput={(e) =>
                      props.setDraft(
                        "slots",
                        idx(),
                        "constraints",
                        cIndex,
                        "supplementDisplay",
                        e.currentTarget.value,
                      )
                    }
                    onBlur={(e) => {
                      const cents = dollarsToCents(e.currentTarget.value);
                      props.setDraft(
                        "slots",
                        idx(),
                        "constraints",
                        cIndex,
                        "supplementDisplay",
                        centsToDollars(cents),
                      );
                    }}
                  />
                </div>
                <span
                  class="is-size-7 has-text-grey"
                  style={{ "white-space": "nowrap" }}
                >
                  {dollarsToCents(constraint().supplementDisplay) === 0
                    ? "incl."
                    : `+€${centsToDollars(dollarsToCents(constraint().supplementDisplay))}`}
                </span>

                {/* Remove */}
                <button
                  class="delete is-small"
                  title="Remove constraint"
                  onClick={() => props.onRemoveConstraint(cIndex)}
                />
              </div>
            );
          }}
        </Index>
      </div>
    </>
  );
}
