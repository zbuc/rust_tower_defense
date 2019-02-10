use crate::geometry;
use crate::serde_derive::{Deserialize, Serialize};

pub const DEFAULT_MAP_NAME: &str = "Default Map";
pub const DEFAULT_MAP: &str = "data/map.bin";
pub const DEFAULT_MAP_DIMENSIONS: geometry::BoundingBox =
    geometry::BoundingBox(geometry::Point(0, 0), geometry::Point(100, 100));

#[derive(Debug, Deserialize, Serialize)]
pub struct GameMap {
    pub name: String,
    pub dimensions: geometry::BoundingBox,
}
