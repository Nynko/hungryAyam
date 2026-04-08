import { Show, createSignal, createMemo, onCleanup } from "solid-js";
import type { OrderSession } from "@bindings/OrderSession";
import {
  sessionLoading,
  sessionStatusColor,
  cancelSession,
  closeSession,
  sendSession,
  reopenSession,
  fetchActiveSession,
} from "@/stores/orderStore";
import { isAuthenticated } from "@/stores/authStore";
import { showConfirm } from "@/stores/confirmStore";

interface ActiveSessionBannerProps {
  session: OrderSession;
  restaurantId: string;
  /** Called when the session state changes (so parent can refetch). */
  onSessionChanged?: () => void;
}

/**
 * Format a duration in milliseconds to a human-readable countdown string.
 * e.g. "2h 15m" or "45m 12s" or "Ended"
 */
function formatCountdown(ms: number): string {
  if (ms <= 0) return "Ended";

  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

export default function ActiveSessionBanner(props: ActiveSessionBannerProps) {
  const [actionLoading, setActionLoading] = createSignal<string | null>(null);

  // ── Live countdown ──────────────────────────────────────────────
  const endDate = createMemo(() => new Date(props.session.end_date));
  const startDate = createMemo(() => new Date(props.session.start_date));

  const [now, setNow] = createSignal(Date.now());
  const interval = setInterval(() => setNow(Date.now()), 1000);
  onCleanup(() => clearInterval(interval));

  const timeRemaining = createMemo(() => endDate().getTime() - now());
  const hasStarted = createMemo(() => now() >= startDate().getTime());
  const hasEnded = createMemo(() => now() >= endDate().getTime());

  const countdownText = createMemo(() => {
    if (!hasStarted()) {
      const untilStart = startDate().getTime() - now();
      return `Starts in ${formatCountdown(untilStart)}`;
    }
    if (hasEnded()) {
      if (props.session.allow_late) {
        return "Past end time (late orders allowed)";
      }
      return "Ended";
    }
    return formatCountdown(timeRemaining());
  });

  const orderCount = createMemo(() => props.session.orders.length);

  const totalRevenue = createMemo(() =>
    props.session.orders.reduce((sum, o) => sum + o.total_price_cents, 0),
  );

  // ── Session actions ─────────────────────────────────────────────

  const doAction = async (
    action: "cancel" | "close" | "send" | "reopen",
    fn: (id: string) => Promise<OrderSession | null>,
    confirmOpts?: { title: string; message: string; danger?: boolean },
  ) => {
    if (confirmOpts) {
      const confirmed = await showConfirm(confirmOpts);
      if (!confirmed) return;
    }

    setActionLoading(action);
    const result = await fn(props.session.id);
    setActionLoading(null);

    if (result) {
      // Refresh the parent's active session
      await fetchActiveSession(props.restaurantId);
      props.onSessionChanged?.();
    }
  };

  const handleClose = () =>
    doAction("close", closeSession, {
      title: "Close Session?",
      message:
        "No new orders will be accepted. You can reopen it later if needed.",
    });

  const handleCancel = () =>
    doAction("cancel", cancelSession, {
      title: "Cancel Session?",
      message:
        "This will cancel the session and all its orders. This cannot be undone easily.",
      danger: true,
    });

  const handleSend = () =>
    doAction("send", sendSession, {
      title: "Send Orders?",
      message:
        "This marks the session as sent. No further changes can be made after sending.",
    });

  const handleReopen = () => doAction("reopen", reopenSession);

  const statusEmoji = createMemo(() => {
    switch (props.session.status) {
      case "Open":
        return "🟢";
      case "Closed":
        return "🟡";
      case "Sent":
        return "📨";
      case "Cancelled":
        return "❌";
      default:
        return "📋";
    }
  });

  return (
    <div class="notification mb-4">
      {/* Top row: status + countdown */}
      <div class="is-flex is-justify-content-space-between is-align-items-center is-flex-wrap-wrap" style={{ gap: "0.5rem" }}>
        <div>
          <span class="is-size-5 has-text-weight-bold">
            {statusEmoji()} Order Session
          </span>
          <span class={`tag ${sessionStatusColor(props.session.status)} ml-2`}>
            {props.session.status}
          </span>
        </div>

        <Show when={props.session.status === "Open"}>
          <div class="has-text-weight-semibold">
            <Show
              when={!hasEnded() && hasStarted()}
              fallback={
                <span class="is-size-7 has-text-weight-medium">
                  {countdownText()}
                </span>
              }
            >
              <span class="icon-text">
                <span class="icon">⏱️</span>
                <span>Orders close in {countdownText()}</span>
              </span>
            </Show>
          </div>
        </Show>
      </div>

      {/* Info row */}
      <div class="is-flex is-flex-wrap-wrap mt-2" style={{ gap: "1rem" }}>
        <Show when={props.session.pickup_time}>
          {(pt) => (
            <span class="is-size-7">
              <strong>Pickup:</strong>{" "}
              {new Date(pt()).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" })}
            </span>
          )}
        </Show>
        <span class="is-size-7">
          <strong>Orders close:</strong>{" "}
          {endDate().toLocaleString(undefined, {
            dateStyle: "short",
            timeStyle: "short",
          })}
        </span>
        <span class="is-size-7">
          <strong>Start:</strong>{" "}
          {startDate().toLocaleString(undefined, {
            dateStyle: "short",
            timeStyle: "short",
          })}
        </span>
        <Show when={props.session.allow_late}>
          <span class="tag is-warning is-small">Late orders allowed</span>
        </Show>
        <span class="is-size-7">
          <strong>Orders:</strong> {orderCount()}
        </span>
        <Show when={totalRevenue() > 0}>
          <span class="is-size-7">
            <strong>Total:</strong> €{(totalRevenue() / 100).toFixed(2)}
          </span>
        </Show>
      </div>

      {/* Admin action buttons — only for authenticated users */}
      <Show when={isAuthenticated()}>
        <div class="mt-3">
          <div class="buttons are-small">
            {/* Open session actions */}
            <Show when={props.session.status === "Open"}>
              <button
                class="button is-warning is-small"
                classList={{ "is-loading": actionLoading() === "close" }}
                disabled={sessionLoading() || actionLoading() !== null}
                onClick={handleClose}
              >
                <span class="icon is-small"><span>🔒</span></span>
                <span>Close Session</span>
              </button>
              <button
                class="button is-danger is-small is-outlined"
                classList={{ "is-loading": actionLoading() === "cancel" }}
                disabled={sessionLoading() || actionLoading() !== null}
                onClick={handleCancel}
              >
                <span class="icon is-small"><span>✕</span></span>
                <span>Cancel</span>
              </button>
            </Show>

            {/* Closed session actions */}
            <Show when={props.session.status === "Closed"}>
              <button
                class="button is-info is-small"
                classList={{ "is-loading": actionLoading() === "send" }}
                disabled={sessionLoading() || actionLoading() !== null}
                onClick={handleSend}
              >
                <span class="icon is-small"><span>📨</span></span>
                <span>Mark as Sent</span>
              </button>
              <button
                class="button is-success is-small is-outlined"
                classList={{ "is-loading": actionLoading() === "reopen" }}
                disabled={sessionLoading() || actionLoading() !== null}
                onClick={handleReopen}
              >
                <span class="icon is-small"><span>🔓</span></span>
                <span>Reopen</span>
              </button>
              <button
                class="button is-danger is-small is-outlined"
                classList={{ "is-loading": actionLoading() === "cancel" }}
                disabled={sessionLoading() || actionLoading() !== null}
                onClick={handleCancel}
              >
                <span class="icon is-small"><span>✕</span></span>
                <span>Cancel</span>
              </button>
            </Show>

            {/* Sent — terminal state, no actions */}
            <Show when={props.session.status === "Sent"}>
              <span class="is-size-7 has-text-weight-medium">
                Orders have been sent. This session is complete.
              </span>
            </Show>

            {/* Cancelled — terminal state */}
            <Show when={props.session.status === "Cancelled"}>
              <span class="is-size-7 has-text-weight-medium">
                This session was cancelled.
              </span>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}