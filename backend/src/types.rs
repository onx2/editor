use crate::shape::PrimitiveShape;

#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
}

#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(spacetimedb::SpacetimeType, Debug, Clone, PartialEq)]
pub enum AssetKind {
    /// Path to the 3D asset file used to visualize this object in the world.
    /// It is expected this path is nested under your bevy asset location (defaults to /assets).
    /// Example, Some("models/alien.glb")
    Path(String),
    /// Primitive shape used to visualize this object in the world.
    PrimitiveShape(PrimitiveShape),
}
