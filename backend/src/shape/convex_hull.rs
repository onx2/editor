use super::triangle::Triangle;
use crate::types::Vec3;

#[derive(spacetimedb::SpacetimeType, Clone, Default, Debug, PartialEq)]
pub struct ConvexHull {
    /// Point cloud
    pub points: Vec<Vec3>,
    /// Triangles that form the hull
    pub indices: Vec<Triangle>,
}
