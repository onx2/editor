/// A ball shape, also known as a sphere in 3D or a circle in 2D.
#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq)]
pub struct Ball {
    pub radius: f32,
}
