import { createSignal } from "solid-js";
import type { Restaurant } from "@bindings/Restaurant";
import type { CreateRestaurant } from "@bindings/CreateRestaurant";
import type { ApiResponse } from "@bindings/ApiResponse";

// ── State ─────────────────────────────────────────────────────────
const [restaurants, setRestaurants] = createSignal<Restaurant[]>([]);
const [restaurantsLoading, setRestaurantsLoading] = createSignal(false);
const [restaurantsError, setRestaurantsError] = createSignal<string | null>(null);

/** Timestamp of the last successful fetch (ms since epoch), or null if never fetched. */
const [lastFetchedAt, setLastFetchedAt] = createSignal<number | null>(null);

// ── Fetch / Refetch ───────────────────────────────────────────────

/**
 * Fetch all restaurants from the API and cache them in the store.
 *
 * If data is already cached and `force` is false, this is a no-op
 * (use `refetchRestaurants` when you explicitly want fresh data).
 */
async function fetchRestaurants(force = false): Promise<Restaurant[]> {
  // Skip if we already have data and the caller didn't ask to force-refresh.
  if (!force && lastFetchedAt() !== null && restaurants().length > 0) {
    return restaurants();
  }

  try {
    setRestaurantsLoading(true);
    setRestaurantsError(null);

    const res = await fetch("/api/restaurants");

    if (!res.ok) {
      throw new Error(`GET /api/restaurants responded with ${res.status}`);
    }

    const json: ApiResponse<Restaurant[]> = await res.json();

    if (json.success && json.data != null) {
      setRestaurants(json.data);
      setLastFetchedAt(Date.now());
      return json.data;
    } else {
      throw new Error(json.error ?? "Unexpected response from /api/restaurants");
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setRestaurantsError(msg);
    console.error("[restaurantStore] Failed to fetch restaurants:", msg);
    return [];
  } finally {
    setRestaurantsLoading(false);
  }
}

/**
 * Force-refresh the cached restaurant list from the API.
 */
async function refetchRestaurants(): Promise<Restaurant[]> {
  return fetchRestaurants(true);
}

/**
 * Create a new restaurant via `POST /api/restaurants`.
 *
 * On success the new restaurant is appended to the local cache
 * so a full refetch isn't needed.
 *
 * Returns the created `Restaurant` on success, or `null` on failure
 * (the error message is available via `restaurantsError()`).
 */
async function createRestaurant(
  request: CreateRestaurant,
): Promise<Restaurant | null> {
  try {
    setRestaurantsLoading(true);
    setRestaurantsError(null);

    const res = await fetch("/api/restaurants", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    const json: ApiResponse<Restaurant> = await res.json();

    if (res.ok && json.success && json.data != null) {
      // Append to the local cache so the UI updates immediately.
      setRestaurants((prev) => [...prev, json.data!]);
      return json.data;
    } else {
      const msg = json.error ?? `Create failed with status ${res.status}`;
      setRestaurantsError(msg);
      return null;
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setRestaurantsError(msg);
    console.error("[restaurantStore] Failed to create restaurant:", msg);
    return null;
  } finally {
    setRestaurantsLoading(false);
  }
}

/**
 * Clear the local cache (e.g. on logout or when navigating away).
 */
function clearRestaurants(): void {
  setRestaurants([]);
  setRestaurantsError(null);
  setLastFetchedAt(null);
}

/**
 * Clear only the error signal (e.g. when dismissing an error notification).
 */
function clearRestaurantsError(): void {
  setRestaurantsError(null);
}

export {
  restaurants,
  restaurantsLoading,
  restaurantsError,
  lastFetchedAt,
  fetchRestaurants,
  refetchRestaurants,
  createRestaurant,
  clearRestaurants,
  clearRestaurantsError,
};