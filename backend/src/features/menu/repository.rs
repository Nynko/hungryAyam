use anyhow::{anyhow, Result};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::{
        availability::{db_model::AvailabilityRuleRow, domain::AvailabilityRule},
        item::{
            domain::item::Item,
            dto::CreateItemRequest,
            repository::ItemRepository,
        },
        menu::{
            db_model::{MenuRow, MenuSectionItemRow, MenuSectionRow},
            domain::{
                actions::update_actions::UpdateMenuAction,
                menu::Menu,
                section::{CreateMenuSection, MenuSection},
                section_item::{CreateMenuSectionItem, MenuSectionItem},
            },
            dto::CreateMenuRequest,
        },
    },
    types::{name::Name, price::PriceCents},
};

#[derive(Clone)]
pub struct MenuRepository {
    pool: PgPool,
    item_repository: ItemRepository,
}

impl MenuRepository {
    pub fn new(pool: PgPool, item_repository: ItemRepository) -> Self {
        Self {
            pool,
            item_repository,
        }
    }

    // ==================== MENU OPERATIONS ====================

    /// Create a new menu with sections and items
    pub async fn create(&self, request: CreateMenuRequest, user_id: Uuid) -> Result<Menu> {
        let mut tx = self.pool.begin().await?;

        // Insert the menu
        let menu_row = sqlx::query_as!(
            MenuRow,
            r#"
            INSERT INTO menus (restaurant_id, name, description, is_active, permanent, created_by, updated_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                restaurant_id,
                name as "name: Name",
                description,
                is_active,
                permanent,
                position,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            "#,
            request.restaurant_id,
            request.name.as_ref(),
            request.description,
            request.is_active,
            request.permanent,
            user_id,
            user_id
        )
        .fetch_one(&mut *tx)
        .await?;

        // Create sections recursively
        let sections = self
            .create_sections_recursive(&mut tx, menu_row.id, None, request.sections, user_id, 0)
            .await?;

        tx.commit().await?;

        Ok(self.row_to_menu(menu_row, sections, None))
    }

    /// Get a menu by ID with all sections and items
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Menu>> {
        let menu_row = sqlx::query_as!(
            MenuRow,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                is_active,
                permanent,
                position,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            FROM menus
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        match menu_row {
            Some(row) => {
                let sections = self.load_menu_sections(row.id).await?;
                let availability_rule = self.load_availability_rule(row.availability_rule_id).await?;
                Ok(Some(self.row_to_menu(row, sections, availability_rule)))
            }
            None => Ok(None),
        }
    }

    /// Get all menus for a restaurant
    pub async fn get_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Menu>> {
        let menu_rows = sqlx::query_as!(
            MenuRow,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                is_active,
                permanent,
                position,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            FROM menus
            WHERE restaurant_id = $1
            ORDER BY position ASC, name ASC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut menus = Vec::with_capacity(menu_rows.len());
        for row in menu_rows {
            let sections = self.load_menu_sections(row.id).await?;
            let availability_rule = self.load_availability_rule(row.availability_rule_id).await?;
            menus.push(self.row_to_menu(row, sections, availability_rule));
        }

        Ok(menus)
    }

    /// Get only active menus for a restaurant
    pub async fn get_active_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Menu>> {
        let menu_rows = sqlx::query_as!(
            MenuRow,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                is_active,
                permanent,
                position,
                created_at,
                updated_at,
                created_by,
                updated_by,
                availability_rule_id
            FROM menus
            WHERE restaurant_id = $1 AND is_active = true
            ORDER BY position ASC, name ASC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut menus = Vec::with_capacity(menu_rows.len());
        for row in menu_rows {
            let sections = self.load_menu_sections(row.id).await?;
            let availability_rule = self.load_availability_rule(row.availability_rule_id).await?;
            menus.push(self.row_to_menu(row, sections, availability_rule));
        }

        Ok(menus)
    }

    /// Delete a menu (cascade deletes sections and items)
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM menus WHERE id = $1", id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Reset a non-permanent menu – sets all items to `is_available = false`.
    /// Keeps the items in the "candidate pool" for easy re-selection.
    pub async fn reset(&self, id: Uuid, user_id: Uuid) -> Result<Option<u64>> {
        let menu = sqlx::query!(
            r#"SELECT id, permanent FROM menus WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        match menu {
            Some(m) => {
                if m.permanent {
                    return Err(anyhow!("Cannot reset a permanent menu"));
                }

                let result = sqlx::query!(
                    r#"
                    UPDATE menu_section_items
                    SET is_available = false,
                        updated_at = NOW(),
                        updated_by = $2
                    WHERE section_id IN (
                        SELECT id FROM menu_sections WHERE menu_id = $1
                    )
                    "#,
                    id,
                    user_id
                )
                .execute(&self.pool)
                .await?;

                Ok(Some(result.rows_affected()))
            }
            None => Ok(None),
        }
    }

    // ==================== UPDATE ACTIONS ====================

    /// Execute a sequence of `UpdateMenuAction`s inside a single transaction.
    ///
    /// Returns the fully-loaded menu after all actions have been applied,
    /// or `None` if the menu does not exist.
    ///
    /// Each action produces an `Option<Uuid>` that is stored in a result
    /// vector so that subsequent actions can reference newly-created entities
    /// via `EntityRef::CreatedBy(index)`.
    pub async fn execute_update_actions(
        &self,
        menu_id: Uuid,
        actions: &[UpdateMenuAction],
        user_id: Uuid,
        max_depth: i32,
    ) -> Result<Option<Menu>> {
        // Verify the menu exists before starting the transaction.
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM menus WHERE id = $1) as "exists!""#,
            menu_id
        )
        .fetch_one(&self.pool)
        .await?;

        if !exists {
            return Ok(None);
        }

        let mut tx = self.pool.begin().await?;
        let mut result_ids: Vec<Option<Uuid>> = Vec::with_capacity(actions.len());

        for (idx, action) in actions.iter().enumerate() {
            let produced_id: Option<Uuid> = match action {
                // ── 1. UpdateMenu ──────────────────────────────────────
                UpdateMenuAction::UpdateMenu(update) => {
                    if update.id != menu_id {
                        return Err(anyhow!(
                            "Action [{}]: UpdateMenu.id ({}) does not match the request menu_id ({})",
                            idx, update.id, menu_id
                        ));
                    }

                    let row_id = sqlx::query_scalar!(
                        r#"
                        UPDATE menus
                        SET name        = COALESCE($1, name),
                            description = COALESCE($2, description),
                            is_active   = COALESCE($3, is_active),
                            permanent   = COALESCE($4, permanent),
                            updated_at  = NOW(),
                            updated_by  = $6
                        WHERE id = $5
                        RETURNING id
                        "#,
                        update.name.as_ref().map(|n| n.as_ref()),
                        update.description,
                        update.is_active,
                        update.permanent,
                        update.id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    Some(row_id)
                }

                // ── 2. UpdateMenuSection ──────────────────────────────
                UpdateMenuAction::UpdateMenuSection {
                    section_id,
                    update,
                } => {
                    // section_id is a plain Uuid — it always targets an
                    // existing section (no EntityRef needed).
                    // parent_id lives inside the UpdateMenuSection struct
                    // as Option<Uuid>; COALESCE handles the "don't change"
                    // case when it is None.
                    let row_id = sqlx::query_scalar!(
                        r#"
                        UPDATE menu_sections
                        SET name        = COALESCE($1, name),
                            description = COALESCE($2, description),
                            position    = COALESCE($3, position),
                            is_active   = COALESCE($4, is_active),
                            parent_id   = COALESCE($5, parent_id),
                            updated_at  = NOW(),
                            updated_by  = $7
                        WHERE id = $6
                        RETURNING id
                        "#,
                        update.name.as_ref().map(|n| n.as_ref()),
                        update.description,
                        update.position,
                        update.is_active,
                        update.parent_id,
                        section_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    Some(row_id)
                }

                // ── 3. UpdateMenuSectionItem ──────────────────────────
                UpdateMenuAction::UpdateMenuSectionItem {
                    item_id,
                    update,
                    item_tags,
                } => {
                    // item_id is a plain Uuid — always targets an existing
                    // menu_section_item row.
                    // All fields are Option (bare `update` defaults to all_optional).
                    let row_id = sqlx::query_scalar!(
                        r#"
                        UPDATE menu_section_items
                        SET section_id           = COALESCE($1, section_id),
                            position             = COALESCE($2, position),
                            price_override_cents  = COALESCE($3, price_override_cents),
                            is_available          = COALESCE($4, is_available),
                            updated_at            = NOW(),
                            updated_by            = $6
                        WHERE id = $5
                        RETURNING id
                        "#,
                        update.section_id,
                        update.position,
                        update.price_override_cents.as_ref().map(|p| p.as_ref()),
                        update.is_available,
                        *item_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    // Optionally update the underlying catalog item.
                    if let Some(ref item_update) = update.item {
                        sqlx::query!(
                            r#"
                            UPDATE items
                            SET name              = COALESCE($1, name),
                                description       = COALESCE($2, description),
                                base_price_cents  = COALESCE($3, base_price_cents),
                                image_url         = COALESCE($4, image_url),
                                active            = COALESCE($5, active),
                                updated_at        = NOW(),
                                updated_by        = COALESCE($7, updated_by)
                            WHERE id = $6
                            "#,
                            item_update.name.as_ref().map(|n| n.as_ref()),
                            item_update.description,
                            item_update.base_price_cents.as_ref().map(|p| p.as_ref()),
                            item_update.image_url.as_ref().map(|u| u.to_string()),
                            item_update.active,
                            item_update.id,
                            user_id
                        )
                        .execute(&mut *tx)
                        .await?;

                    }

                    // Optionally replace tags on the catalog item.
                    if let Some(tags) = item_tags {
                        let catalog_item_id = sqlx::query_scalar!(
                            r#"SELECT item_id FROM menu_section_items WHERE id = $1"#,
                            *item_id
                        )
                        .fetch_one(&mut *tx)
                        .await?;

                        self.item_repository.set_item_tags(catalog_item_id, tags.to_vec()).await?;
                    }

                    Some(row_id)
                }

                // ── 4. AddSection ─────────────────────────────────────
                UpdateMenuAction::AddSection { parent_id, section } => {
                    let resolved_parent = parent_id.resolve(&result_ids)?;

                    // If the resolved parent is the menu itself → top-level
                    // (parent_id = NULL). Otherwise it is a subsection.
                    let db_parent_id = if resolved_parent == menu_id {
                        None
                    } else {
                        Some(resolved_parent)
                    };

                    // Validate nesting depth.
                    if let Some(parent_section_id) = db_parent_id {
                        let parent_depth = self
                            .get_section_depth(&mut tx, parent_section_id)
                            .await?;
                        if parent_depth + 1 > max_depth {
                            return Err(anyhow!(
                                "Action [{}]: Adding a subsection would exceed the maximum nesting depth ({}).",
                                idx,
                                max_depth
                            ));
                        }
                    }
                    // Top-level sections are at depth 1 which is always ≤ max_depth
                    // (assuming max_depth ≥ 1).

                    let row_id = sqlx::query_scalar!(
                        r#"
                        INSERT INTO menu_sections
                            (menu_id, parent_id, name, description, position, is_active, created_by, updated_by)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                        RETURNING id
                        "#,
                        menu_id,
                        db_parent_id,
                        section.name.as_ref(),
                        section.description,
                        section.position,
                        section.is_active,
                        user_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    Some(row_id)
                }

                // ── 5. AddItem ────────────────────────────────────────
                UpdateMenuAction::AddItem { section_id, item, item_tags } => {
                    let resolved_section_id = section_id.resolve(&result_ids)?;

                    // Verify the section exists and belongs to this menu.
                    let section_exists = sqlx::query_scalar!(
                        r#"SELECT EXISTS(
                            SELECT 1 FROM menu_sections
                            WHERE id = $1 AND menu_id = $2
                        ) as "exists!""#,
                        resolved_section_id,
                        menu_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    if !section_exists {
                        return Err(anyhow!(
                            "Action [{}]: Section {} does not exist in menu {}",
                            idx,
                            resolved_section_id,
                            menu_id
                        ));
                    }

                    // Create the catalog item first.
                    let new_item_id = sqlx::query_scalar!(
                        r#"
                        INSERT INTO items
                            (restaurant_id, name, description, base_price_cents,
                             image_url, active, created_by, updated_by)
                        VALUES (
                            (SELECT restaurant_id FROM menus WHERE id = $1),
                            $2, $3, $4, $5, $6, $7, $8
                        )
                        RETURNING id
                        "#,
                        menu_id,
                        item.item.name.as_ref(),
                        item.item.description,
                        item.item.base_price_cents.as_ref(),
                        item.item.image_url.as_ref().map(|u| u.to_string()),
                        item.item.active,
                        user_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    // Link the catalog item to the section.
                    let row_id = sqlx::query_scalar!(
                        r#"
                        INSERT INTO menu_section_items
                            (section_id, item_id, position, price_override_cents,
                             is_available, created_by, updated_by)
                        VALUES ($1, $2, $3, $4, $5, $6, $7)
                        RETURNING id
                        "#,
                        resolved_section_id,
                        new_item_id,
                        item.position,
                        item.price_override_cents.as_ref().map(|p| p.as_ref()),
                        item.is_available,
                        user_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    // Set tags on the newly created catalog item if provided.
                    if !item_tags.is_empty() {
                        self.item_repository.set_item_tags(new_item_id, item_tags.to_vec()).await?;
                    }

                    Some(row_id)
                }

                // ── 6. ChangePositionMenu ────────────────────────────
                UpdateMenuAction::ChangePositionMenu { position } => {
                    sqlx::query_scalar!(
                        r#"
                        UPDATE menus
                        SET position   = $1,
                            updated_at = NOW(),
                            updated_by = $3
                        WHERE id = $2
                        RETURNING id
                        "#,
                        position.as_ref(),
                        menu_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    None
                }

                // ── 7. ChangePositionSection ──────────────────────────
                UpdateMenuAction::ChangePositionSection {
                    section_id,
                    position,
                } => {
                    let resolved_id = section_id.resolve(&result_ids)?;

                    let row_id = sqlx::query_scalar!(
                        r#"
                        UPDATE menu_sections
                        SET position   = $1,
                            updated_at = NOW(),
                            updated_by = $3
                        WHERE id = $2
                        RETURNING id
                        "#,
                        position.as_ref(),
                        resolved_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    Some(row_id)
                }

                // ── 7. ChangePositionItem ─────────────────────────────
                UpdateMenuAction::ChangePositionItem { item_id, position } => {
                    let resolved_id = item_id.resolve(&result_ids)?;

                    let row_id = sqlx::query_scalar!(
                        r#"
                        UPDATE menu_section_items
                        SET position   = $1,
                            updated_at = NOW(),
                            updated_by = $3
                        WHERE id = $2
                        RETURNING id
                        "#,
                        position.as_ref(),
                        resolved_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    Some(row_id)
                }

                // ── 8. ChangeSectionForItem ───────────────────────────
                UpdateMenuAction::ChangeSectionForItem {
                    item_id,
                    section_id,
                } => {
                    let resolved_item_id = item_id.resolve(&result_ids)?;
                    let resolved_section_id = section_id.resolve(&result_ids)?;

                    // Verify the target section exists in this menu.
                    let section_exists = sqlx::query_scalar!(
                        r#"SELECT EXISTS(
                            SELECT 1 FROM menu_sections
                            WHERE id = $1 AND menu_id = $2
                        ) as "exists!""#,
                        resolved_section_id,
                        menu_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    if !section_exists {
                        return Err(anyhow!(
                            "Action [{}]: Target section {} does not exist in menu {}",
                            idx,
                            resolved_section_id,
                            menu_id
                        ));
                    }

                    let row_id = sqlx::query_scalar!(
                        r#"
                        UPDATE menu_section_items
                        SET section_id = $1,
                            updated_at = NOW(),
                            updated_by = $3
                        WHERE id = $2
                        RETURNING id
                        "#,
                        resolved_section_id,
                        resolved_item_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    Some(row_id)
                }

                // ── 9. ChangeSectionForSubSection ─────────────────────
                UpdateMenuAction::ChangeSectionForSubSection {
                    subsection_id,
                    section_id,
                } => {
                    let resolved_subsection_id = subsection_id.resolve(&result_ids)?;
                    let resolved_section_id = section_id.resolve(&result_ids)?;

                    // The new parent_id: if it equals menu_id → top-level.
                    let db_parent_id = if resolved_section_id == menu_id {
                        None
                    } else {
                        Some(resolved_section_id)
                    };

                    // Validate nesting depth after the move.
                    if let Some(new_parent_id) = db_parent_id {
                        let parent_depth = self
                            .get_section_depth(&mut tx, new_parent_id)
                            .await?;
                        let subtree_depth = self
                            .get_subtree_depth(&mut tx, resolved_subsection_id)
                            .await?;
                        if parent_depth + subtree_depth > max_depth {
                            return Err(anyhow!(
                                "Action [{}]: Moving subsection {} under section {} would exceed the maximum nesting depth ({}).",
                                idx, resolved_subsection_id, new_parent_id, max_depth
                            ));
                        }
                    }

                    // Prevent cycles: ensure new parent is not a descendant
                    // of the subsection being moved.
                    if let Some(new_parent_id) = db_parent_id {
                        let is_descendant = sqlx::query_scalar!(
                            r#"
                            WITH RECURSIVE ancestors AS (
                                SELECT id, parent_id FROM menu_sections WHERE id = $1
                                UNION ALL
                                SELECT ms.id, ms.parent_id
                                FROM menu_sections ms
                                JOIN ancestors a ON a.parent_id = ms.id
                            )
                            SELECT EXISTS(
                                SELECT 1 FROM ancestors WHERE id = $2
                            ) as "exists!"
                            "#,
                            new_parent_id,
                            resolved_subsection_id
                        )
                        .fetch_one(&mut *tx)
                        .await?;

                        if is_descendant {
                            return Err(anyhow!(
                                "Action [{}]: Cannot move subsection {} under section {} — it would create a cycle.",
                                idx, resolved_subsection_id, new_parent_id
                            ));
                        }
                    }

                    let row_id = sqlx::query_scalar!(
                        r#"
                        UPDATE menu_sections
                        SET parent_id  = $1,
                            updated_at = NOW(),
                            updated_by = $3
                        WHERE id = $2
                        RETURNING id
                        "#,
                        db_parent_id,
                        resolved_subsection_id,
                        user_id
                    )
                    .fetch_one(&mut *tx)
                    .await?;

                    Some(row_id)
                }
            };

            result_ids.push(produced_id);
        }

        tx.commit().await?;

        // Return the fully-loaded menu after all mutations.
        self.get_by_id(menu_id).await
    }

    // ==================== HELPER METHODS ====================

    /// Load all top-level sections for a menu (builds tree structure).
    async fn load_menu_sections(&self, menu_id: Uuid) -> Result<Vec<MenuSection>> {
        self.load_subsections(menu_id, None).await
    }

    /// Load subsections for a given parent (`None` = root level).
    async fn load_subsections(
        &self,
        menu_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> Result<Vec<MenuSection>> {
        let section_rows = match parent_id {
            Some(pid) => {
                sqlx::query_as!(
                    MenuSectionRow,
                    r#"
                    SELECT
                        id, menu_id, parent_id,
                        name as "name: Name",
                        description, position, is_active,
                        created_at, updated_at,
                        created_by, updated_by
                    FROM menu_sections
                    WHERE menu_id = $1 AND parent_id = $2
                    ORDER BY position ASC
                    "#,
                    menu_id,
                    pid
                )
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as!(
                    MenuSectionRow,
                    r#"
                    SELECT
                        id, menu_id, parent_id,
                        name as "name: Name",
                        description, position, is_active,
                        created_at, updated_at,
                        created_by, updated_by
                    FROM menu_sections
                    WHERE menu_id = $1 AND parent_id IS NULL
                    ORDER BY position ASC
                    "#,
                    menu_id
                )
                .fetch_all(&self.pool)
                .await?
            }
        };

        let mut sections = Vec::with_capacity(section_rows.len());
        for row in section_rows {
            let items = self.load_section_items(row.id).await?;
            let subsections = Box::pin(self.load_subsections(menu_id, Some(row.id))).await?;
            sections.push(self.row_to_section(row, items, subsections));
        }

        Ok(sections)
    }

    /// Load items for a section.
    async fn load_section_items(&self, section_id: Uuid) -> Result<Vec<MenuSectionItem>> {
        let item_rows = sqlx::query_as!(
            MenuSectionItemRow,
            r#"
            SELECT
                id, section_id, item_id, position,
                price_override_cents as "price_override_cents?: PriceCents",
                is_available,
                created_at, updated_at,
                created_by, updated_by
            FROM menu_section_items
            WHERE section_id = $1
            ORDER BY position ASC
            "#,
            section_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(item_rows.len());
        for row in item_rows {
            if let Some(catalog_item) = self.item_repository.get_by_id(row.item_id).await? {
                items.push(self.row_to_section_item(row, catalog_item));
            }
        }

        Ok(items)
    }

    /// Create sections recursively (for initial menu creation).
    async fn create_sections_recursive(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        menu_id: Uuid,
        parent_id: Option<Uuid>,
        section_inputs: Vec<CreateMenuSection>,
        user_id: Uuid,
        _depth: i32,
    ) -> Result<Vec<MenuSection>> {
        let mut sections = Vec::with_capacity(section_inputs.len());

        for input in section_inputs {
            let section_row = sqlx::query_as!(
                MenuSectionRow,
                r#"
                INSERT INTO menu_sections
                    (menu_id, parent_id, name, description, position, is_active, created_by, updated_by)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING
                    id, menu_id, parent_id,
                    name as "name: Name",
                    description, position, is_active,
                    created_at, updated_at,
                    created_by, updated_by
                "#,
                menu_id,
                parent_id,
                input.name.as_ref(),
                input.description,
                input.position,
                input.is_active,
                user_id,
                user_id
            )
            .fetch_one(&mut **tx)
            .await?;

            // Create items for this section.
            let items = self
                .create_section_items(tx, section_row.id, input.items, user_id)
                .await?;

            // Recursively create subsections.
            let subsections = Box::pin(self.create_sections_recursive(
                tx,
                menu_id,
                Some(section_row.id),
                input.subsections,
                user_id,
                _depth + 1,
            ))
            .await?;

            sections.push(self.row_to_section(section_row, items, subsections));
        }

        Ok(sections)
    }

    /// Create section items during initial menu creation.
    ///
    /// Each `CreateMenuSectionItem` contains a nested `CreateItem` that will
    /// be inserted into the catalog first.
    async fn create_section_items(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        section_id: Uuid,
        item_inputs: Vec<CreateMenuSectionItem>,
        user_id: Uuid,
    ) -> Result<Vec<MenuSectionItem>> {
        let mut items = Vec::with_capacity(item_inputs.len());

        for input in item_inputs {
            // Create the catalog item.
            // NOTE: this goes through item_repository which uses its own pool
            // connection (outside the current transaction). A future improvement
            // would be to make item creation transaction-aware.
            let catalog_item = self.item_repository.create(
                user_id,
                CreateItemRequest { item: input.item, tags: vec![] },
            ).await?;

            let item_row = sqlx::query_as!(
                MenuSectionItemRow,
                r#"
                INSERT INTO menu_section_items
                    (section_id, item_id, position, price_override_cents, is_available, created_by, updated_by)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING
                    id, section_id, item_id, position,
                    price_override_cents as "price_override_cents?: PriceCents",
                    is_available,
                    created_at, updated_at,
                    created_by, updated_by
                "#,
                section_id,
                catalog_item.id,
                input.position,
                input.price_override_cents.as_ref().map(|p| p.as_ref()),
                input.is_available,
                user_id,
                user_id
            )
            .fetch_one(&mut **tx)
            .await?;

            items.push(self.row_to_section_item(item_row, catalog_item));
        }

        Ok(items)
    }

    /// Return the depth of a section (1 = top-level, 2 = child of top-level, etc.)
    async fn get_section_depth(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        section_id: Uuid,
    ) -> Result<i32> {
        let depth = sqlx::query_scalar!(
            r#"
            WITH RECURSIVE ancestors AS (
                SELECT id, parent_id, 1 AS depth
                FROM menu_sections
                WHERE id = $1
                UNION ALL
                SELECT a.id, ms.parent_id, a.depth + 1
                FROM ancestors a
                JOIN menu_sections ms ON a.parent_id = ms.id
            )
            SELECT MAX(depth) as "depth!"
            FROM ancestors
            "#,
            section_id
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(depth)
    }

    /// Return the maximum depth of the subtree rooted at `section_id`
    /// (1 if the section has no children).
    async fn get_subtree_depth(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        section_id: Uuid,
    ) -> Result<i32> {
        let depth = sqlx::query_scalar!(
            r#"
            WITH RECURSIVE descendants AS (
                SELECT id, 1 AS depth
                FROM menu_sections
                WHERE id = $1
                UNION ALL
                SELECT ms.id, d.depth + 1
                FROM menu_sections ms
                JOIN descendants d ON ms.parent_id = d.id
            )
            SELECT MAX(depth) as "depth!"
            FROM descendants
            "#,
            section_id
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(depth)
    }

    // ==================== CONVERSION HELPERS ====================

    async fn load_availability_rule(&self, rule_id: Option<Uuid>) -> Result<Option<AvailabilityRule>> {
        match rule_id {
            Some(id) => {
                let row = sqlx::query_as!(
                    AvailabilityRuleRow,
                    r#"SELECT id, valid_from, valid_to, start_time, end_time, weekdays,
                              public_holidays_country, public_holidays_mode, active
                       FROM availability_rules WHERE id = $1"#,
                    id
                )
                .fetch_optional(&self.pool)
                .await?;
                Ok(row.map(|r| AvailabilityRule {
                    id: r.id,
                    valid_from: r.valid_from,
                    valid_to: r.valid_to,
                    start_time: r.start_time,
                    end_time: r.end_time,
                    weekdays: r.weekdays,
                    public_holidays_country: r.public_holidays_country,
                    public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s {
                        "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude),
                        "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only),
                        _ => None,
                    }),
                    active: r.active,
                }))
            }
            None => Ok(None),
        }
    }

    fn row_to_menu(&self, row: MenuRow, sections: Vec<MenuSection>, availability_rule: Option<AvailabilityRule>) -> Menu {
        Menu {
            id: row.id,
            restaurant_id: row.restaurant_id,
            name: row.name,
            description: row.description,
            is_active: row.is_active,
            permanent: row.permanent,
            position: row.position,
            updated_at: row.updated_at,
            created_by: row.created_by,
            updated_by: row.updated_by,
            sections,
            availability_rule,
        }
    }

    fn row_to_section(
        &self,
        row: MenuSectionRow,
        items: Vec<MenuSectionItem>,
        subsections: Vec<MenuSection>,
    ) -> MenuSection {
        MenuSection {
            id: row.id,
            menu_id: row.menu_id,
            parent_id: row.parent_id,
            name: row.name,
            description: row.description,
            position: row.position,
            is_active: row.is_active,
            created_by: row.created_by,
            updated_by: row.updated_by,
            items,
            subsections,
        }
    }

    fn row_to_section_item(&self, row: MenuSectionItemRow, item: Item) -> MenuSectionItem {
        MenuSectionItem {
            id: row.id,
            section_id: row.section_id,
            position: row.position,
            price_override_cents: row.price_override_cents,
            is_available: row.is_available,
            created_by: row.created_by,
            updated_by: row.updated_by,
            item,
        }
    }
}
