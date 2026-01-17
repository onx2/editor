#[derive(spacetimedb::SpacetimeType, Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Triangle {
    pub v1: u32,
    pub v2: u32,
    pub v3: u32,
}
