use crate::geometry;
use crate::serde_derive::{Deserialize, Serialize};

pub const DEFAULT_MAP_NAME: &str = "Default Map";
pub const DEFAULT_MAP: &str = "data/map.bin";
pub const DEFAULT_MAP_DIMENSIONS: geometry::BoundingBox =
    geometry::BoundingBox::new(geometry::Point::new(0, 0), geometry::Point::new(100, 100));

#[derive(Debug, Deserialize, Serialize)]
pub struct GameMap {
    pub name: String,
    pub dimensions: geometry::BoundingBox,
}
