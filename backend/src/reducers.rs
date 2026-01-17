use crate::{
    WorldObject,
    types::{AssetKind, Quat, Vec3},
    world_object,
};
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

// --------------------------------------------------------------------------------
// TODO: Use hard-coded JWT identity for editor to prevent these reducers from
// being called by other clients, there isn't a way that I'm aware of to make these
// public only to the editor client and have them ignored by the server otherwise.
// --------------------------------------------------------------------------------

#[spacetimedb::reducer]
pub fn insert_object(ctx: &ReducerContext, mut object: WorldObject) {
    if let AssetKind::Path(path) = object.asset {
        if !is_valid_relative_asset_path(&path) {
            log::warn!("insert_object rejected invalid asset path: {:?}", path);
            return;
        }

        // Normalize Windows separators to forward slashes so paths in the DB are consistent.
        object.asset = AssetKind::Path(path.replace('\\', "/"));
    }

    ctx.db.world_object().insert(object);
}

#[spacetimedb::reducer]
pub fn move_object(ctx: &ReducerContext, id: u64, translation: Vec3) -> Result<(), String> {
    let Some(mut object) = ctx.db.world_object().id().find(&id) else {
        return Err(format!("Unable to find object with ID: {}", id));
    };
    object.translation = translation;
    ctx.db.world_object().id().update(object);
    Ok(())
}

#[spacetimedb::reducer]
pub fn rotate_object(ctx: &ReducerContext, id: u64, rotation: Quat) -> Result<(), String> {
    let Some(mut object) = ctx.db.world_object().id().find(&id) else {
        return Err(format!("Unable to find object with ID: {}", id));
    };
    object.rotation = rotation;
    ctx.db.world_object().id().update(object);
    Ok(())
}

#[spacetimedb::reducer]
pub fn scale_object(ctx: &ReducerContext, id: u64, scale: Vec3) -> Result<(), String> {
    let Some(mut object) = ctx.db.world_object().id().find(&id) else {
        return Err(format!("Unable to find object with ID: {}", id));
    };
    object.scale = scale;
    ctx.db.world_object().id().update(object);
    Ok(())
}
