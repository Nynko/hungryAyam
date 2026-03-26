import { Show, createSignal, createEffect, onMount } from "solid-js";
import type { RestaurantOrderSettings } from "@bindings/RestaurantOrderSettings";
import type { UpdateOrderSettingsRequest } from "@bindings/UpdateOrderSettingsRequest";
import type { UpdateRestaurant } from "@bindings/UpdateRestaurant";
import type { AvailabilityRule } from "@bindings/AvailabilityRule";
import type { Restaurant } from "@bindings/Restaurant";
import {
  fetchOrderSettings,
  updateOrderSettings,
  orderLoading,
  orderError,
  clearOrderError,
} from "@/stores/orderStore";
import { updateRestaurant as updateRestaurantApi, restaurantsError, clearRestaurantsError } from "@/stores/restaurantStore";
import AvailabilityRuleEditor from "@/components/AvailabilityRuleEditor";

interface RestaurantSettingsPanelProps {
  restaurantId: string;
  restaurant?: Restaurant | null;
}

/**
 * Format a NaiveTime string from the backend (e.g. "08:00:00") to an
 * HTML time input value (e.g. "08:00").
 */
function formatTimeForInput(timeStr: string | null | undefined): string {
  if (!timeStr) return "";
  // Backend sends "HH:MM:SS" or "HH:MM" — HTML input wants "HH:MM"
  const parts = timeStr.split(":");
  if (parts.length >= 2) {
    return `${parts[0].padStart(2, "0")}:${parts[1].padStart(2, "0")}`;
  }
  return timeStr;
}

/**
 * Format a time input value (e.g. "08:00") to a NaiveTime string for the
 * backend (e.g. "08:00:00").
 */
function formatTimeForBackend(timeStr: string): string {
  if (!timeStr) return "";
  const parts = timeStr.split(":");
  if (parts.length === 2) {
    return `${parts[0]}:${parts[1]}:00`;
  }
  return timeStr;
}

export default function RestaurantSettingsPanel(props: RestaurantSettingsPanelProps) {
  const [settings, setSettings] = createSignal<RestaurantOrderSettings | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [success, setSuccess] = createSignal<string | null>(null);
  const [expanded, setExpanded] = createSignal(false);

  // ── Form state ──────────────────────────────────────────────
  const [defaultStartTime, setDefaultStartTime] = createSignal("");
  const [defaultEndTime, setDefaultEndTime] = createSignal("");
  const [autoCreateSession, setAutoCreateSession] = createSignal(true);
  const [autoCloseSession, setAutoCloseSession] = createSignal(true);
  const [menuResetEnabled, setMenuResetEnabled] = createSignal(false);
  const [menuResetTime, setMenuResetTime] = createSignal("");
  const [timezone, setTimezone] = createSignal("");

  // ── Restaurant info form state ──────────────────────────────
  const [restaurantName, setRestaurantName] = createSignal("");
  const [restaurantImageUrl, setRestaurantImageUrl] = createSignal("");
  const [restaurantSaving, setRestaurantSaving] = createSignal(false);
  const [restaurantSuccess, setRestaurantSuccess] = createSignal<string | null>(null);

  // ── Load settings ───────────────────────────────────────────
  const loadSettings = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await fetchOrderSettings(props.restaurantId);
      if (result) {
        setSettings(result);
        populateForm(result);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const populateForm = (s: RestaurantOrderSettings) => {
    setDefaultStartTime(formatTimeForInput(s.default_start_time));
    setDefaultEndTime(formatTimeForInput(s.default_end_time));
    setAutoCreateSession(s.auto_create_session);
    setAutoCloseSession(s.auto_close_session);
    setMenuResetEnabled(s.menu_reset_time != null);
    setMenuResetTime(formatTimeForInput(s.menu_reset_time));
    setTimezone(s.timezone);
  };

  onMount(() => {
    loadSettings();
  });

  // ── Populate restaurant fields ──────────────────────────────
  createEffect(() => {
    const r = props.restaurant;
    if (r) {
      setRestaurantName(r.name);
      setRestaurantImageUrl(r.image_url ?? "");
    }
  });

  // Auto-dismiss success message
  createEffect(() => {
    if (success()) {
      const timer = setTimeout(() => setSuccess(null), 4000);
      return () => clearTimeout(timer);
    }
  });

  createEffect(() => {
    if (restaurantSuccess()) {
      const timer = setTimeout(() => setRestaurantSuccess(null), 4000);
      return () => clearTimeout(timer);
    }
  });

  // ── Save restaurant info ────────────────────────────────────
  const handleSaveRestaurant = async () => {
    const r = props.restaurant;
    if (!r) return;

    const trimmedName = restaurantName().trim();
    if (!trimmedName) {
      setError("Restaurant name is required.");
      return;
    }

    setRestaurantSaving(true);
    setError(null);
    clearRestaurantsError();

    const request: UpdateRestaurant = {
      id: r.id,
      name: trimmedName,
      image_url: restaurantImageUrl().trim() || null,
    };

    const result = await updateRestaurantApi(request);
    setRestaurantSaving(false);

    if (result) {
      setRestaurantSuccess("Restaurant info saved!");
    } else {
      setError(restaurantsError() || "Failed to save restaurant info.");
    }
  };

  // ── Save ────────────────────────────────────────────────────
  const handleSave = async () => {
    const s = settings();
    if (!s) return;

    setError(null);
    setSuccess(null);
    clearOrderError();

    // Validation
    const start = defaultStartTime().trim();
    const end = defaultEndTime().trim();
    if (!start || !end) {
      setError("Default start and end times are required.");
      return;
    }
    if (start >= end) {
      setError("Default end time must be after start time.");
      return;
    }
    if (menuResetEnabled() && !menuResetTime().trim()) {
      setError("Please set a reset time or disable menu auto-reset.");
      return;
    }
    if (!timezone().trim()) {
      setError("Timezone is required.");
      return;
    }

    setSaving(true);

    const request: UpdateOrderSettingsRequest = {
      id: s.id,
      default_start_time: formatTimeForBackend(start),
      default_end_time: formatTimeForBackend(end),
      sending_method: null, // don't change
      timezone: timezone().trim(),
      auto_create_session: autoCreateSession(),
      menu_reset_time: menuResetEnabled() ? formatTimeForBackend(menuResetTime()) : null,
      update_menu_reset_time: true, // always send the menu_reset_time value
      auto_close_session: autoCloseSession(),
    };

    const result = await updateOrderSettings(request);
    setSaving(false);

    if (result) {
      setSettings(result);
      populateForm(result);
      setSuccess("Settings saved!");
    } else {
      setError(orderError() || "Failed to save settings.");
    }
  };

  // ── Common timezones for quick selection ────────────────────
  const commonTimezones = [
    "Asia/Jakarta",
    "Asia/Singapore",
    "Asia/Tokyo",
    "Asia/Shanghai",
    "Asia/Kolkata",
    "Europe/Paris",
    "Europe/London",
    "Europe/Berlin",
    "America/New_York",
    "America/Chicago",
    "America/Los_Angeles",
    "Australia/Sydney",
    "Pacific/Auckland",
  ];

  // ── Availability ────────────────────────────────────────────
  const handleAvailabilityChanged = (_rule: AvailabilityRule | null) => {
    // The parent can refetch if needed. For now the AvailabilityRuleEditor
    // manages its own display state.
  };

  const displayError = () => error() || (expanded() ? orderError() : null);

  return (
    <div class="box mb-5">
      {/* ── Header (collapsible) ─────────────────────────────── */}
      <div
        class="is-flex is-justify-content-space-between is-align-items-center is-clickable"
        onClick={() => {
          if (!expanded()) loadSettings(); // refresh on expand
          setExpanded(!expanded());
        }}
      >
        <div>
          <h3 class="title is-5 mb-0">
            <span class="mr-2">⚙️</span>
            Restaurant Settings
          </h3>
          <p class="has-text-grey is-size-7 mt-1">
            Configure restaurant info, availability, ordering, and scheduling
          </p>
        </div>
        <button class="button is-small is-light">
          {expanded() ? "▲ Collapse" : "▼ Expand"}
        </button>
      </div>

      <Show when={expanded()}>
        <hr class="my-4" />

        {/* ── Loading ──────────────────────────────────────── */}
        <Show when={loading()}>
          <div class="has-text-centered py-4">
            <progress class="progress is-primary is-small" max="100" />
            <p class="has-text-grey is-size-7 mt-1">Loading settings…</p>
          </div>
        </Show>

        {/* ── Error ───────────────────────────────────────── */}
        <Show when={displayError()}>
          <div class="notification is-danger is-light mb-4">
            <button
              class="delete"
              onClick={() => {
                setError(null);
                clearOrderError();
              }}
            />
            {displayError()}
          </div>
        </Show>

        {/* ── Success ─────────────────────────────────────── */}
        <Show when={success()}>
          <div class="notification is-success is-light mb-4">
            <button class="delete" onClick={() => setSuccess(null)} />
            {success()}
          </div>
        </Show>

        <Show when={settings() && !loading()}>
          {/* ── Restaurant Info ─────────────────────────────── */}
          <div class="mb-4">
            <h4 class="title is-6 mb-2">
              <span class="mr-1">🏪</span> Restaurant Info
            </h4>

            <Show when={restaurantSuccess()}>
              <div class="notification is-success is-light py-2 px-3 mb-3" style={{ "font-size": "0.85rem" }}>
                <button class="delete is-small" onClick={() => setRestaurantSuccess(null)} />
                {restaurantSuccess()}
              </div>
            </Show>

            <div class="columns is-multiline">
              <div class="column is-6">
                <div class="field">
                  <label class="label">Name</label>
                  <div class="control">
                    <input
                      class="input"
                      type="text"
                      placeholder="Restaurant name"
                      value={restaurantName()}
                      onInput={(e) => setRestaurantName(e.currentTarget.value)}
                      disabled={restaurantSaving()}
                    />
                  </div>
                </div>
              </div>

              <div class="column is-6">
                <div class="field">
                  <label class="label">Image URL</label>
                  <div class="control">
                    <input
                      class="input"
                      type="url"
                      placeholder="https://example.com/logo.png (optional)"
                      value={restaurantImageUrl()}
                      onInput={(e) => setRestaurantImageUrl(e.currentTarget.value)}
                      disabled={restaurantSaving()}
                    />
                  </div>
                </div>
              </div>

              <Show when={restaurantImageUrl().trim()}>
                <div class="column is-12">
                  <div class="field">
                    <label class="label is-small">Preview</label>
                    <figure
                      class="image is-3by2"
                      style={{
                        "max-width": "200px",
                        "background-color": "var(--bulma-scheme-main-bis)",
                        overflow: "hidden",
                        "border-radius": "4px",
                      }}
                    >
                      <img
                        src={restaurantImageUrl().trim()}
                        alt="Preview"
                        style={{
                          "object-fit": "cover",
                          width: "100%",
                          height: "100%",
                        }}
                        onError={(e) => {
                          (e.currentTarget as HTMLImageElement).style.display = "none";
                        }}
                      />
                    </figure>
                  </div>
                </div>
              </Show>

              <div class="column is-12">
                <div class="is-flex is-justify-content-flex-end">
                  <button
                    class="button is-primary is-small"
                    classList={{ "is-loading": restaurantSaving() }}
                    disabled={restaurantSaving() || !restaurantName().trim()}
                    onClick={handleSaveRestaurant}
                  >
                    <span class="icon is-small"><span>💾</span></span>
                    <span>Save Info</span>
                  </button>
                </div>
              </div>
            </div>

            <hr class="my-3" />
          </div>

          <div class="columns is-multiline">
            {/* ── Default session start time ─────────────── */}
            <div class="column is-6">
              <div class="field">
                <label class="label">Default Session Start Time</label>
                <div class="control">
                  <input
                    class="input"
                    type="time"
                    value={defaultStartTime()}
                    onInput={(e) => setDefaultStartTime(e.currentTarget.value)}
                    disabled={saving()}
                  />
                </div>
                <p class="help">
                  Default start time for new order sessions.
                </p>
              </div>
            </div>

            {/* ── Default session end time ───────────────── */}
            <div class="column is-6">
              <div class="field">
                <label class="label">Default Session End Time</label>
                <div class="control">
                  <input
                    class="input"
                    type="time"
                    value={defaultEndTime()}
                    onInput={(e) => setDefaultEndTime(e.currentTarget.value)}
                    disabled={saving()}
                  />
                </div>
                <p class="help">
                  Default end time (deadline) for new order sessions.
                </p>
              </div>
            </div>

            {/* ── Timezone ──────────────────────────────────── */}
            <div class="column is-6">
              <div class="field">
                <label class="label">Timezone</label>
                <div class="control">
                  <div class="select is-fullwidth">
                    <select
                      value={timezone()}
                      onChange={(e) => setTimezone(e.currentTarget.value)}
                      disabled={saving()}
                    >
                      <option value="" disabled>
                        Select timezone…
                      </option>
                      {/* If current timezone not in the common list, show it first */}
                      <Show
                        when={
                          timezone() &&
                          !commonTimezones.includes(timezone())
                        }
                      >
                        <option value={timezone()}>{timezone()}</option>
                      </Show>
                      {commonTimezones.map((tz) => (
                        <option value={tz}>{tz}</option>
                      ))}
                    </select>
                  </div>
                </div>
                <p class="help">
                  Used for interpreting reset times and session scheduling.
                </p>
              </div>
            </div>

            {/* ── Auto-create session toggle ─────────────── */}
            <div class="column is-6">
              <div class="field mt-4">
                <label class="checkbox">
                  <input
                    type="checkbox"
                    checked={autoCreateSession()}
                    onChange={(e) =>
                      setAutoCreateSession(e.currentTarget.checked)
                    }
                    disabled={saving()}
                  />{" "}
                  <strong>Auto-create session</strong>
                </label>
                <p class="help">
                  Automatically create an order session when someone places
                  the first order and no active session exists.
                </p>
              </div>
            </div>

            {/* ── Separator ─────────────────────────────────── */}
            <div class="column is-12">
              <hr class="my-2" />
              <h4 class="title is-6 mb-2">
                <span class="mr-1">⏰</span> Scheduling
              </h4>
            </div>

            {/* ── Auto-close session toggle ──────────────── */}
            <div class="column is-6">
              <div class="field">
                <label class="checkbox">
                  <input
                    type="checkbox"
                    checked={autoCloseSession()}
                    onChange={(e) =>
                      setAutoCloseSession(e.currentTarget.checked)
                    }
                    disabled={saving()}
                  />{" "}
                  <strong>Auto-close sessions</strong>
                </label>
                <p class="help">
                  Automatically close order sessions when their end time
                  passes (checked every minute). Sessions with "allow late"
                  enabled are excluded.
                </p>
              </div>
            </div>

            {/* ── Menu auto-reset ───────────────────────── */}
            <div class="column is-6">
              <div class="field">
                <label class="checkbox">
                  <input
                    type="checkbox"
                    checked={menuResetEnabled()}
                    onChange={(e) => {
                      setMenuResetEnabled(e.currentTarget.checked);
                      if (!e.currentTarget.checked) {
                        setMenuResetTime("");
                      } else if (!menuResetTime()) {
                        setMenuResetTime("06:00");
                      }
                    }}
                    disabled={saving()}
                  />{" "}
                  <strong>Auto-reset non-permanent menus</strong>
                </label>
                <p class="help">
                  At the configured time each day, all items in
                  non-permanent menus are set to "unavailable". You can then
                  easily re-select which items to offer today.
                </p>
              </div>

              <Show when={menuResetEnabled()}>
                <div class="field mt-3">
                  <label class="label is-small">Reset Time</label>
                  <div class="control">
                    <input
                      class="input"
                      type="time"
                      value={menuResetTime()}
                      onInput={(e) =>
                        setMenuResetTime(e.currentTarget.value)
                      }
                      disabled={saving()}
                    />
                  </div>
                  <p class="help">
                    Time of day (in the restaurant's timezone) when the
                    reset happens. Typical values: 00:00 (midnight) or
                    06:00 (early morning).
                  </p>
                </div>
              </Show>
            </div>

            {/* ── Current schedule summary ─────────────────── */}
            <div class="column is-12">
              <div
                class="notification is-info is-light py-3 px-4"
                style={{ "font-size": "0.85rem" }}
              >
                <p class="mb-1">
                  <strong>📋 Schedule Summary</strong>
                </p>
                <ul style={{ "list-style": "disc", "padding-left": "1.5rem" }}>
                  <li>
                    Default sessions:{" "}
                    <strong>
                      {defaultStartTime() || "—"} → {defaultEndTime() || "—"}
                    </strong>{" "}
                    ({timezone() || "no timezone"})
                  </li>
                  <li>
                    Auto-create session:{" "}
                    <strong>{autoCreateSession() ? "Yes" : "No"}</strong>
                  </li>
                  <li>
                    Auto-close sessions:{" "}
                    <strong>{autoCloseSession() ? "Yes" : "No"}</strong>
                  </li>
                  <li>
                    Menu auto-reset:{" "}
                    <strong>
                      {menuResetEnabled()
                        ? `Daily at ${menuResetTime() || "—"}`
                        : "Disabled"}
                    </strong>
                  </li>
                </ul>
              </div>
            </div>

            {/* ── Availability Rule ─────────────────────────── */}
            <div class="column is-12">
              <hr class="my-2" />
              <h4 class="title is-6 mb-2">
                <span class="mr-1">🕐</span> Availability
              </h4>
              <p class="has-text-grey is-size-7 mb-3">
                Control when this restaurant is available for ordering.
                When an availability rule is active, the restaurant will only
                be accessible during the specified times/days.
              </p>
              <AvailabilityRuleEditor
                rule={props.restaurant?.availability_rule ?? null}
                entityType="restaurant"
                entityId={props.restaurantId}
                onChanged={handleAvailabilityChanged}
              />
            </div>
          </div>

          {/* ── Save button ────────────────────────────────── */}
          <div class="is-flex is-justify-content-flex-end mt-3">
            <button
              class="button is-primary"
              classList={{ "is-loading": saving() }}
              disabled={saving()}
              onClick={handleSave}
            >
              <span class="icon is-small">
                <span>💾</span>
              </span>
              <span>Save Settings</span>
            </button>
          </div>
        </Show>
      </Show>
    </div>
  );
}