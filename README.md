# HungryAyam

<p align="center">
  <img src="docs/images/hungry-ayam-banner.png" alt="HungryAyam" width="220" />
</p>

<p align="center">
  <em>Rust + SolidJS + PostgreSQL so your team can decide what to eat for lunch.</em>
</p>

No accounts needed. Share a link, pick your food, done.  
One `docker-compose up` and a handful of env vars away from feeding your whole group.

---

## What it does

- Browse restaurant menus and place orders without creating an account
- Group orders around a session (open → closed → sent)
- Support for **daily menus** (offers/bundles with slot-based pricing)
- AI-assisted menu creation from photos
- Admin panel to manage restaurants, menus, offers, and users
- Availability rules: restrict items or menus by date, time, or weekday

<!-- Screenshot placeholder -->
<!-- ![Orders view](docs/images/orders-screenshot.png) -->

---

## Stack

| Layer    | Tech                                        |
|----------|---------------------------------------------|
| Backend  | Rust, Axum 0.7, SQLx, PostgreSQL 17         |
| Frontend | SolidJS, TypeScript, Vite, Bulma CSS        |
| Build    | Docker (multi-stage), docker-compose        |
| Auth     | HttpOnly cookies, Argon2, role-based access |

TypeScript types are auto-generated from Rust structs via `#[ts(export)]`.

---

## Project structure

```
HungryAyam/
├── backend/
│   ├── src/
│   │   ├── features/        # Business modules (restaurant, menu, offer, order, user…)
│   │   ├── scheduler/       # Background jobs (menu reset, session auto-close)
│   │   └── auth/            # Session & role middleware
│   ├── migrations/          # SQL migrations (SQLx embedded)
│   └── bindings/            # Auto-generated TypeScript types
├── frontend/
│   └── src/
│       ├── pages/           # Route-level views
│       ├── components/      # UI components
│       ├── stores/          # Reactive state (SolidJS)
│       └── lib/             # Utilities (availability, API…)
└── hungry_ayam_derive/      # Custom Rust derive macros
```

---

## Access model

- **Site password** — a shared password gates access to the whole app
- **Guests** — enter the site password, pick a name, and start ordering. No registration.
- **Password users** — guests can upgrade to a named account. Admins can grant Editor or Admin roles to unlock management features.

---

## Configuration

Copy `.env.example` to `.env` and adjust:

| Variable           | Description                                           |
|--------------------|-------------------------------------------------------|
| `POSTGRES_USER`    | PostgreSQL username                                   |
| `POSTGRES_PASSWORD`| PostgreSQL password                                   |
| `POSTGRES_DB`      | PostgreSQL database name                              |
| `APP_PORT`         | Host port for the frontend (default: `8080`)          |
| `VITE_APP_TITLE`   | App name shown in the UI and browser tab              |
| `ANTHROPIC_API_KEY`| Required for AI-assisted menu scanning from photos    |
| `SECURE_COOKIES`   | Set to `false` for local HTTP dev (default: `true`)   |
| `CORS_ORIGIN`      | Allowed CORS origin — only needed for local dev       |

---

## Running locally

```bash
# Start backend + database
docker-compose up

# Frontend (dev)
deno task dev
```

Requires Docker and Deno.

---

<p align="center">
  <img src="docs/images/cool-chicken.png" alt="" width="120" />
</p>
