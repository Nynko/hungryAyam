import type { AvailabilityRule } from "@bindings/AvailabilityRule";

/**
 * Check whether an availability rule allows access at the given date and time.
 *
 * Returns `true` (available) if:
 * - rule is `null` or `undefined` → always available
 * - rule.active is `false` → rule disabled, always available
 * - All active constraints pass (date range, weekday, public holidays, time range)
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

  // ── Public holiday check ───────────────────────────────────────
  if (rule.public_holidays_country && rule.public_holidays_mode) {
    const isHoliday = isPublicHoliday(rule.public_holidays_country, date);
    if (rule.public_holidays_mode === "exclude" && isHoliday) return false;
    if (rule.public_holidays_mode === "only" && !isHoliday) return false;
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
      if (timeStr < startTime || timeStr > endTime) return false;
    } else {
      // Overnight range
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

  const reasons: string[] = [];

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

  const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
  if (rule.weekdays && rule.weekdays.length > 0) {
    const jsDay = now.getDay();
    const isoDay = jsDay === 0 ? 6 : jsDay - 1;
    if (!rule.weekdays.includes(isoDay)) {
      reasons.push(`Open on ${rule.weekdays.map((d) => WEEKDAY_LABELS[d]).join(", ")}`);
    }
  }

  if (rule.public_holidays_country && rule.public_holidays_mode) {
    const isHoliday = isPublicHoliday(rule.public_holidays_country, now);
    if (rule.public_holidays_mode === "exclude" && isHoliday) {
      reasons.push("Closed on public holidays");
    } else if (rule.public_holidays_mode === "only" && !isHoliday) {
      reasons.push("Only available on public holidays");
    }
  }

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

// ── Public holiday computation ─────────────────────────────────────

function isPublicHoliday(country: string, date: Date): boolean {
  const holidays = publicHolidays(country, date.getFullYear());
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  const dateStr = `${y}-${m}-${d}`;
  return holidays.includes(dateStr);
}

/**
 * Returns public holidays as "YYYY-MM-DD" strings for the given country and year.
 * Add new countries here.
 */
function publicHolidays(country: string, year: number): string[] {
  switch (country.toUpperCase()) {
    case "FR": return frenchHolidays(year);
    default: return [];
  }
}

function frenchHolidays(year: number): string[] {
  const easter = easterSunday(year);
  const fmt = (d: Date) => {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  };
  const addDays = (d: Date, n: number) => new Date(d.getTime() + n * 86400000);

  return [
    // Fixed
    `${year}-01-01`, // New Year's Day
    `${year}-05-01`, // Labour Day
    `${year}-05-08`, // Victory in Europe Day
    `${year}-07-14`, // Bastille Day
    `${year}-08-15`, // Assumption of Mary
    `${year}-11-01`, // All Saints' Day
    `${year}-11-11`, // Armistice Day
    `${year}-12-25`, // Christmas Day
    // Easter-based
    fmt(easter),              // Easter Sunday
    fmt(addDays(easter, 1)),  // Easter Monday
    fmt(addDays(easter, 39)), // Ascension Day
    fmt(addDays(easter, 50)), // Whit Monday
  ].sort();
}

/** Anonymous Gregorian algorithm for Easter Sunday. */
function easterSunday(year: number): Date {
  const a = year % 19;
  const b = Math.floor(year / 100);
  const c = year % 100;
  const d = Math.floor(b / 4);
  const e = b % 4;
  const f = Math.floor((b + 8) / 25);
  const g = Math.floor((b - f + 1) / 3);
  const h = (19 * a + b - d - g + 15) % 30;
  const i = Math.floor(c / 4);
  const k = c % 4;
  const l = (32 + 2 * e + 2 * i - h - k) % 7;
  const m = Math.floor((a + 11 * h + 22 * l) / 451);
  const month = Math.floor((h + l - 7 * m + 114) / 31); // 1-based
  const day = ((h + l - 7 * m + 114) % 31) + 1;
  return new Date(year, month - 1, day);
}
