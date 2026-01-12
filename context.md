# Editor Project Context

## TL;DR
- This repo is a standalone Bevy editor (`editor/client`, Bevy `0.17.3`) intended to author a game world whose **source of truth is SpacetimeDB** (not a local scene file).
- The editor will:
  - connect to a user-provided SpacetimeDB URL + DB name,
  - point at a user-provided Bevy project `assets/` directory,
  - let users place `.glb` assets into the world by calling reducers,
  - render the world by subscribing to the `world_objects` table and spawning/updating/despawning entities accordingly.
- Collision is **static-only** for now. Each world object can have an optional `collision_shape` supporting primitives, heightfield, and convex hull (convex hull stores points once generated).
- Safety/recovery is handled by a **local snapshot backup file** under the project root:
  - Snapshot is not the primary storage format; it’s for recovery after accidental DB wipes (`publish --delete-data`).
  - Snapshot is written **only after editor-initiated DB changes are confirmed committed** (acknowledged by the DB subscription/cache), never on a periodic timer.
  - If snapshot and DB diverge, the editor becomes **Out of Sync**, blocks editing, and prompts the user to either:
    - accept snapshot (overwrite DB), or
    - accept DB (overwrite snapshot).

This document captures the current state of the project, key architectural decisions, and the intended direction. It is written to help a future engineer (or LLM) pick up exactly where we left off.

---

## Repository layout

Workspace root: `editor/`

- `editor/client/`
  - Bevy-based standalone editor application (currently Bevy `0.17.3`).
  - Contains rendering/editor UX code (grid, flycam, etc.).
- `editor/backend/`
  - Planned: a shared SpacetimeDB schema + reducers crate (to be published on crates.io).
  - End-user game server will depend on this crate to get the tables/reducers used by the editor.
- `editor/Cargo.toml`
  - Workspace manifest.

Target end-user project structure (expected):

- `/root-of-project`
  - `/client` (bevy game app)
    - `/assets`
    - `/src/main.rs`
    - `Cargo.toml`
  - `/server` (SpacetimeDB server crate)
    - depends on the shared `backend` crate for tables/reducers
  - `Cargo.toml`

The editor is a standalone binary/executable. The user will run it and provide:
- SpacetimeDB URL and database name
- Path to the target project's root (or assets path)

---

## Implemented: infinite grid

### Goal
Provide a UE/Unity-like infinite editor grid (XZ plane at y=0), with axis lines and fade-out.

### Approach
A custom render pipeline inspired by `bevy_infinite_grid`:
- Draw a fullscreen quad (triangle strip, 4 vertices).
- In the fragment shader:
  - Reconstruct a per-pixel view ray from the camera matrices.
  - Intersect the ray with a plane.
  - Procedurally render anti-aliased grid lines using `fwidth`.

Key details:
- Rendered via `Transparent3d`.
- Uses reversed-Z depth settings (Bevy 3D default) so the grid sits in the scene correctly.
- Axis line colors are hard-coded to the typical editor convention:
  - X axis: red-ish
  - Z axis: blue-ish
- Grid is toggleable.

### Toggle + render world resource mirroring
Grid toggling is controlled by a main-world resource:
- `InfiniteGridEnabled(bool)` (defaults to enabled)

Because render systems run in the render world (`RenderApp`), the toggle resource is extracted/mirrored into the render world each frame, avoiding "resource does not exist" panics.

Hotkey:
- `G` toggles the grid visibility.

---

## Implemented: fly camera (viewport navigation)

### Goal
Unreal-like viewport fly navigation, with additional pan/zoom support and sane behavior on both mouse wheels and trackpads.

### Controls (current)
- Fly mode:
  - Hold RMB: enables look + movement
  - WASD: move
  - Q/E: down/up
- Pan:
  - Hold MMB + drag: move camera left/right/up/down (screen-space pan)
- Zoom:
  - Scroll wheel / trackpad: dolly forward/back along camera forward

### Cursor lock/hide (Bevy 0.17)
Cursor state is controlled through the `CursorOptions` component on the primary window entity (not a field on `Window`).

### Mouse wheel vs trackpad handling
Bevy `0.17` wheel events include `MouseScrollUnit`:
- `Line` (typical mouse wheel ticks): treat as discrete steps, do not scale by delta time.
- `Pixel` (typical trackpad): normalize by a tunable constant. Do not multiply by delta time (dt scaling made trackpad zoom extremely slow and made tuning feel ineffective).

Tunable settings include:
- `scroll_zoom_speed_ratio` (zoom magnitude relative to fly speed)
- `trackpad_pixels_per_scroll` (normalization factor for `Pixel` scrolling)

Note: Movement (WASD/QE) is frame-rate independent via `delta_secs()`.

---

## Product direction: DB-backed collaborative world editing (SpacetimeDB)

### Primary objective (reduced scope)
Build a standalone editor to author static world objects for a Bevy game.

- The editor is NOT focused on saving scenes to RON as a primary storage mechanism.
- The source of truth for the world is SpacetimeDB tables.
- The editor performs actions by calling reducers:
  - insert/spawn objects
  - move objects (set transform)
  - delete objects
  - set/update collision shapes
- The editor renders the world by subscribing to `world_objects` and spawning/updating/despawning entities accordingly.

### Assets
- Drag and drop `.glb` assets only (materials are out of scope).
- The editor is configured with an `assets_root` path pointing to the game project's assets directory.
- Asset paths stored in DB should be relative to the assets root (portable across machines).

### Collision
- Focus: static collision only (no friction/restitution/physics tuning initially).
- Desired DB column: `collision_shape` which supports:
  - primitive shapes (box, sphere, capsule, etc.)
  - heightfield
  - convex hull (store hull points once generated)
- Collision shape is optional:
  - `collision_shape: Option<CollisionShape>` (None means no collider)
- Collision debug visualization:
  - Use Rapier's Bevy debug render plugin.
  - The editor simply attaches the appropriate Rapier collider component to each spawned world object.

Convex hull workflow (future feature):
- User triggers generation via an editor action.
- Editor computes hull points from the asset mesh.
- Store the resulting points in DB as a `ConvexHull { points }` collision shape variant.

---

## Safety and recovery: snapshot backups + sync gating

### Problem
Users can accidentally wipe SpacetimeDB data (e.g. `publish --delete-data`), which would remove all world objects.

### Constraints
- We do NOT want the editor to accidentally overwrite its backup snapshot with "empty DB" after a wipe.
- We do NOT want periodic autosave based on current DB state.

### Snapshot file (local backup)
- The editor writes a local snapshot backup file under the project root:
  - Suggested path: `<project_root>/.editor/world_snapshot.(ron|json)`
- This is NOT the primary storage format; it is a recovery mechanism.

### Snapshot update policy (commit/ack)
- Do not autosave on a timer.
- Do not write snapshot from the DB state periodically.
- Write snapshot only after editor-initiated changes are confirmed committed (acknowledged) via SpacetimeDB subscription/cache events.

Mechanism:
- Maintain an in-memory `PendingOps` buffer for reducer calls (insert/update/delete/collision updates).
- When subscription/cache events confirm the expected change, treat it as "committed" and write snapshot ("on commit").
- Later: debounce snapshot writes to avoid rewriting constantly during drag operations.

### Out-of-sync detection
- Compare snapshot content to the current subscribed DB table state (fingerprint/hash).
- If mismatch: editor is Out of Sync.

### Out-of-sync UX and interaction gating
When Out of Sync:
- Block all world-edit operations (spawning, moving, deleting, collision edits).
- Allow non-destructive operations (camera navigation, toggles, browsing assets).

Provide a resolution dialog/banner offering:
1. **Accept snapshot backup**
   - Overwrite DB `world_objects` to match snapshot.
   - Warning: destructive; may delete DB changes.
2. **Accept SpacetimeDB table**
   - Overwrite snapshot file to match current DB state.
   - Warning: destructive; loses local backup history.

Import/restore approach:
- Import is editor-driven.
- The server should not read local files.
- Editor reads snapshot file and passes parsed `Vec<WorldObject>` to a reducer like:
  - `import_world_objects_replace(objects)`
  - which clears and re-inserts `world_objects`.

---

## Known gotchas and lessons learned
- Render-world resources must exist in `RenderApp` (or be extracted) if used by render systems.
- Shader uniform layouts must match WGSL struct layouts exactly; mismatches can make the grid disappear.
- Trackpad scroll uses pixel deltas and can be high-frequency; dt scaling can make it unresponsive and break tuning.

---

## Next major milestones (high level)
1. SpacetimeDB integration in editor client:
   - subscribe to `world_objects`
   - apply spawn/update/despawn in Bevy world
2. Implement `PendingOps` + commit/ack-based snapshot writing.
3. Implement out-of-sync detection + interaction gating.
4. Implement out-of-sync resolution UI:
   - accept snapshot → overwrite DB
   - accept DB → overwrite snapshot
5. Content browser for `.glb` under `assets_root`.
6. Insert object (click-to-place / drag-drop) via reducer.
7. Collision shapes (primitive) + Rapier debug render toggle.
8. Convex hull generation action (later).