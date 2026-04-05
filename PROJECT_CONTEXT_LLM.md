# Project Context – Food Ordering Application

This document describes **what this project is trying to achieve** and how it is built.
It is intended to provide product-level context for humans and LLMs working on the codebase.

---

## What Is This Project?

This project is a **semi-private food ordering web application** designed
for small groups (friends, colleagues, families).

Typical use cases:
- Group lunch orders at a fixed restaurant
- Rotating daily menus ("Menu du Jour") with bundle pricing
- Temporary ordering pages shared via a link

The application is **not public-facing**, **not indexed**, and **not designed
for anonymous internet traffic**.

---

## Core Product Goals

- Extremely easy to use for invited users
- No mandatory user accounts – guests identify by name only
- Shareable links for orders
- Simple UI and predictable behavior
- Strong server-side enforcement of rules
- Low operational cost

---

## Access Model

The application uses a **trust-by-link + cookie model**:

1. **Site-level password gate** – visitors enter a shared password to view the app.
   Stored as `access_hash` in `app_settings`. Sets a `site_access` cookie.
2. **Guest users** – identify by name only. No email/password.
   Created via `POST /api/auth/guest`. Session stored in `user_sessions`.
3. **Password users** – created by admins. Can have roles: **Viewer**, **User**, **Editor**, **Admin**.
   Login via `POST /api/auth/login`.

Roles control API access:
- `SiteAccess` – site password verified (read-only browsing)
- `AuthUser` – any authenticated guest or password user (can place orders)
- `EditorUser` – Editor or Admin (manage restaurants, menus, offers, sessions)
- `AdminUser` – Admin only (user management, settings)

Sessions are stored in `user_sessions` (token in HttpOnly cookie).

---

## Core Concepts

### Restaurant
A restaurant represents a real-world place from which users order food.

A restaurant:
- Has a name and optional image, phone number, website
- Can have multiple menus and offers
- Can have an optional **availability rule**
- Has a `restaurant_order_settings` record (defaults, timezone, auto-behaviors)

---

### Menu
A menu belongs to a restaurant and represents what can be ordered.

Menus:
- Can be **permanent** (items not auto-reset) or **temporary** (items reset daily at configured time)
- Are hierarchical: sections can contain sub-sections or items
- Each section-item link has an optional `price_override_cents` and an `is_available` flag
- Can have an optional **availability rule**

The **background scheduler** resets non-permanent menu item availability daily at
`restaurant_order_settings.menu_reset_time` (restaurant local timezone).

---

### Offer
An offer represents a **bundle/fixed-price menu** (e.g. "Menu du Jour").

An offer:
- Has a `base_price_cents`
- Contains **slots** (e.g. "Choose your starter", "Choose your drink")
- Each slot has `min_items`, `max_items`, and an optional `supplement_cents`
- Each slot has **constraints** defining eligible items (by Item, Tag, or Section)
- Constraints also have an optional `supplement_cents` (per-item surcharge)
- Can optionally link to a `menu_id` (UI hint: non-permanent menus linked to an offer display as offer cards)
- Can have an optional **availability rule**

Pricing example:
- Offer base: €12.50
- Slot "Drink": +€1.50 supplement
  - Constraint "Soft drink": +€0 extra → total €14.00
  - Constraint "Alcoholic drink": +€2.50 extra → total €16.50

---

### Availability Rule
A **reusable** rule that can be attached to any of: restaurant, menu, menu_section, item, offer.

Rule dimensions (all optional, AND'd together):
- `valid_from` / `valid_to` – date range (inclusive)
- `start_time` / `end_time` – daily time window (supports overnight ranges e.g. 22:00–06:00)
- `weekdays` – array of ISO weekday numbers (0=Monday)
- `active` – master toggle (if false, entity treated as always available)

Client-side evaluation is handled in `frontend/src/lib/availability.ts`.

---

### Order Session
An order session is a **time-bounded ordering window** for a restaurant.

A session:
- Has a `start_date` and `end_date`
- Has a `status`: **Open → Closed/Cancelled → Sent** (can reopen from Closed)
- Has `allow_late` (if true, orders accepted past `end_date`)
- Only one active session per restaurant at a time

The scheduler can auto-close Open sessions when `end_date` passes
(if `restaurant_order_settings.auto_close_session = true`).

---

### Order
An order belongs to a user + session. It contains:
- Line items (`order_items`), each referencing an item, optional offer slot, and optional notes
- An optional `offer_id` (if placed via an offer)
- A `total_price_cents`

---

### User Participation
Users:
- Guests are identified per session (name + cookie)
- Password users retain identity across sessions
- Can add/modify their own selections
- Can see aggregated session summaries

---

## Non-Goals

This project intentionally does NOT aim to:
- Be a marketplace
- Handle payments
- Manage deliveries
- Provide public restaurant listings
- Scale to large anonymous audiences

---

## Technical Stack

| Layer | Technology |
|---|---|
| Backend | Rust, Axum 0.7, SQLx, Tokio |
| Database | PostgreSQL 17 |
| Frontend | SolidJS 1.9, Vite 7, TypeScript, Bulma CSS |
| Build | Docker multi-stage (Rust → Debian, Deno → Nginx) |
| Auth | HttpOnly cookies, Argon2 password hashing |
| Type safety | `#[ts(export)]` macros generate TypeScript bindings from Rust |

### Backend Architecture

- **Feature modules**: each feature has `domain/`, `service.rs`, `repository.rs`, `routes.rs`, `dto.rs`
- **Compile-time queries**: SQLx with SQLX_OFFLINE mode (`.sqlx/` cache files committed)
- **Migrations**: embedded via `sqlx::migrate!()` (21 migration files in `backend/migrations/`)
- **Custom derive macros**: `hungry_ayam_derive` generates Create/Update request structs from domain types
- **Background scheduler**: `backend/src/scheduler/` handles menu resets and session auto-close
  using `tokio::time::sleep_until` + `tokio::sync::Notify` (smart wake-up, not polling)

### Frontend Architecture

- SolidJS signals and stores for reactive state (no Redux)
- TypeScript types auto-generated from Rust structs (in `backend/bindings/`)
- Drag-and-drop via `@atlaskit/pragmatic-drag-and-drop` (menu editor)
- Complex offer editor lives in `frontend/src/components/menu-editor/OfferEditor.tsx`

---

## Design Philosophy

This project favors:
- Pragmatism over abstraction
- Explicitness over magic
- Refactoring when necessary, not upfront
- Clean separation *when it matters*

Architecture grows **only when requirements grow**.

---

## Summary for LLMs

When working on this project, keep in mind:

- This is a **small-group, semi-private tool** – UX simplicity trumps feature completeness
- **Business rules must live on the backend** (Rust, type-safe)
- **Guests are first-class users** – avoid adding mandatory auth flows
- **Availability rules are reusable** – do not embed date/time logic directly in entities
- **Offers are complex** – slots + constraints + pricing tiers; read `offer/domain.rs` before touching
- **The scheduler is smart** – it wakes on demand via Notify; don't add polling loops
- **Menus are hierarchical** – sections can nest (max depth configurable in `app_settings`)
- Avoid premature abstractions; three similar lines of code is fine
- Do not add accounts, OAuth, or identity providers

Any proposed change should:
- Improve clarity
- Reduce friction for end users
- Or support future growth without adding complexity today
