use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::{
        availability::{
            db_model::AvailabilityRuleRow,
            domain::AvailabilityRule,
        },
        item::{
            db_model::ItemRow,
            domain::{
                item::Item,
                tag::{TagInput, Tag, UpdateTag},
            },
            dto::{CreateItemRequest, UpdateItemRequest},
        },
        user::domain::User,
    },
    types::{name::Name, price::PriceCents, url::ImageSource},
};

#[derive(Clone)]
pub struct ItemRepository {
    pool: PgPool,
}

impl ItemRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ==================== ITEM OPERATIONS ====================

    pub async fn create(&self, user_id: Uuid, request: CreateItemRequest) -> Result<Item> {
        let row = sqlx::query_as!(
            ItemRow,
            r#"
            INSERT INTO items (restaurant_id, name, description, base_price_cents, image_url, created_by, updated_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: ImageSource",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            "#,
            request.item.restaurant_id,
            request.item.name.as_ref(),
            request.item.description,
            request.item.base_price_cents.as_ref(),
            request.item.image_url.as_ref().map(|u| u.to_string()),
            user_id,
            user_id,
        )
        .fetch_one(&self.pool)
        .await?;

        // Set tags if provided
        let tags = if !request.tags.is_empty() {
            self.set_item_tags(row.id, request.tags).await?
        } else {
            vec![]
        };

        Ok(self.row_to_item(row, tags, None))
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Item>> {
        let row = sqlx::query_as!(
            ItemRow,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: ImageSource",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            FROM items
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let tags = self.get_tags_for_item(id).await?;
                let rule = if let Some(rule_id) = row.availability_rule_id {
                    sqlx::query_as!(
                        AvailabilityRuleRow,
                        r#"
                        SELECT id, valid_from, valid_to, start_time, end_time, weekdays, public_holidays_country, public_holidays_mode, active
                        FROM availability_rules
                        WHERE id = $1
                        "#,
                        rule_id
                    )
                    .fetch_optional(&self.pool)
                    .await?
                    .map(|r| AvailabilityRule {
                        id: r.id,
                        valid_from: r.valid_from,
                        valid_to: r.valid_to,
                        start_time: r.start_time,
                        end_time: r.end_time,
                        weekdays: r.weekdays,
                        public_holidays_country: r.public_holidays_country,
                        public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s { "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude), "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only), _ => None }),
                        active: r.active,
                    })
                } else {
                    None
                };
                Ok(Some(self.row_to_item(row, tags, rule)))
            }
            None => Ok(None),
        }
    }

    /// Get all items for a specific restaurant
    pub async fn get_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        let rows = sqlx::query_as!(
            ItemRow,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: ImageSource",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            FROM items
            WHERE restaurant_id = $1
            ORDER BY name ASC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_items_with_tags(rows).await
    }

    /// Get only active items for a specific restaurant
    pub async fn get_active_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        let rows = sqlx::query_as!(
            ItemRow,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: ImageSource",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            FROM items
            WHERE restaurant_id = $1 AND active = true
            ORDER BY name ASC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_items_with_tags(rows).await
    }

    /// Update an item
    pub async fn update(&self, user_id: Uuid, request: UpdateItemRequest) -> Result<Option<Item>> {
        let row = sqlx::query_as!(
            ItemRow,
            r#"
            UPDATE items
            SET name = COALESCE($1, name),
                description = COALESCE($2, description),
                base_price_cents = COALESCE($3, base_price_cents),
                image_url = COALESCE($4, image_url),
                active = COALESCE($5, active),
                updated_at = NOW(),
                updated_by = $7
            WHERE id = $6
            RETURNING
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: ImageSource",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            "#,
            request.item.name.as_ref().map(|n| n.as_ref()),
            request.item.description,
            request.item.base_price_cents.as_ref().map(|p| p.as_ref()),
            request.item.image_url.as_ref().map(|u| u.to_string()),
            request.item.active,
            request.item.id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                // Update tags if provided
                let tags = if let Some(tag_inputs) = request.tags {
                    self.set_item_tags(request.item.id, tag_inputs).await?
                } else {
                    self.get_tags_for_item(request.item.id).await?
                };
                let rule = if let Some(rule_id) = row.availability_rule_id {
                    sqlx::query_as!(
                        AvailabilityRuleRow,
                        r#"
                        SELECT id, valid_from, valid_to, start_time, end_time, weekdays, public_holidays_country, public_holidays_mode, active
                        FROM availability_rules
                        WHERE id = $1
                        "#,
                        rule_id
                    )
                    .fetch_optional(&self.pool)
                    .await?
                    .map(|r| AvailabilityRule {
                        id: r.id,
                        valid_from: r.valid_from,
                        valid_to: r.valid_to,
                        start_time: r.start_time,
                        end_time: r.end_time,
                        weekdays: r.weekdays,
                        public_holidays_country: r.public_holidays_country,
                        public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s { "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude), "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only), _ => None }),
                        active: r.active,
                    })
                } else {
                    None
                };
                Ok(Some(self.row_to_item(row, tags, rule)))
            }
            None => Ok(None),
        }
    }

    /// Delete an item
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM items WHERE id = $1", id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ==================== TAG OPERATIONS ====================

    /// Get all tags
    pub async fn get_all_tags(&self) -> Result<Vec<Tag>> {
        let tags = sqlx::query_as!(
            Tag,
            r#"
            SELECT id, name as "name: Name"
            FROM tags
            ORDER BY name ASC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(tags)
    }

    /// Get a tag by ID
    pub async fn get_tag_by_id(&self, id: Uuid) -> Result<Option<Tag>> {
        let tag = sqlx::query_as!(
            Tag,
            r#"
            SELECT id, name as "name: Name"
            FROM tags
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(tag)
    }

    /// Update a tag
    pub async fn update_tag(&self, request: UpdateTag) -> Result<Option<Tag>> {
        let tag = sqlx::query_as!(
            Tag,
            r#"
            UPDATE tags
            SET name = COALESCE($1, name)
            WHERE id = $2
            RETURNING id, name as "name: Name"
            "#,
            request.name.as_ref().map(|n| n.as_ref()),
            request.id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(tag)
    }

    /// Delete a tag (will cascade remove from item_tags)
    pub async fn delete_tag(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM tags WHERE id = $1", id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get tags for a specific item
    pub async fn get_tags_for_item(&self, item_id: Uuid) -> Result<Vec<Tag>> {
        let tags = sqlx::query_as!(
            Tag,
            r#"
            SELECT t.id, t.name as "name: Name"
            FROM tags t
            INNER JOIN item_tags it ON it.tag_id = t.id
            WHERE it.item_id = $1
            ORDER BY t.name ASC
            "#,
            item_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(tags)
    }

    /// Set tags for an item using TagInput (replaces existing associations).
    /// `TagInput::Existing(id)` references a tag by ID; `TagInput::New(name)` upserts by name.
    pub async fn set_item_tags(&self, item_id: Uuid, tag_inputs: Vec<TagInput>) -> Result<Vec<Tag>> {
        // Delete existing tag associations
        sqlx::query!("DELETE FROM item_tags WHERE item_id = $1", item_id)
            .execute(&self.pool)
            .await?;

        if tag_inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut tags = Vec::new();

        for input in tag_inputs {
            let tag = match input {
                TagInput::Existing(tag_id) => {
                    match self.get_tag_by_id(tag_id).await? {
                        Some(tag) => tag,
                        None => continue, // Skip if tag doesn't exist
                    }
                }
                TagInput::New(tag_name) => {
                    // Create or find tag by name (upsert)
                    sqlx::query_as!(
                        Tag,
                        r#"
                        INSERT INTO tags (name)
                        VALUES ($1)
                        ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
                        RETURNING id, name as "name: Name"
                        "#,
                        tag_name.as_ref()
                    )
                    .fetch_one(&self.pool)
                    .await?
                }
            };

            // Associate tag with item
            sqlx::query!(
                "INSERT INTO item_tags (item_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                item_id,
                tag.id
            )
            .execute(&self.pool)
            .await?;

            tags.push(tag);
        }

        Ok(tags)
    }

    // ==================== HELPERS ====================

    /// Convert an ItemRow to Item with tags and an optional availability rule
    fn row_to_item(&self, row: ItemRow, tags: Vec<Tag>, availability_rule: Option<AvailabilityRule>) -> Item {
        Item {
            id: row.id,
            restaurant_id: row.restaurant_id,
            name: row.name,
            description: row.description,
            base_price_cents: row.base_price_cents,
            image_url: row.image_url,
            active: row.active,
            created_at: row.created_at,
            updated_at: row.updated_at,
            created_by: row.created_by,
            updated_by: row.updated_by,
            tags,
            availability_rule,
        }
    }

    /// Convert multiple ItemRows to Items, fetching tags and availability rules for each
    async fn rows_to_items_with_tags(&self, rows: Vec<ItemRow>) -> Result<Vec<Item>> {
        if rows.is_empty() {
            return Ok(vec![]);
        }

        // Collect all item IDs
        let item_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();

        // Fetch all tags for these items in one query
        let item_tags = sqlx::query!(
            r#"
            SELECT it.item_id, t.id as tag_id, t.name as "tag_name: Name"
            FROM item_tags it
            INNER JOIN tags t ON t.id = it.tag_id
            WHERE it.item_id = ANY($1)
            ORDER BY t.name ASC
            "#,
            &item_ids
        )
        .fetch_all(&self.pool)
        .await?;

        // Group tags by item_id
        let mut tags_map: std::collections::HashMap<Uuid, Vec<Tag>> = std::collections::HashMap::new();
        for record in item_tags {
            let tag = Tag {
                id: record.tag_id,
                name: record.tag_name,
            };
            tags_map.entry(record.item_id).or_default().push(tag);
        }

        // Collect all availability_rule_ids that are not None
        let rule_ids: Vec<Uuid> = rows.iter().filter_map(|r| r.availability_rule_id).collect();

        let rules_map: std::collections::HashMap<Uuid, AvailabilityRule> = if !rule_ids.is_empty() {
            let rule_rows = sqlx::query_as!(
                AvailabilityRuleRow,
                r#"
                SELECT id, valid_from, valid_to, start_time, end_time, weekdays, public_holidays_country, public_holidays_mode, active
                FROM availability_rules
                WHERE id = ANY($1)
                "#,
                &rule_ids
            )
            .fetch_all(&self.pool)
            .await?;

            rule_rows
                .into_iter()
                .map(|r| {
                    let rule = AvailabilityRule {
                        id: r.id,
                        valid_from: r.valid_from,
                        valid_to: r.valid_to,
                        start_time: r.start_time,
                        end_time: r.end_time,
                        weekdays: r.weekdays,
                        public_holidays_country: r.public_holidays_country,
                        public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s { "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude), "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only), _ => None }),
                        active: r.active,
                    };
                    (rule.id, rule)
                })
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        // Convert rows to items with their tags and availability rules
        let items = rows
            .into_iter()
            .map(|row| {
                let tags = tags_map.remove(&row.id).unwrap_or_default();
                let rule = row
                    .availability_rule_id
                    .and_then(|rid| rules_map.get(&rid).cloned());
                self.row_to_item(row, tags, rule)
            })
            .collect();

        Ok(items)
    }
}