use crate::shape::segment::Segment;

/// A capsule shape, also known as a pill or capped cylinder.
#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub segment: Segment,
    pub radius: f32,
}
