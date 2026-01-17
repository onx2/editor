use super::{
    ball::Ball, capsule::Capsule, convex_hull::ConvexHull, cuboid::Cuboid, heightfield::Heightfield,
};

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
