extern crate rust_tower_defense;

use rust_tower_defense::geometry::{BoundingBox, Point, Polygon};

#[test]
fn bbox_point_inside_contains() {
    let p = Point::new(0, 0);
    let p2 = Point::new(2, 0);
    let p3 = Point::new(2, 2);

    let bb1 = BoundingBox::new(p, p3);

    // point on the edge is not considered inside
    assert!(!p2.inside(bb1));
    assert!(!bb1.contains(p2));

    let p4 = Point::new(1, 1);

    assert!(p4.inside(bb1));
    assert!(bb1.contains(p4));

    let p5 = Point::new(50, 50);

    assert!(!bb1.contains(p5));
    assert!(!p5.inside(bb1));
}
