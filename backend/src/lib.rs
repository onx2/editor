mod reducers;
mod shape;
mod types;

use shape::CollisionShape;
use types::{AssetKind, Quat, Vec3};

/// The stored reference to an object that exists in the world.
#[spacetimedb::table(name = world_object, public)]
pub struct WorldObject {
    #[primary_key]
    #[auto_inc]
    pub id: u64,

    /// The thing that should be rendered into the world.
    /// This could be an asset from a path or a primitive shape.
    pub asset: AssetKind,

    /// The position of the object in 3D space.
    pub translation: Vec3,

    /// The orientation of the object represented as a quaternion.
    pub rotation: Quat,

    /// The scale factors applied to the object along the X, Y, and Z axes.
    pub scale: Vec3,

    /// Defines the physical boundaries and behavior of the object
    /// for physics calculations and hit detection.
    pub collision_shape: CollisionShape,
}
