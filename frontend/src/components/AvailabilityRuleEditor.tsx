import { Show, For, createSignal, createEffect, batch } from "solid-js";
import type { AvailabilityRule } from "@bindings/AvailabilityRule";
import type { CreateAvailabilityRule } from "@bindings/CreateAvailabilityRule";
import type { UpdateAvailabilityRule } from "@bindings/UpdateAvailabilityRule";
import type { AssignAvailabilityRequest } from "@bindings/AssignAvailabilityRequest";
import type { ApiResponse } from "@bindings/ApiResponse";
import { showConfirm } from "@/stores/confirmStore";

// ── Weekday helpers ──────────────────────────────────────────────
const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

// ── Time formatting ──────────────────────────────────────────────

/** "HH:MM:SS" → "HH:MM" for <input type="time"> */
function formatTimeForInput(timeStr: string | null): string {
  if (!timeStr) return "";
  const parts = timeStr.split(":");
  if (parts.length >= 2) return `${parts[0].padStart(2, "0")}:${parts[1].padStart(2, "0")}`;
  return timeStr;
}

/** "HH:MM" → "HH:MM:SS" for the backend */
function formatTimeForBackend(timeStr: string): string | null {
  if (!timeStr) return null;
  const parts = timeStr.split(":");
  if (parts.length === 2) return `${parts[0]}:${parts[1]}:00`;
  return timeStr;
}

// ── API helpers ─────────────────────────────────────────────────

async function createRule(rule: CreateAvailabilityRule): Promise<AvailabilityRule | null> {
  const res = await fetch("/api/availability-rules", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(rule),
  });
  const json: ApiResponse<AvailabilityRule> = await res.json();
  if (res.ok && json.success && json.data) return json.data;
  throw new Error(json.error ?? `Create failed (${res.status})`);
}

async function updateRule(rule: UpdateAvailabilityRule): Promise<AvailabilityRule | null> {
  const res = await fetch("/api/update-availability-rule", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(rule),
  });
  const json: ApiResponse<AvailabilityRule> = await res.json();
  if (res.ok && json.success && json.data) return json.data;
  throw new Error(json.error ?? `Update failed (${res.status})`);
}

async function deleteRule(ruleId: string): Promise<void> {
  const res = await fetch(`/api/availability-rules/${ruleId}`, { method: "DELETE" });
  const json: ApiResponse<unknown> = await res.json();
  if (!res.ok || !json.success) throw new Error(json.error ?? `Delete failed (${res.status})`);
}

async function assignRule(
  entityType: string,
  entityId: string,
  ruleId: string | null,
): Promise<void> {
  const body: AssignAvailabilityRequest = { availability_rule_id: ruleId };
  const res = await fetch(`/api/${entityType}s/${entityId}/availability-rule`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const json: ApiResponse<unknown> = await res.json();
  if (!res.ok || !json.success) throw new Error(json.error ?? `Assign failed (${res.status})`);
}

// ── Component ────────────────────────────────────────────────────

interface AvailabilityRuleEditorProps {
  /** The current availability rule, if any. */
  rule: AvailabilityRule | null;
  /** Entity type for the assignment endpoint. */
  entityType: "restaurant" | "menu" | "item" | "offer";
  /** Entity ID for the assignment endpoint. */
  entityId: string;
  /** Called after a successful save/remove so the parent can refresh. */
  onChanged?: (rule: AvailabilityRule | null) => void;
}

export default function AvailabilityRuleEditor(props: AvailabilityRuleEditorProps) {
  // ── State ──────────────────────────────────────────────────────
  const [editing, setEditing] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [success, setSuccess] = createSignal<string | null>(null);

  // Form fields
  const [active, setActive] = createSignal(true);
  const [validFrom, setValidFrom] = createSignal("");
  const [validTo, setValidTo] = createSignal("");
  const [startTime, setStartTime] = createSignal("");
  const [endTime, setEndTime] = createSignal("");
  const [weekdays, setWeekdays] = createSignal<number[]>([]);

  // ── Auto-dismiss success ───────────────────────────────────────
  createEffect(() => {
    if (success()) {
      const timer = setTimeout(() => setSuccess(null), 4000);
      return () => clearTimeout(timer);
    }
  });

  // ── Populate form from existing rule ───────────────────────────
  const populateForm = (rule: AvailabilityRule | null) => {
    if (rule) {
      setActive(rule.active);
      setValidFrom(rule.valid_from ?? "");
      setValidTo(rule.valid_to ?? "");
      setStartTime(formatTimeForInput(rule.start_time));
      setEndTime(formatTimeForInput(rule.end_time));
      setWeekdays(rule.weekdays ?? []);
    } else {
      setActive(true);
      setValidFrom("");
      setValidTo("");
      setStartTime("");
      setEndTime("");
      setWeekdays([]);
    }
  };

  const startEditing = () => {
    populateForm(props.rule);
    setEditing(true);
    setError(null);
    setSuccess(null);
  };

  const cancelEditing = () => {
    setEditing(false);
    setError(null);
  };

  // ── Toggle weekday ─────────────────────────────────────────────
  const toggleWeekday = (day: number) => {
    setWeekdays((prev) =>
      prev.includes(day) ? prev.filter((d) => d !== day) : [...prev, day].sort(),
    );
  };

  // ── Save (create or update) ────────────────────────────────────
  const handleSave = async () => {
    setError(null);
    setSuccess(null);
    setSaving(true);

    try {
      const ruleData = {
        valid_from: validFrom() || null,
        valid_to: validTo() || null,
        start_time: formatTimeForBackend(startTime()),
        end_time: formatTimeForBackend(endTime()),
        weekdays: weekdays().length > 0 ? weekdays() : null,
        active: active(),
      };

      let savedRule: AvailabilityRule | null;

      if (props.rule) {
        // Update existing rule
        savedRule = await updateRule({
          id: props.rule.id,
          ...ruleData,
        });
      } else {
        // Create new rule and assign
        savedRule = await createRule(ruleData);
        if (savedRule) {
          await assignRule(props.entityType, props.entityId, savedRule.id);
        }
      }

      setEditing(false);
      setSuccess("Availability rule saved!");
      props.onChanged?.(savedRule);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  // ── Remove rule ────────────────────────────────────────────────
  const handleRemove = async () => {
    if (!props.rule) return;

    const confirmed = await showConfirm({
      title: "Remove availability rule?",
      message:
        "This will remove the availability restriction from this " +
        props.entityType +
        ". The rule itself will be deleted.",
      confirmText: "Remove",
      cancelText: "Cancel",
      danger: true,
    });
    if (!confirmed) return;

    setSaving(true);
    setError(null);

    try {
      // Unassign first, then delete the rule
      await assignRule(props.entityType, props.entityId, null);
      await deleteRule(props.rule.id);

      setEditing(false);
      setSuccess("Availability rule removed.");
      props.onChanged?.(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  // ── Render: summary of existing rule ───────────────────────────
  const renderRuleSummary = (rule: AvailabilityRule) => {
    const parts: string[] = [];

    if (!rule.active) {
      parts.push("Disabled (always available)");
    } else {
      if (rule.valid_from || rule.valid_to) {
        parts.push(`${rule.valid_from ?? "…"} → ${rule.valid_to ?? "…"}`);
      }
      if (rule.start_time || rule.end_time) {
        parts.push(
          `${formatTimeForInput(rule.start_time) || "…"} – ${formatTimeForInput(rule.end_time) || "…"}`,
        );
      }
      if (rule.weekdays && rule.weekdays.length > 0 && rule.weekdays.length < 7) {
        parts.push(rule.weekdays.map((d) => WEEKDAY_LABELS[d]).join(", "));
      }
      if (parts.length === 0) parts.push("Active (no constraints)");
    }

    return parts.join(" · ");
  };

  return (
    <div>
      {/* ── Error ─────────────────────────────────────────── */}
      <Show when={error()}>
        <div class="notification is-danger is-light py-2 px-3 mb-2" style={{ "font-size": "0.85rem" }}>
          <button class="delete is-small" onClick={() => setError(null)} />
          {error()}
        </div>
      </Show>

      {/* ── Success ───────────────────────────────────────── */}
      <Show when={success()}>
        <div class="notification is-success is-light py-2 px-3 mb-2" style={{ "font-size": "0.85rem" }}>
          <button class="delete is-small" onClick={() => setSuccess(null)} />
          {success()}
        </div>
      </Show>

      {/* ── Display mode (no rule) ────────────────────────── */}
      <Show when={!editing() && !props.rule}>
        <div
          class="is-flex is-align-items-center is-justify-content-space-between"
          style={{ gap: "0.5rem" }}
        >
          <span class="has-text-grey is-size-7">
            No availability rule — always available
          </span>
          <button
            class="button is-small is-primary is-outlined"
            onClick={startEditing}
          >
            <span class="icon is-small"><span>🕐</span></span>
            <span>Add Rule</span>
          </button>
        </div>
      </Show>

      {/* ── Display mode (has rule) ───────────────────────── */}
      <Show when={!editing() && props.rule}>
        {(rule) => (
          <div
            class="is-flex is-align-items-center is-justify-content-space-between is-flex-wrap-wrap"
            style={{ gap: "0.5rem" }}
          >
            <div>
              <span
                class={`tag is-small mr-2 ${rule().active ? "is-success" : "is-warning"}`}
              >
                {rule().active ? "🟢 Active" : "⏸ Disabled"}
              </span>
              <span class="is-size-7">{renderRuleSummary(rule())}</span>
            </div>
            <div class="buttons are-small">
              <button class="button is-small is-info is-outlined" onClick={startEditing}>
                <span class="icon is-small"><span>✏️</span></span>
                <span>Edit</span>
              </button>
              <button
                class="button is-small is-danger is-outlined"
                classList={{ "is-loading": saving() }}
                disabled={saving()}
                onClick={handleRemove}
              >
                <span class="icon is-small"><span>🗑️</span></span>
              </button>
            </div>
          </div>
        )}
      </Show>

      {/* ── Edit mode ─────────────────────────────────────── */}
      <Show when={editing()}>
        <div
          class="box p-3 mt-2"
          style={{
            "border-left": "3px solid hsl(204, 71%, 53%)",
            "background-color": "hsl(204, 71%, 98%)",
          }}
        >
          <div class="is-flex is-justify-content-space-between is-align-items-center mb-3">
            <span class="has-text-weight-semibold is-size-6">
              {props.rule ? "Edit Availability Rule" : "New Availability Rule"}
            </span>
            <button class="delete is-small" onClick={cancelEditing} />
          </div>

          <div class="columns is-multiline">
            {/* Active toggle */}
            <div class="column is-12">
              <div class="field">
                <label class="checkbox">
                  <input
                    type="checkbox"
                    checked={active()}
                    onChange={(e) => setActive(e.currentTarget.checked)}
                    disabled={saving()}
                  />{" "}
                  <strong>Rule active</strong>
                </label>
                <p class="help">
                  When disabled, this {props.entityType} is treated as always available.
                </p>
              </div>
            </div>

            {/* Date range */}
            <div class="column is-6">
              <div class="field">
                <label class="label is-small">Valid from</label>
                <div class="control">
                  <input
                    class="input is-small"
                    type="date"
                    value={validFrom()}
                    onInput={(e) => setValidFrom(e.currentTarget.value)}
                    disabled={saving() || !active()}
                  />
                </div>
                <p class="help">Leave blank for no start date</p>
              </div>
            </div>

            <div class="column is-6">
              <div class="field">
                <label class="label is-small">Valid to</label>
                <div class="control">
                  <input
                    class="input is-small"
                    type="date"
                    value={validTo()}
                    onInput={(e) => setValidTo(e.currentTarget.value)}
                    disabled={saving() || !active()}
                  />
                </div>
                <p class="help">Leave blank for no end date</p>
              </div>
            </div>

            {/* Time range */}
            <div class="column is-6">
              <div class="field">
                <label class="label is-small">Start time</label>
                <div class="control">
                  <input
                    class="input is-small"
                    type="time"
                    value={startTime()}
                    onInput={(e) => setStartTime(e.currentTarget.value)}
                    disabled={saving() || !active()}
                  />
                </div>
                <p class="help">Overnight ranges supported (e.g. 22:00–06:00)</p>
              </div>
            </div>

            <div class="column is-6">
              <div class="field">
                <label class="label is-small">End time</label>
                <div class="control">
                  <input
                    class="input is-small"
                    type="time"
                    value={endTime()}
                    onInput={(e) => setEndTime(e.currentTarget.value)}
                    disabled={saving() || !active()}
                  />
                </div>
                <p class="help">Leave blank for no end time constraint</p>
              </div>
            </div>

            {/* Weekdays */}
            <div class="column is-12">
              <div class="field">
                <label class="label is-small">Weekdays</label>
                <div class="buttons are-small">
                  <For each={WEEKDAY_LABELS}>
                    {(label, idx) => (
                      <button
                        class={`button is-small ${weekdays().includes(idx()) ? "is-primary" : "is-light"}`}
                        disabled={saving() || !active()}
                        onClick={() => toggleWeekday(idx())}
                        type="button"
                      >
                        {label}
                      </button>
                    )}
                  </For>
                </div>
                <p class="help">
                  {weekdays().length === 0
                    ? "No filter — available every day"
                    : `Available on: ${weekdays().map((d) => WEEKDAY_LABELS[d]).join(", ")}`}
                </p>
              </div>
            </div>
          </div>

          {/* Actions */}
          <hr class="my-2" />
          <div class="buttons are-small">
            <button
              class="button is-primary is-small"
              classList={{ "is-loading": saving() }}
              disabled={saving()}
              onClick={handleSave}
            >
              <span class="icon is-small"><span>💾</span></span>
              <span>{props.rule ? "Save Rule" : "Create Rule"}</span>
            </button>
            <button
              class="button is-light is-small"
              disabled={saving()}
              onClick={cancelEditing}
            >
              Cancel
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
}