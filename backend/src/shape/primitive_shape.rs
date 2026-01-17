use super::{ball::Ball, capsule::Capsule, cuboid::Cuboid};

#[derive(spacetimedb::SpacetimeType, Debug, Clone, PartialEq)]
pub enum PrimitiveShape {
    Cuboid(Cuboid),
    Ball(Ball),
    Capsule(Capsule),
}
