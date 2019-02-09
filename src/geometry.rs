#[derive(Debug)]
pub struct Point(pub u32, pub u32);

#[derive(Debug)]
pub struct BoundingBox(pub Point, pub Point);

impl BoundingBox {
    pub fn area(&self) -> u32 {
        ((self.1).0 - (self.0).0) * ((self.1).1 - (self.0).1)
    }
}