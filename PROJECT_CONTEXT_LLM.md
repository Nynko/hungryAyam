# Project Context – Food Ordering Application

This document describes **what this project is trying to achieve**.
It is intended to provide product-level context for humans and LLMs
working on the codebase.

---

## What Is This Project?

This project is a **semi-private food ordering web application** designed
for small groups (friends, colleagues, families).

Typical use cases:
- Group lunch orders
- Shared restaurant orders
- Temporary ordering pages shared via a link

The application is **not public-facing**, **not indexed**, and **not designed
for anonymous internet traffic**.

---

## Core Product Goals

- Extremely easy to use for invited users
- No user accounts or email authentication
- Shareable links for orders
- Simple UI and predictable behavior
- Strong server-side enforcement of rules
- Low operational cost

---

## Access Model (High-Level)

The application uses a **trust-by-link + cookie model**:

- Users gain access either by:
  - Visiting a valid order link
  - Entering a simple shared password (site-level)

- Access is stored in cookies
- No persistent user accounts
- No OAuth, email, or identity providers

This is a **deliberate UX choice** to reduce friction.

---

## Core Concepts

### Restaurant
A restaurant represents a real-world place from which users order food.

A restaurant:
- Has a name and optional image
- Can have multiple menus
- Can have multiple group orders over time

---

### Menu
A menu belongs to a restaurant and represents what can be ordered.

Menus:
- Can be **permanent** (reused across orders)
- Or **temporary** (deleted after an order ends)
- Are hierarchical:
  - Sections can contain sub-sections or items

Example:
- Entrée
  - Salads
  - Soups
- Main
- Dessert

---

### Order
An order represents a **group ordering session**.

An order:
- Belongs to one restaurant
- Uses one menu
- Has a deadline (optional)
- Can allow or disallow late orders
- Is shared via a link

Rules:
- Only one active order per restaurant at a time
- A new order cannot be created if an existing order already has items

---

### User Participation
Users:
- Do not have accounts
- Are identified per order (name + cookie)
- Can add and modify their own selections
- Can see other users’ selections (optional live updates)

---

## Non-Goals

This project intentionally does NOT aim to:
- Be a marketplace
- Handle payments
- Manage deliveries
- Provide public restaurant listings
- Scale to large anonymous audiences

---

## Technical Direction

The backend is implemented in **Rust** for:
- Strong typing
- Explicit business rules
- Predictable behavior
- Long-term maintainability

The frontend is implemented in **SolidJS** and kept intentionally thin.

---

## Design Philosophy

This project favors:
- Pragmatism over abstraction
- Explicitness over magic
- Refactoring when necessary, not upfront
- Clean separation *when it matters*

The architecture is designed to grow **only when requirements grow**.

---

## Summary for LLMs

When working on this project, keep in mind:

- This is a **small-group, semi-private tool**
- UX simplicity is more important than feature completeness
- Security is “reasonable”, not enterprise-grade
- Business rules must live on the backend
- Avoid introducing accounts or complex auth flows
- Avoid premature abstractions

Any proposed change should:
- Improve clarity
- Reduce friction
- Or support future growth without adding complexity today
