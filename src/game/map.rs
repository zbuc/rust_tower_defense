use super::super::geometry::{BoundingBox, Point};
use crate::serde_derive::{Deserialize, Serialize};

pub const DEFAULT_MAP_NAME: &str = "Default Map";
pub const DEFAULT_MAP: &str = "data/map.bin";
pub const DEFAULT_MAP_DIMENSIONS: BoundingBox =
    BoundingBox::new(Point::new(0, 0), Point::new(100, 100));

#[derive(Debug, Deserialize, Serialize)]
pub struct GameMap {
    pub name: String,
    pub dimensions: BoundingBox,
}
