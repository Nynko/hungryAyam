# Hungry Ayam Derive Macros

Custom derive macros for domain-driven design in Rust.

## Overview

This crate provides macros to simplify the flow between DTOs, domain objects, and database models with built-in validation support.

## Macros

| Macro | Purpose |
|-------|---------|
| `domain_struct` | Generate `CreateX` and `UpdateX` structs from domain (recommended) |
| `IntoDomain` | Convert DTOs/DB models → Domain objects |

## Flows

### Simple Case: DTO = Domain

When your API contract matches your domain exactly, use type aliases:

```rust
// domain.rs
#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Restaurant {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    pub name: Name,  // Validated type - validates on deserialize!
    pub image_url: Option<UrlString>,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
}

// dto.rs - Simple type aliases!
pub type CreateRestaurantRequest = CreateRestaurant;
pub type UpdateRestaurantRequest = UpdateRestaurant;
pub type RestaurantDto = Restaurant;
```

**Flow:**
```
JSON Request
    ↓ (Deserialize - validation happens here via validated_type!)
CreateRestaurant
    ↓
Repository
    ↓
RestaurantRow (DB)
    ↓ (IntoDomain)
Restaurant
```

### Complex Case: DTO ≠ Domain (Aggregates, Different Shapes)

When your API contract differs from your domain (e.g., aggregates, different field names):

```rust
// dto.rs - Custom DTO with IntoDomain conversion
#[derive(Debug, Serialize, Deserialize, TS, IntoDomain)]
#[ts(export)]
#[into_domain(CreateUser)]
pub struct CreateUserRequest {
    pub name: Option<String>,
    #[into_domain_with_email]
    pub email: Option<String>,
    #[into_domain_with(Name::new)]  // Custom conversion function
    pub display_name: String,
}
```

**Flow:**
```
JSON Request
    ↓ (Deserialize)
CreateUserRequest (DTO)
    ↓ (IntoDomain - conversion/validation here)
CreateUser (Domain)
    ↓
Repository
    ↓
UserRow (DB)
    ↓ (IntoDomain)
User
```

### DB Model → Domain

```rust
// db_model.rs
#[derive(Debug, sqlx::FromRow, IntoDomain)]
#[into_domain(Restaurant)]
pub struct RestaurantRow {
    pub id: Uuid,
    #[into_domain_with(parse_name)]  // String → Name
    pub name: String,
    #[into_domain_with_urlstring]    // String → UrlString
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

---

## `validated_type!` Macro

Create self-validating newtypes that validate on deserialization. Define this in your backend crate.

```rust
// The conversion function: ExternalType -> Result<DomainType, Error>
pub fn parse_name(value: String) -> anyhow::Result<Name> {
    if value.trim().is_empty() {
        anyhow::bail!("Name cannot be empty");
    }
    if value.len() > 100 {
        anyhow::bail!("Name cannot exceed 100 characters");
    }
    Ok(Name(value))
}

// Create the validated type
validated_type!(
    /// A validated restaurant name (non-empty, max 100 chars).
    pub Name(String) => String, parse_name
);
```

The macro generates:
- `Serialize` / `Deserialize` (validates on deserialize!)
- `TS` for TypeScript generation
- `sqlx::Type`, `Encode`, `Decode` for database support
- `Deref`, `AsRef`, `From`, `Display`

**The same `parse_name` function is used for:**
1. Deserialization validation (via `validated_type!`)
2. DB model → Domain conversion (via `#[into_domain_with(parse_name)]`)

---

## `#[domain_struct]` Attribute

Generate `Create{Name}` and/or `Update{Name}` structs automatically.

```rust
#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Restaurant {
    #[create_ignore]        // Exclude from CreateRestaurant
    #[update_required]      // Keep as Uuid (not Option<Uuid>) in UpdateRestaurant
    pub id: Uuid,
    pub name: Name,
    pub description: Option<String>,
    #[derived_domain_ignore]  // Exclude from both
    pub created_at: DateTime<Utc>,
}

// Generates:
// - CreateRestaurant { name: Name, description: Option<String> }
// - UpdateRestaurant { id: Uuid, name: Option<Name>, description: Option<String> }
```

### Field Attributes

| Attribute | CreateX | UpdateX |
|-----------|---------|---------|
| `#[derived_domain_ignore]` | Exclude | Exclude |
| `#[create_ignore]` | Exclude | - |
| `#[update_ignore]` | - | Exclude |
| `#[create_optional]` | Wrap in Option | - |
| `#[derived_domain_optional]` | Wrap in Option | - |
| `#[update_required]` | - | Keep as-is (no Option wrap) |

**Note:** `UpdateX` automatically wraps non-Option fields in `Option<T>` unless `#[update_required]` is specified.

---

## `#[derive(IntoDomain)]`

Convert structs to domain objects with optional transformations.

```rust
#[derive(IntoDomain)]
#[into_domain(User)]
pub struct UserRow {
    pub id: Uuid,
    #[into_domain_name = "user_email"]  // Rename field
    #[into_domain_with_email]           // Parse to EmailAddress
    pub email: Option<String>,
    #[into_domain_with(parse_name)]     // Custom conversion function
    pub name: String,
    #[into_domain_ignored]              // Skip field
    pub internal: String,
}
```

### Field Attributes

| Attribute | Description |
|-----------|-------------|
| `#[into_domain(Type)]` | Target type (required, on struct) |
| `#[into_domain_name = "name"]` | Rename field in target |
| `#[into_domain_ignored]` | Skip field |
| `#[into_domain_with(fn)]` | Custom conversion function (`fn(T) -> Result<U, E>`) |
| `#[into_domain_with_email]` | `Option<String>` → `Option<EmailAddress>` |
| `#[into_domain_with_urlstring]` | `Option<String>` → `Option<UrlString>` |

---

## Requirements

Define the `IntoDomain` trait in your project:

```rust
pub trait IntoDomain<T> {
    fn into_domain(self) -> T;
}
```

---

## Complete Example

```rust
// ============ types/validated.rs ============
pub fn parse_name(value: String) -> anyhow::Result<Name> {
    if value.trim().is_empty() { anyhow::bail!("Name cannot be empty"); }
    if value.len() > 100 { anyhow::bail!("Name too long"); }
    Ok(Name(value))
}

validated_type!(
    pub Name(String) => String, parse_name
);

// ============ domain.rs ============
#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Restaurant {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    pub name: Name,
    pub image_url: Option<UrlString>,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
}

// ============ dto.rs ============
pub type CreateRestaurantRequest = CreateRestaurant;
pub type UpdateRestaurantRequest = UpdateRestaurant;

// ============ db_model.rs ============
#[derive(sqlx::FromRow, IntoDomain)]
#[into_domain(Restaurant)]
pub struct RestaurantRow {
    pub id: Uuid,
    #[into_domain_with(parse_name)]
    pub name: String,
    #[into_domain_with_urlstring]
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============ repository.rs ============
pub async fn create(&self, request: CreateRestaurant) -> Result<Restaurant> {
    let row = sqlx::query_as!(RestaurantRow,
        "INSERT INTO restaurants (name, image_url) VALUES ($1, $2) RETURNING *",
        request.name.as_ref(),  // Name derefs to &str
        option_to_string(request.image_url)
    ).fetch_one(&self.pool).await?;
    
    Ok(row.into_domain())
}
```
