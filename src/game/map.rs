use crate::geometry;

#[derive(Debug)]
pub struct GameMap {
    pub name: String,
    pub dimensions: geometry::BoundingBox,
}