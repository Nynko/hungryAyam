import { createSignal, Show, onMount } from "solid-js";
import {
  createSession,
  fetchOrderSettings,
  sessionLoading,
  orderError,
  clearOrderError,
} from "@/stores/orderStore";
import type { CreateOrderSession } from "@bindings/CreateOrderSession";

interface CreateSessionModalProps {
  isOpen: boolean;
  restaurantId: string;
  onClose: () => void;
  /** Called after a session is successfully created. */
  onCreated?: () => void;
}

/**
 * Round a Date up to the nearest 5 minutes and format as `YYYY-MM-DDTHH:mm`
 * (the format required by `<input type="datetime-local">`).
 */
function toDatetimeLocal(date: Date): string {
  // Round up to next 5 minutes
  const ms = 5 * 60 * 1000;
  const rounded = new Date(Math.ceil(date.getTime() / ms) * ms);

  const y = rounded.getFullYear();
  const M = String(rounded.getMonth() + 1).padStart(2, "0");
  const d = String(rounded.getDate()).padStart(2, "0");
  const h = String(rounded.getHours()).padStart(2, "0");
  const m = String(rounded.getMinutes()).padStart(2, "0");
  return `${y}-${M}-${d}T${h}:${m}`;
}

/**
 * Given a time string like "11:30:00" or "11:30" and a base Date,
 * return a Date object on the same day with that time.
 * If the resulting time is in the past, bump to tomorrow.
 */
function timeStringToDate(timeStr: string, base: Date): Date {
  const parts = timeStr.split(":");
  const hours = parseInt(parts[0], 10);
  const minutes = parseInt(parts[1] ?? "0", 10);

  const result = new Date(base);
  result.setHours(hours, minutes, 0, 0);

  // If the resulting time is in the past, move to tomorrow
  if (result.getTime() < base.getTime()) {
    result.setDate(result.getDate() + 1);
  }

  return result;
}

export default function CreateSessionModal(props: CreateSessionModalProps) {
  const [startDate, setStartDate] = createSignal("");
  const [endDate, setEndDate] = createSignal("");
  const [allowLate, setAllowLate] = createSignal(false);
  const [validationError, setValidationError] = createSignal<string | null>(null);
  const [settingsLoaded, setSettingsLoaded] = createSignal(false);

  // Load default times from restaurant order settings when the modal opens
  const loadDefaults = async () => {
    const now = new Date();

    // Try to load restaurant order settings for smart defaults
    const settings = await fetchOrderSettings(props.restaurantId);

    if (settings) {
      setSettingsLoaded(true);
      const start = timeStringToDate(settings.default_start_time, now);
      const end = timeStringToDate(settings.default_end_time, start);

      // If end <= start, bump end to next day
      if (end.getTime() <= start.getTime()) {
        end.setDate(end.getDate() + 1);
      }

      setStartDate(toDatetimeLocal(start));
      setEndDate(toDatetimeLocal(end));
    } else {
      // Fallback: start now, end in 2 hours
      setStartDate(toDatetimeLocal(now));
      const twoHoursLater = new Date(now.getTime() + 2 * 60 * 60 * 1000);
      setEndDate(toDatetimeLocal(twoHoursLater));
    }
  };

  // Reset form state
  const resetForm = () => {
    setStartDate("");
    setEndDate("");
    setAllowLate(false);
    setValidationError(null);
    setSettingsLoaded(false);
    clearOrderError();
  };

  // When the modal opens, load defaults
  onMount(() => {
    // We watch the `isOpen` prop reactively in the component below,
    // but also load defaults initially if already open
    if (props.isOpen) {
      loadDefaults();
    }
  });

  // Watch for modal opening
  const handleOpen = () => {
    resetForm();
    loadDefaults();
  };

  const handleClose = () => {
    if (sessionLoading()) return;
    resetForm();
    props.onClose();
  };

  const handleSubmit = async (e: SubmitEvent) => {
    e.preventDefault();
    setValidationError(null);
    clearOrderError();

    const start = startDate().trim();
    const end = endDate().trim();

    if (!start || !end) {
      setValidationError("Both start and end dates are required.");
      return;
    }

    const startMs = new Date(start).getTime();
    const endMs = new Date(end).getTime();

    if (isNaN(startMs) || isNaN(endMs)) {
      setValidationError("Invalid date format.");
      return;
    }

    if (endMs <= startMs) {
      setValidationError("End date must be after the start date.");
      return;
    }

    const request: CreateOrderSession = {
      restaurant_id: props.restaurantId,
      start_date: new Date(start).toISOString(),
      end_date: new Date(end).toISOString(),
      allow_late: allowLate(),
    };

    const session = await createSession(request);

    if (session) {
      resetForm();
      props.onCreated?.();
      props.onClose();
    }
  };

  const displayError = () => validationError() || orderError();

  return (
    <div
      class="modal"
      classList={{ "is-active": props.isOpen }}
      ref={(el) => {
        // When the modal becomes active, load defaults
        const observer = new MutationObserver(() => {
          if (el.classList.contains("is-active")) {
            handleOpen();
          }
        });
        observer.observe(el, { attributes: true, attributeFilter: ["class"] });
      }}
    >
      <div class="modal-background" onClick={handleClose} />
      <div class="modal-card" style={{ "max-width": "520px" }}>
        <header class="modal-card-head">
          <p class="modal-card-title">📋 New Order Session</p>
          <button
            class="delete"
            aria-label="close"
            onClick={handleClose}
            disabled={sessionLoading()}
          />
        </header>

        <form onSubmit={handleSubmit}>
          <section class="modal-card-body">
            {/* Settings info */}
            <Show when={settingsLoaded()}>
              <div class="notification is-info is-light is-size-7 py-2 px-3 mb-4">
                Default times loaded from restaurant settings. Adjust as needed.
              </div>
            </Show>

            {/* Error */}
            <Show when={displayError()}>
              <div class="notification is-danger is-light">
                <button
                  class="delete"
                  type="button"
                  onClick={() => {
                    setValidationError(null);
                    clearOrderError();
                  }}
                />
                {displayError()}
              </div>
            </Show>

            {/* Start date */}
            <div class="field">
              <label class="label">Start Date & Time</label>
              <div class="control">
                <input
                  class="input"
                  type="datetime-local"
                  value={startDate()}
                  onInput={(e) => setStartDate(e.currentTarget.value)}
                  disabled={sessionLoading()}
                  required
                />
              </div>
              <p class="help">
                When the session opens for orders.
              </p>
            </div>

            {/* End date */}
            <div class="field">
              <label class="label">End Date & Time</label>
              <div class="control">
                <input
                  class="input"
                  type="datetime-local"
                  value={endDate()}
                  onInput={(e) => setEndDate(e.currentTarget.value)}
                  disabled={sessionLoading()}
                  required
                />
              </div>
              <p class="help">
                When the session stops accepting orders.
              </p>
            </div>

            {/* Allow late */}
            <div class="field">
              <label class="checkbox">
                <input
                  type="checkbox"
                  checked={allowLate()}
                  onChange={(e) => setAllowLate(e.currentTarget.checked)}
                  disabled={sessionLoading()}
                />
                {" "}Allow late orders after end time
              </label>
              <p class="help">
                If checked, users can still place orders after the end
                time (the session stays open until manually closed).
              </p>
            </div>

            {/* Quick presets */}
            <div class="field">
              <label class="label is-size-7 has-text-grey">Quick Presets</label>
              <div class="buttons are-small">
                <button
                  class="button is-light"
                  type="button"
                  disabled={sessionLoading()}
                  onClick={() => {
                    const now = new Date();
                    setStartDate(toDatetimeLocal(now));
                    setEndDate(
                      toDatetimeLocal(
                        new Date(now.getTime() + 30 * 60 * 1000),
                      ),
                    );
                  }}
                >
                  30 min
                </button>
                <button
                  class="button is-light"
                  type="button"
                  disabled={sessionLoading()}
                  onClick={() => {
                    const now = new Date();
                    setStartDate(toDatetimeLocal(now));
                    setEndDate(
                      toDatetimeLocal(
                        new Date(now.getTime() + 60 * 60 * 1000),
                      ),
                    );
                  }}
                >
                  1 hour
                </button>
                <button
                  class="button is-light"
                  type="button"
                  disabled={sessionLoading()}
                  onClick={() => {
                    const now = new Date();
                    setStartDate(toDatetimeLocal(now));
                    setEndDate(
                      toDatetimeLocal(
                        new Date(now.getTime() + 2 * 60 * 60 * 1000),
                      ),
                    );
                  }}
                >
                  2 hours
                </button>
                <button
                  class="button is-light"
                  type="button"
                  disabled={sessionLoading()}
                  onClick={() => {
                    const now = new Date();
                    setStartDate(toDatetimeLocal(now));
                    // End of day (23:59)
                    const eod = new Date(now);
                    eod.setHours(23, 59, 0, 0);
                    setEndDate(toDatetimeLocal(eod));
                  }}
                >
                  Until end of day
                </button>
              </div>
            </div>
          </section>

          <footer class="modal-card-foot">
            <div class="buttons">
              <button
                class="button is-primary"
                type="submit"
                classList={{ "is-loading": sessionLoading() }}
                disabled={
                  sessionLoading() || !startDate().trim() || !endDate().trim()
                }
              >
                Create Session
              </button>
              <button
                class="button"
                type="button"
                onClick={handleClose}
                disabled={sessionLoading()}
              >
                Cancel
              </button>
            </div>
          </footer>
        </form>
      </div>
    </div>
  );
}