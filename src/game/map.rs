use crate::geometry;
use crate::serde_derive::{Serialize, Deserialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GameMap {
    pub name: String,
    pub dimensions: geometry::BoundingBox,
}