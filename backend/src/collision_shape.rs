use crate::primitives::Vec3;

/// A line segment shape.
/// A segment is the simplest 1D shape, defined by two endpoints. It represents a straight line between two points with no thickness or volume.
#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq)]
pub struct Segment {
    pub a: Vec3,
    pub b: Vec3,
}

/// A capsule shape, also known as a pill or capped cylinder.
#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub segment: Segment,
    pub radius: f32,
}

/// A 3D heightfield
#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone, PartialEq)]
pub struct Heightfield {
    pub width: u32,
    pub height: u32,
    pub heights: Vec<f32>,
    pub scale: Vec3,
}

/// A cuboid shape, also known as a box or rectangle.
#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq)]
pub struct Cuboid {
    pub half_extents: Vec3,
}

/// A ball shape, also known as a sphere in 3D or a circle in 2D.
#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq)]
pub struct Ball {
    pub radius: f32,
}

#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Triangle {
    pub v1: u32,
    pub v2: u32,
    pub v3: u32,
}

#[derive(spacetimedb::SpacetimeType, Clone, Default, Debug, PartialEq)]
pub struct ConvexHull {
    /// Point cloud
    pub points: Vec<Vec3>,
    /// Triangles that form the hull
    pub indices: Vec<Triangle>,
}

#[derive(spacetimedb::SpacetimeType, Debug, Clone, PartialEq)]
pub enum CollisionShape {
    None,
    Cuboid(Cuboid),
    Ball(Ball),
    Capsule(Capsule),
    Heightfield(Heightfield),
    ConvexHull(ConvexHull),
}

impl Default for CollisionShape {
    fn default() -> Self {
        Self::None
    }
}
