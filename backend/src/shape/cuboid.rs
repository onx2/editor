use crate::types::Vec3;

/// A cuboid shape, also known as a box or rectangle.
#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq)]
pub struct Cuboid {
    pub half_extents: Vec3,
}
