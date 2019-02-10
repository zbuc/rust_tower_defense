use crate::serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Point(pub u32, pub u32);

impl Point {
    pub fn x(&self) -> &u32 {
        &self.0
    }

    pub fn y(&self) -> &u32 {
        &self.1
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BoundingBox(pub Point, pub Point);

impl BoundingBox {
    pub fn area(&self) -> u32 {
        (self.upper_right().x() - self.lower_left().x()) * (self.upper_right().y() - self.lower_left().y())
    }

    pub fn lower_left(&self) -> &Point {
        &self.0
    }

    pub fn upper_right(&self) -> &Point {
        &self.1
    }
}
