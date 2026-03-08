use crate::features::offer::domain::{
    CreateOffer, CreateOfferSlot, CreateOfferSlotConstraint, Offer, OfferSlot,
    OfferSlotConstraint, SlotConstraintKind,
};
use crate::types::{name::Name, price::PriceCents};

// ==================== Domain Unit Tests ====================

#[test]
fn slot_constraint_kind_from_db_item() {
    let item_id = uuid::Uuid::new_v4();
    let kind = SlotConstraintKind::from_db(Some(item_id), None, None).unwrap();
    assert_eq!(kind.item_id(), Some(item_id));
    assert_eq!(kind.tag_id(), None);
    assert_eq!(kind.section_id(), None);
}

#[test]
fn slot_constraint_kind_from_db_tag() {
    let tag_id = uuid::Uuid::new_v4();
    let kind = SlotConstraintKind::from_db(None, Some(tag_id), None).unwrap();
    assert_eq!(kind.item_id(), None);
    assert_eq!(kind.tag_id(), Some(tag_id));
    assert_eq!(kind.section_id(), None);
}

#[test]
fn slot_constraint_kind_from_db_section() {
    let section_id = uuid::Uuid::new_v4();
    let kind = SlotConstraintKind::from_db(None, None, Some(section_id)).unwrap();
    assert_eq!(kind.item_id(), None);
    assert_eq!(kind.tag_id(), None);
    assert_eq!(kind.section_id(), Some(section_id));
}

#[test]
fn slot_constraint_kind_from_db_none_fails() {
    let result = SlotConstraintKind::from_db(None, None, None);
    assert!(result.is_err());
}

#[test]
fn slot_constraint_kind_from_db_multiple_fails() {
    let id1 = uuid::Uuid::new_v4();
    let id2 = uuid::Uuid::new_v4();
    let result = SlotConstraintKind::from_db(Some(id1), Some(id2), None);
    assert!(result.is_err());
}

#[test]
fn create_offer_slot_constraint_serialization_roundtrip() {
    let constraint = CreateOfferSlotConstraint {
        kind: SlotConstraintKind::Tag(uuid::Uuid::new_v4()),
    };
    let json = serde_json::to_string(&constraint).unwrap();
    let deserialized: CreateOfferSlotConstraint = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", constraint.kind),
        format!("{:?}", deserialized.kind)
    );
}

#[test]
fn create_offer_serialization_roundtrip() {
    let item_id = uuid::Uuid::new_v4();
    let restaurant_id = uuid::Uuid::new_v4();

    let create = CreateOffer {
        restaurant_id,
        menu_id: None,
        title: Name::try_from("Menu du Jour".to_string()).unwrap(),
        description: Some("Daily special menu".to_string()),
        fixed_price_cents: PriceCents::try_from(1200).unwrap(),
        is_active: true,
        slots: vec![CreateOfferSlot {
            label: Name::try_from("Starter".to_string()).unwrap(),
            min_items: 1,
            max_items: 1,
            constraints: vec![CreateOfferSlotConstraint {
                kind: SlotConstraintKind::Item(item_id),
            }],
        }],
    };

    let json = serde_json::to_string(&create).unwrap();
    let deserialized: CreateOffer = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.restaurant_id, restaurant_id);
    assert_eq!(*deserialized.title, "Menu du Jour");
    assert_eq!(*deserialized.fixed_price_cents, 1200);
    assert!(deserialized.is_active);
    assert_eq!(deserialized.slots.len(), 1);
    assert_eq!(deserialized.slots[0].min_items, 1);
    assert_eq!(deserialized.slots[0].max_items, 1);
    assert_eq!(deserialized.slots[0].constraints.len(), 1);
}

#[test]
fn slot_constraint_kind_serde_item_variant() {
    let id = uuid::Uuid::new_v4();
    let kind = SlotConstraintKind::Item(id);
    let json = serde_json::to_string(&kind).unwrap();
    let deserialized: SlotConstraintKind = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.item_id(), Some(id));
}

#[test]
fn slot_constraint_kind_serde_tag_variant() {
    let id = uuid::Uuid::new_v4();
    let kind = SlotConstraintKind::Tag(id);
    let json = serde_json::to_string(&kind).unwrap();
    let deserialized: SlotConstraintKind = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.tag_id(), Some(id));
}

#[test]
fn slot_constraint_kind_serde_section_variant() {
    let id = uuid::Uuid::new_v4();
    let kind = SlotConstraintKind::Section(id);
    let json = serde_json::to_string(&kind).unwrap();
    let deserialized: SlotConstraintKind = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.section_id(), Some(id));
}

// ==================== Service Validation Tests ====================
// NOTE: Full service-level tests (create_offer, validate_offer_order, etc.)
// require a test database and are left for integration tests.
// The domain unit tests above cover the pure logic.