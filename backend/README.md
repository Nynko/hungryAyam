# HungryAyam Backend (Rust)

A **Rust/Axum backend** for a semi-private group food ordering application.

The architecture intentionally prioritizes:

* **Simplicity**
* **Velocity**
* **Explicit evolution paths**

Domain models, database rows, and API DTOs share the same Rust structs by default — a deliberate choice for this stage of development, not a limitation.

---

## 🧱 High-Level Architecture

```
Frontend (SolidJS + Deno)
        |
        | JSON / HTTPS
        ▼
Rust Backend (Axum)
        |
        | SQLx
        ▼
PostgreSQL
```

The backend is the **single authority**:

* All business rules are enforced server-side
* The frontend is considered untrusted
* No direct database access from the frontend

---

## 📁 Project Structure

```
src/
├─ features/
│  ├─ auth/              # JWT, cookies, session tokens, middleware extractors
│  ├─ user/              # Users, roles, guest/password auth
│  ├─ restaurant/        # Restaurant CRUD
│  ├─ item/              # Menu items & tags
│  ├─ menu/              # Menus, sections, section items
│  ├─ order/             # Order sessions, orders, order items, settings
│  ├─ offer/             # Offer slots, constraints, price validation
│  ├─ availability/      # Availability rules (date range, time window, weekdays)
│  ├─ upload/            # Image upload & WebP conversion
│  └─ menu_scan/         # AI-powered menu scanning (Claude API)
│
├─ types/                # Validated custom types (Name, Email, PriceCents, …)
├─ errors/               # ApiError — typed errors, internal errors logged not exposed
├─ scheduler/            # Background tasks (auto-close sessions, reset menus)
├─ state.rs              # Shared app state (DB pool, config)
├─ app.rs                # Router & middleware setup
└─ main.rs               # Application bootstrap
```

---

## 🌐 API Overview

All routes are prefixed with `/api/`.

| Group | Key endpoints |
|---|---|
| **Auth** | `POST /auth/guest`, `POST /auth/login`, `POST /auth/register`, `GET /auth/me`, `POST /auth/logout` |
| **Admin** | User management, role assignment, magic link generation, editor domain config |
| **Restaurants** | CRUD + active listing |
| **Items & Tags** | CRUD, batch create, restaurant-scoped listing |
| **Menus** | CRUD with sections & items, permanent vs. non-permanent menus |
| **Order Sessions** | Lifecycle: Open → Closed/Cancelled → Sent, per-restaurant settings |
| **Orders** | Create, list (mine / all / summaries) |
| **Offers** | Slots & constraints, price validation, activate/deactivate |
| **Availability** | Rules assignable to restaurants, menus, items, or offers |
| **Uploads** | `POST /uploads` — auto-resized WebP images |
| **Menu Scan** | `POST /menu-scan` (images), `POST /menu-scan-url` (URL scraping) |
| **Setup** | `GET /setup`, `POST /setup` — first-run admin bootstrap |

---

## 🔐 Authentication

**Session-token based** — tokens live in the `user_sessions` table and are read from the `Authorization: Bearer` header or the `session_token` cookie.

### Auth methods

| Method | Description |
|---|---|
| **Guest (NameWithCookie)** | Name only, no password, 30-day session |
| **Password** | Email + Argon2 password hash, 7-day session |

### Roles

| Role | Capabilities |
|---|---|
| _(unauthenticated)_ | Site access gate only |
| **User** | View menus, create orders |
| **Editor** | + Create/edit restaurants, menus, items, offers, order sessions |
| **Admin** | + User management, role assignment, site settings |

### Site access gate

A shared site password (SHA-256 hashed, stored in `app_settings`) lets external visitors access the app. Grants a `site_access` cookie (365 days). Shareable magic links supported.

### Editor eligibility

Only password-authenticated users can become editors. An email domain restriction can optionally be enforced (configurable by admins).

---

## 🧠 Core Model Strategy

Each concept (e.g. `Restaurant`) is represented by **one Rust struct** that currently serves three roles:

1. **Domain model** — business truth
2. **Database row** — `sqlx::FromRow`
3. **API DTO** — TypeScript bindings via `ts-rs`

Type aliases express architectural intent without duplication:

```rust
pub type RestaurantRow = Restaurant; // repository layer
pub type RestaurantDto = Restaurant; // API layer
```

### Evolution path

| Scenario | Action |
|---|---|
| DB schema diverges from domain | Remove `sqlx::FromRow`, introduce `RestaurantRow`, add conversion |
| API shape diverges from domain | Remove `TS`/`#[ts(export)]`, introduce `RestaurantDto`, map explicitly |
| Both diverge | Domain stays pure; repository and API each get their own types |

No large refactors required — the architecture already shows where and how to split.

---

## ✨ Notable Features

### AI-Powered Menu Scanning

* Endpoint: `POST /api/menu-scan` (up to 5 images × 10 MB) and `POST /api/menu-scan-url` (URL scraping + images)
* Uses **Claude claude-sonnet-4-20250514** to extract sections, items, tags, and prices from menu photos or web pages
* Daily rate limits: 20 global / 5 per user
* Cancellation token propagated to Claude API on client disconnect

### Image Upload & Optimisation

* Accepts JPEG, PNG, WebP, GIF (max 10 MB)
* Auto-resized to max 1200×1200 px (Lanczos3)
* Converted to **WebP** (quality 82) and stored in `/uploads/`

### Offers / Deals System

A flexible pricing model for fixed-price menus (e.g., "menu du jour"):

* **Offer** — base price, optional menu link
* **Slots** — e.g., Starter / Main / Dessert, each with min/max item counts and a flat supplement
* **Constraints** — filter allowed items per slot (by item, tag, or menu section), each with an optional per-item supplement
* Price is validated server-side before order creation

### Availability Rules

Rules are assignable to restaurants, menus, items, and offers:

* Date range (`valid_from` / `valid_to`)
* Daily time window (`start_time` / `end_time`, supports overnight ranges)
* Weekday filter (ISO 8601: Mon=0 … Sun=6)
* Master `active` toggle

### Order Session Lifecycle

```
Open → Closed → Sent
     ↘ Cancelled
```

* Sessions auto-close after `duration_minutes` (background scheduler)
* Closed sessions can be reopened; Sent/Cancelled are terminal
* Non-permanent menus auto-reset after a session closes
* Per-restaurant: order limit and minimum order price

---

## 🗄️ Database & Migrations

* Managed via **SQLx migrations** (`migrations/`)
* 24 migrations as of April 2026
* Schema lives in SQL, not Rust — versioned and repeatable

Key tables: `app_settings`, `users`, `user_sessions`, `restaurants`, `menus`, `menu_sections`, `menu_section_items`, `items`, `tags`, `order_sessions`, `orders`, `user_order_items`, `offers`, `offer_slots`, `offer_slot_constraints`, `availability_rules`, `restaurant_order_settings`, `scheduled_tasks`

---

## ⚙️ Key Dependencies

| Crate | Purpose |
|---|---|
| `axum` 0.7 | Web framework |
| `tokio` 1 | Async runtime |
| `sqlx` 0.8 | SQL toolkit (compile-time verified queries, Postgres) |
| `serde` / `serde_json` | Serialization |
| `ts-rs` 7 | TypeScript type generation |
| `argon2` | Password hashing |
| `image` / `webp` | Image processing & WebP encoding |
| `reqwest` 0.12 | HTTP client (Claude API calls) |
| `tokio-util` | CancellationToken |
| `tracing` | Structured logging |
| `anyhow` / `thiserror` | Error handling |

---

## ⚠️ Error Handling

* Internal logic uses `anyhow`
* Public API returns a typed `ApiError`
* Internal errors are logged (via `tracing`), never exposed to clients

---

## 🎯 Design Philosophy

* **Clarity over ceremony**
* **Explicit intent over abstraction**
* **Refactor only when necessary**

As long as `Domain == DB == API`, sharing one struct is the simplest and safest option. When that equality breaks, the architecture already shows where and how to split — with no large rewrites required.
