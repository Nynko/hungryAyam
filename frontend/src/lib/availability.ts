import type { AvailabilityRule } from "@bindings/AvailabilityRule";

/**
 * Check whether an availability rule allows access at the given date and time.
 *
 * Returns `true` (available) if:
 * - rule is `null` or `undefined` → always available
 * - rule.active is `false` → rule disabled, always available
 * - All active constraints pass (date range, weekday, time range)
 *
 * Time format from backend is "HH:MM:SS".
 * Date format from backend is "YYYY-MM-DD".
 * Weekdays: 0=Monday … 6=Sunday (ISO 8601).
 */
export function isAvailableNow(rule: AvailabilityRule | null | undefined): boolean {
  if (!rule) return true;
  if (!rule.active) return true;

  const now = new Date();
  return isAvailableAt(rule, now);
}

/**
 * Check availability at a specific Date.
 */
export function isAvailableAt(rule: AvailabilityRule, date: Date): boolean {
  if (!rule.active) return true;

  // ── Date range check ───────────────────────────────────────────
  // Build a YYYY-MM-DD string from the local date
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const dateStr = `${year}-${month}-${day}`;

  if (rule.valid_from && dateStr < rule.valid_from) return false;
  if (rule.valid_to && dateStr > rule.valid_to) return false;

  // ── Weekday check ──────────────────────────────────────────────
  // JS: 0=Sun, 1=Mon … 6=Sat → ISO: 0=Mon … 6=Sun
  if (rule.weekdays && rule.weekdays.length > 0) {
    const jsDay = date.getDay(); // 0=Sun
    const isoDay = jsDay === 0 ? 6 : jsDay - 1; // 0=Mon … 6=Sun
    if (!rule.weekdays.includes(isoDay)) return false;
  }

  // ── Time range check ───────────────────────────────────────────
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");
  const timeStr = `${hours}:${minutes}:${seconds}`;

  const startTime = rule.start_time;
  const endTime = rule.end_time;

  if (startTime && endTime) {
    if (startTime <= endTime) {
      // Normal range: e.g. 08:00:00 - 14:00:00
      if (timeStr < startTime || timeStr > endTime) return false;
    } else {
      // Overnight range: e.g. 22:00:00 - 06:00:00
      if (timeStr < startTime && timeStr > endTime) return false;
    }
  } else if (startTime) {
    if (timeStr < startTime) return false;
  } else if (endTime) {
    if (timeStr > endTime) return false;
  }

  return true;
}

/**
 * Human-readable summary of *when* the entity is next available,
 * or why it's currently unavailable.
 */
export function availabilityStatus(rule: AvailabilityRule | null | undefined): {
  available: boolean;
  reason: string;
} {
  if (!rule || !rule.active) {
    return { available: true, reason: "" };
  }

  const now = new Date();
  const available = isAvailableAt(rule, now);

  if (available) {
    return { available: true, reason: "" };
  }

  // Build a reason string
  const reasons: string[] = [];

  // Date check
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  const dateStr = `${year}-${month}-${day}`;

  if (rule.valid_from && dateStr < rule.valid_from) {
    reasons.push(`Opens ${rule.valid_from}`);
  }
  if (rule.valid_to && dateStr > rule.valid_to) {
    reasons.push(`Closed since ${rule.valid_to}`);
  }

  // Weekday check
  const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
  if (rule.weekdays && rule.weekdays.length > 0) {
    const jsDay = now.getDay();
    const isoDay = jsDay === 0 ? 6 : jsDay - 1;
    if (!rule.weekdays.includes(isoDay)) {
      reasons.push(`Open on ${rule.weekdays.map((d) => WEEKDAY_LABELS[d]).join(", ")}`);
    }
  }

  // Time check
  if (rule.start_time || rule.end_time) {
    const startDisplay = rule.start_time ? rule.start_time.slice(0, 5) : "";
    const endDisplay = rule.end_time ? rule.end_time.slice(0, 5) : "";
    if (startDisplay && endDisplay) {
      reasons.push(`Open ${startDisplay} – ${endDisplay}`);
    } else if (startDisplay) {
      reasons.push(`Opens at ${startDisplay}`);
    } else if (endDisplay) {
      reasons.push(`Open until ${endDisplay}`);
    }
  }

  return {
    available: false,
    reason: reasons.join(" · ") || "Currently unavailable",
  };
}