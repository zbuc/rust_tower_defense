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

    pub fn inside(self, poly: impl Polygon<u32>) -> bool {
        poly.contains(self)
    }

    pub fn above(self, point: Point) -> bool {
        self.y() > point.y()
    }

    pub fn below(self, point: Point) -> bool {
        self.y() < point.y()
    }

    pub fn right_of(self, point: Point) -> bool {
        self.x() > point.x()
    }

    pub fn left_of(self, point: Point) -> bool {
        self.x() < point.x()
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct BoundingBox(Point, Point);

pub trait Polygon<T> {
    fn area(&self) -> T;
    fn contains(&self, point: Point) -> bool;
}

pub trait Location {
    fn get_center_point(&self) -> Point;
}

impl BoundingBox {
    pub const fn new(ll: Point, ur: Point) -> BoundingBox {
        BoundingBox(ll, ur)
    }

    pub fn lower_left(&self) -> Point {
        self.0
    }

    pub fn upper_right(&self) -> Point {
        self.1
    }
}

impl Polygon<u32> for BoundingBox {
    fn area(&self) -> u32 {
        (self.upper_right().x() - self.lower_left().x()) * (self.upper_right().y() - self.lower_left().y())
    }

    fn contains(&self, point: Point) -> bool {
        point.above(self.lower_left()) && point.right_of(self.lower_left()) && point.below(self.upper_right()) && point.left_of(self.upper_right())
    }
}