use crate::types::Vec3;

/// A 3D heightfield
#[derive(spacetimedb::SpacetimeType, Debug, Default, Clone, PartialEq)]
pub struct Heightfield {
    pub width: u32,
    pub height: u32,
    pub heights: Vec<f32>,
    pub scale: Vec3,
}
