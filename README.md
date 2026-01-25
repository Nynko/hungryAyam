# Food Ordering Backend (Rust)

This project is a **Rust backend** for a semi-private food ordering application.

The architecture intentionally prioritizes:

* **Simplicity**
* **Velocity**
* **Explicit evolution paths**

At the current stage, **domain models, database rows, and API DTOs share the same Rust structs**.
This is a deliberate choice, not a limitation.

---

## 🧱 High-Level Architecture

```
Frontend (SolidJS)
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
├─ api/
│  └─ routes/        # HTTP handlers (Axum)
│
├─ domain/           # Core models (domain / DB / API for now)
│
├─ repository/       # SQL queries (SQLx)
│
├─ services/         # Use-cases & business rules
│
├─ auth/             # JWT, cookies, middleware
│
├─ errors.rs         # API error handling
├─ app.rs            # Router & application state
└─ main.rs           # Application bootstrap
```

The **folder structure already expresses architectural intent**, even when some layers share types.

---

## 🧠 Core Model Strategy

### Single Source of Truth

Each business concept (e.g. `Restaurant`) is represented by **one Rust struct** that currently serves three roles:

1. **Domain model** (business truth)
2. **Database row model** (`sqlx::FromRow`)
3. **API DTO** (TypeScript generation)

Example:

```rust
use uuid::Uuid;
use time::OffsetDateTime;
use ts_rs::TS;

// Remove `sqlx::FromRow` if the database diverges from the domain
// Remove `TS` and `#[ts(export)]` if the frontend DTO diverges from the domain
#[derive(Debug, Clone, TS, sqlx::FromRow)]
#[ts(export)]
pub struct Restaurant {
    pub id: Uuid,
    pub name: String,
    pub image_url: Option<String>,
    pub created_at: OffsetDateTime,
}
```

This keeps the system:

* Easy to reason about
* Free of duplication
* Fast to iterate on

---

## 🧩 Type Aliases Instead of Duplicate Models

To keep **architectural clarity without duplication**, type aliases are used:

```rust
pub type RestaurantRow = Restaurant; // repository
pub type RestaurantDto = Restaurant; // API
```

This makes intent explicit:

* These roles are **conceptually distinct**
* They are **structurally identical for now**

---

## 🪜 Evolution Strategy (Very Important)

This project is designed to evolve safely.

### When the database diverges from the domain

Examples:

* denormalization
* audit fields
* joins / views
* legacy schemas

Action:

* Remove `sqlx::FromRow` from the domain model
* Introduce a dedicated `RestaurantRow`
* Add an explicit conversion

---

### When the API diverges from the domain

Examples:

* hide fields
* rename fields
* add computed fields
* version the API

Action:

* Remove `TS` and `#[ts(export)]` from the domain model
* Introduce a dedicated `RestaurantDto`
* Map domain → DTO explicitly

---

### When both diverge

The same pattern applies:

* Domain remains pure
* Repository and API get their own models
* Conversions become explicit and localized

No large refactors are required.

---

## 🗄️ Repository Layer

The repository layer:

* Contains **only SQL queries**
* Uses SQLx
* Returns domain models (or aliases)

**No business logic lives here.**

---

## ⚙️ Services Layer

Services orchestrate:

* Repositories
* Business rules
* Cross-entity workflows

Examples:

* Prevent creating multiple active orders
* Enforce deadlines
* Handle temporary vs permanent menus

Services:

* Do not know about HTTP
* Do not know about JSON

---

## 🌐 API Routes

Routes:

* Validate inputs
* Call services
* Return JSON responses

They:

* Do not contain business rules
* Do not contain SQL

---

## ⚠️ Error Handling

* Internal logic uses `anyhow`
* Public API uses a typed `ApiError`
* Internal errors are logged, not exposed

This keeps the API predictable and safe.

---

## 🗄️ Database & Migrations

* Managed via **SQLx migrations**
* Schema lives in SQL, not Rust
* Versioned and repeatable

```
migrations/
└─ 2026xxxx_init_schema.sql
```

---

## 🎯 Design Philosophy

This codebase favors:

* **Clarity over ceremony**
* **Explicit intent over abstraction**
* **Refactoring only when necessary**

As long as:

```
Domain == DB == API
```

Sharing models is the **simplest and safest option**.

When that equality breaks, the architecture already shows **where and how to split**.

---

## ✅ Summary

* One struct per concept (for now)
* Type aliases to express intent
* No duplication
* Clear evolution paths
* Business rules enforced server-side
* Frontend kept simple and untrusted

This architecture is intentionally **minimal**, **honest**, and **future-proof**.
