use crate::types::Vec3;

/// A line segment shape.
/// A segment is the simplest 1D shape, defined by two endpoints. It represents a straight line between two points with no thickness or volume.
#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq)]
pub struct Segment {
    pub a: Vec3,
    pub b: Vec3,
}
