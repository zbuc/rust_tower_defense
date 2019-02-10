use crate::geometry;
use crate::serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GameMap {
    pub name: String,
    pub dimensions: geometry::BoundingBox,
}
