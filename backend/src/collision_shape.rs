use crate::primitives::Vec3;

#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone, Copy)]
pub struct CapsuleYData {
    half_height: f32,
    radius: f32,
}

#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone)]
pub struct HeightfieldData {
    width: u32,
    height: u32,
    heights: Vec<f32>,
    scale: Vec3,
}

#[derive(spacetimedb::SpacetimeType, Debug, Clone)]
pub enum CollisionShape {
    None,
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    CapsuleY(CapsuleYData),
    Heightfield(HeightfieldData),
    ConvexHull { points: Vec<Vec3> },
}
impl Default for CollisionShape {
    fn default() -> Self {
        Self::None
    }
}
