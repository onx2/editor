mod collision_shape;
mod primitives;

use collision_shape::CollisionShape;
use primitives::{Quat, Vec3};

/// The stored reference to an object that exists in the world.
#[spacetimedb::table(name = world_object)]
pub struct WorldObject {
    #[primary_key]
    #[auto_inc]
    pub id: u64,

    /// An optional path to the 3D asset file used to visualize this object in the world.
    /// It is expected this path is nested under your bevy asset location (defaults to /assets).
    /// Example, Some("images/branding/logo.png")
    pub asset_path: Option<String>,

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
