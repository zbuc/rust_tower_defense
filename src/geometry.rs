use crate::serde_derive::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct Point(u32, u32);

impl Point {
    pub const fn new(x: u32, y: u32) -> Point {
        Point(x, y)
    }

    pub fn x(&self) -> &u32 {
        &self.0
    }

    pub fn y(&self) -> &u32 {
        &self.1
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct BoundingBox(Point, Point);

pub trait Polygon<T> {
    fn area(&self) -> T;
}

impl BoundingBox {
    pub const fn new(ll: Point, ur: Point) -> BoundingBox {
        BoundingBox(ll, ur)
    }

    pub fn lower_left(&self) -> &Point {
        &self.0
    }

    pub fn upper_right(&self) -> &Point {
        &self.1
    }
}

impl Polygon<u32> for BoundingBox {
    fn area(&self) -> u32 {
        (self.upper_right().x() - self.lower_left().x()) * (self.upper_right().y() - self.lower_left().y())
    }
}