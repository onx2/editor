use bevy::prelude::*;

/// Runtime configuration for the editor client.
///
/// This is intended to be initialized once at startup and stored as a Bevy
/// `Resource`, so all systems (UI included) can read a single source of truth.
///
/// Environment variables:
/// - `EDITOR_SPACETIME_URL`  (default: `ws://127.0.0.1:3000`)
/// - `EDITOR_SPACETIME_NAME` (default: `default`)
/// - `EDITOR_ASSET_PATH`     (optional; when unset Bevy defaults to `assets`)
#[derive(Resource, Clone, Debug)]
pub struct ClientRuntimeConfig {
    /// SpacetimeDB websocket URL, e.g. "ws://127.0.0.1:3000"
    pub spacetime_url: String,
    /// SpacetimeDB database/module name, e.g. "my_world"
    pub spacetime_name: String,
    /// Optional asset root override. If set, Bevy will load assets relative to this directory.
    /// Useful when the editor needs to pull assets from a shared location.
    pub asset_path: Option<String>,
}

impl ClientRuntimeConfig {
    pub fn from_env() -> Self {
        let spacetime_url = std::env::var("EDITOR_SPACETIME_URL")
            .unwrap_or_else(|_| "ws://127.0.0.1:3000".to_string());

        let spacetime_name =
            std::env::var("EDITOR_SPACETIME_NAME").unwrap_or_else(|_| "default".to_string());

        let asset_path = std::env::var("EDITOR_ASSET_PATH")
            .ok()
            .filter(|s| !s.is_empty());

        Self {
            spacetime_url,
            spacetime_name,
            asset_path,
        }
    }

    /// Returns the asset root directory Bevy should use for `AssetPlugin.file_path`.
    ///
    /// If `EDITOR_ASSET_PATH` is relative, it is resolved against the client crate
    /// directory (`CARGO_MANIFEST_DIR`) so it works even when you run from the
    /// workspace root (`cargo run -p client`).
    pub fn asset_root_for_bevy(&self) -> String {
        self.asset_root_resolved_or_default()
    }

    /// Returns the asset root directory to use for UI listing/browsing.
    ///
    /// Currently the same as `asset_root_for_bevy()`, but kept separate so you can
    /// later support cases like "load assets from an http base URL" while still
    /// listing local files.
    pub fn asset_root_for_listing(&self) -> String {
        self.asset_root_resolved_or_default()
    }

    fn asset_root_resolved_or_default(&self) -> String {
        let raw = self
            .asset_path
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "assets".to_string());

        let raw_path = std::path::PathBuf::from(&raw);
        if raw_path.is_absolute() {
            return raw;
        }

        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        crate_dir.join(raw_path).to_string_lossy().to_string()
    }
}
