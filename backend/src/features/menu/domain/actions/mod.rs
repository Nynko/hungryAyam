///Actions are here to be a set of actions that
/// a user can do to update or create an instance of a menu
///
/// Action should be linked to only 1 domain
/// Meaning, that we can only referenced one "domain object" per action
/// Even if we do composition inside of a domain (aggregated/composed domain for instance)
///
/// For instance, a user can either create the full menu from a document
/// that he will format properly OR he can:
/// 1. Create the menu (empty)
/// 2. Add a section to the menu
/// 3. Add an item to the section (or a subsection)
///
/// For Update a user can either replace the full menu OR he can:
/// 1. Update an item (price, name...) or a section (name) or the menu (name)
/// 2. Add an item / Add a section (see create actions)
/// 3. Change the order (position) of an item or subsection or section
/// 4. Change an item of section (or a subsection of section)

// pub mod create_actions; --> Actually I don't need create_actions, only update ones
// Creation is always a full one
// Clean that please and put the update_actions.rs in ../domain
pub mod update_actions;
