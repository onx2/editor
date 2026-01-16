# Editor Client

This crate is the Bevy-based editor client.

## Runtime configuration (environment variables)

The editor client reads a few environment variables at startup to determine:

- **Where to connect for SpacetimeDB data**
- **Where to load game assets from**

These are read once on startup (before the Bevy `App` is built).

### Using a `.env` file (recommended for local dev)

Yes—you can use a `.env` file.

When you run the editor client from the workspace root via:

```/dev/null/run.example#L1-1
cargo run -p client
```

…the current working directory is the workspace root, but the client loads `.env` from the **client crate directory** (next to `client/Cargo.toml`).

- `.env` location: `editor/client/.env`
- Example template: `editor/client/.env.example` → copy to `editor/client/.env`

Example:

```/dev/null/dotenv.example#L1-6
# editor/client/.env
EDITOR_SPACETIME_URL=ws://localhost:3000
EDITOR_SPACETIME_NAME=default
EDITOR_ASSET_PATH=../example/game/assets
```

### Relative path resolution for `EDITOR_ASSET_PATH`

If `EDITOR_ASSET_PATH` is a **relative** path, it is resolved relative to the **client crate directory** (`editor/client/`), not your current working directory. This makes it stable when using `cargo run -p client` from the workspace root.

So, to point at `editor/example/game/assets`, set:

```/dev/null/assetpath.example#L1-1
EDITOR_ASSET_PATH=../example/game/assets
```

### `EDITOR_SPACETIME_URL`

SpacetimeDB websocket base URL.

- Type: `String`
- Default: `ws://127.0.0.1:3000`

Example:

```/dev/null/env.example#L1-3
EDITOR_SPACETIME_URL=ws://localhost:3000
```

### `EDITOR_SPACETIME_NAME`

SpacetimeDB database/module name to connect to.

- Type: `String`
- Default: `default`

Example:

```/dev/null/env.example#L1-3
EDITOR_SPACETIME_NAME=my_world
```

### `EDITOR_ASSET_PATH`

Overrides Bevy’s asset root directory (`AssetPlugin.file_path`).

- Type: `String` (path)
- Default: unset (Bevy default, usually `assets`)
- Notes:
  - If set to an empty string, it is treated as **unset**.
  - This is a *local filesystem* path override (useful for pointing the editor at a shared asset folder).

Examples:

```/dev/null/env.example#L1-6
# Use a shared asset directory
EDITOR_ASSET_PATH=/Users/you/projects/game/assets

# Or a relative path (relative to your working directory when you run the client)
EDITOR_ASSET_PATH=../game/assets
```

## Running

Example (macOS / Linux):

```/dev/null/run.example#L1-6
export EDITOR_SPACETIME_URL=ws://localhost:3000
export EDITOR_SPACETIME_NAME=default
export EDITOR_ASSET_PATH=../game/assets

cargo run
```

Example (single command):

```/dev/null/run.example#L1-2
EDITOR_SPACETIME_URL=ws://localhost:3000 EDITOR_SPACETIME_NAME=default EDITOR_ASSET_PATH=../game/assets cargo run
```

## Troubleshooting

- If assets don’t load, verify `EDITOR_ASSET_PATH` points to a directory that contains your asset files and that Bevy can read it.
- If SpacetimeDB doesn’t connect, verify the websocket URL is correct for your server (including scheme: `ws://` or `wss://`).