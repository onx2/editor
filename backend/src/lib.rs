mod collision_shape;
mod primitives;

use collision_shape::CollisionShape;
use primitives::{Quat, Vec3};

use spacetimedb::{ReducerContext, Table};

fn is_valid_relative_asset_path(path: &str) -> bool {
    // We store asset paths relative to the asset root (Bevy AssetPlugin.file_path).
    // Examples:
    // - "FlightHelmet.gltf"
    // - "models/FlightHelmet.gltf"
    //
    // We reject:
    // - absolute paths ("/...", "C:\...")
    // - path traversal ("..")
    // - empty strings
    //
    // Note: We intentionally keep this conservative and filesystem-agnostic.
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Reject Unix absolute paths.
    if trimmed.starts_with('/') {
        return false;
    }

    // Reject Windows absolute paths (drive letters) and UNC paths.
    if trimmed.starts_with("\\\\") {
        return false;
    }
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let c0 = bytes[0];
        let c1 = bytes[1];
        let is_ascii_alpha = (b'A'..=b'Z').contains(&c0) || (b'a'..=b'z').contains(&c0);
        if is_ascii_alpha && c1 == b':' {
            return false;
        }
    }

    // Reject path traversal segments.
    // Normalize separators to '/' for the check.
    let normalized = trimmed.replace('\\', "/");
    if normalized
        .split('/')
        .any(|segment| segment == ".." || segment.is_empty())
    {
        // segment.is_empty() also rejects things like "foo//bar" and leading/trailing '/'
        return false;
    }

    true
}

/// The stored reference to an object that exists in the world.
#[spacetimedb::table(name = world_object, public)]
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

#[spacetimedb::reducer]
pub fn insert_asset(ctx: &ReducerContext, asset_path: String) {
    // Expect callers to pass a path relative to the asset root.
    // Example: "FlightHelmet.gltf" (for .../assets/FlightHelmet.gltf)
    if !is_valid_relative_asset_path(&asset_path) {
        log::warn!("insert_asset rejected invalid asset path: {:?}", asset_path);
        return;
    }

    // Normalize Windows separators to forward slashes so paths in the DB are consistent.
    let asset_path = asset_path.replace('\\', "/");

    // Insert at origin with identity rotation and unit scale.
    // Collision shape defaults to None.
    ctx.db.world_object().insert(WorldObject {
        id: 0, // auto_inc
        asset_path: Some(asset_path),
        translation: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        rotation: Quat {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
        scale: Vec3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
        collision_shape: CollisionShape::None,
    });
}
