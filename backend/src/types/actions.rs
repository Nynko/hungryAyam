use uuid::Uuid;
use serde::{Serialize,Deserialize};
use ts_rs::TS;
use anyhow::{anyhow, Result};

/// Utils for actions domain


#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EntityRef {
    /// References an existing entity in the database
    Existing(Uuid),
    /// References the entity created by the action at this index
    CreatedByAction(usize),
}

impl EntityRef {
    /// Resolve this reference to a concrete UUID.
    ///
    /// - `Existing(uuid)` returns the uuid directly.
    /// - `CreatedBy(index)` looks up the result of a previous action by index
    ///   in the `result_ids` vector. Returns an error if the index is out of
    ///   bounds or if the referenced action did not produce an ID.
    pub fn resolve(&self, result_ids: &[Option<Uuid>]) -> Result<Uuid> {
        match self {
            EntityRef::Existing(uuid) => Ok(*uuid),
            EntityRef::CreatedByAction(index) => {
                let id = result_ids
                    .get(*index)
                    .ok_or_else(|| {
                        anyhow!(
                            "EntityRef::CreatedBy({}) is out of bounds (only {} actions executed so far)",
                            index,
                            result_ids.len()
                        )
                    })?
                    .ok_or_else(|| {
                        anyhow!(
                            "EntityRef::CreatedBy({}) references an action that did not produce an ID",
                            index
                        )
                    })?;
                Ok(id)
            }
        }
    }
}
