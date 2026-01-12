# Roadmap: DB-Backed Bevy Level Editor (SpacetimeDB) + Snapshot Safety + Collision

## TL;DR
- **Source of truth:** SpacetimeDB `world_objects` table. The editor renders only what it subscribes from DB (no optimistic rendering).
- **Assets:** `.glb` only for now. Asset paths in DB are **relative to the target game’s `assets/`** directory (configured in the editor).
- **Collision:** `collision_shape: Option<CollisionShape>` with primitives now; heightfield + convex hull (store points once generated) later. Rapier debug render visualizes colliders.
- **Backups:** Maintain a local snapshot file at `<project_root>/.editor/world_snapshot.(ron|json)` as a recovery mechanism.
  - Snapshot is written **only on commit/ack** (after DB subscription/cache confirms editor-initiated changes), never on a timer.
- **Safety:** If snapshot and DB differ, editor becomes **Out of Sync**, blocks editing, and prompts:
  - **Accept snapshot backup** (overwrite DB world_objects), or
  - **Accept SpacetimeDB table** (overwrite local snapshot).

This roadmap describes a practical implementation plan for a standalone Bevy editor that:
- authors static world objects stored in SpacetimeDB (`world_objects` table),
- renders strictly from subscribed DB state (no optimistic rendering),
- provides a durable local snapshot backup for recovery after accidental DB wipes,
- supports collision shapes (primitives, heightfield, convex hull) and Rapier debug visualization.

This is intentionally scoped to “what we need now” and leaves room for future expansion.

---

## Guiding principles

1. **DB is authoritative for rendering**
   - The editor scene you see is what the SpacetimeDB subscription says exists.
   - Editor actions go through reducers; the editor waits for DB events to reflect changes.

2. **Snapshot file is a recovery mechanism**
   - Snapshot is not the primary storage format.
   - Snapshot must not be overwritten by external DB wipes.
   - Snapshot is updated only after editor-initiated changes are confirmed committed (acknowledged) via DB subscription/cache.

3. **Out-of-sync blocks editing**
   - If snapshot and DB diverge, the editor blocks world edits until the user resolves.
   - Resolution is explicit and potentially destructive.

4. **Start with GLB-only + static collision**
   - `.glb` assets only for now.
   - Collision supports `None | primitives | heightfield | convex hull (stored points)`.

---

## Milestone 0 — Project wiring & config (foundation)

### Goals
- Standalone editor can be started with:
  - SpacetimeDB URL + database name
  - Project root path (recommended) and/or `assets_root` path
- Establish conventions for paths and editor-local storage.

### Deliverables
- CLI flags or config file with at least:
  - `spacetimedb_url`
  - `spacetimedb_db_name`
  - `project_root` (or `assets_root`)
- Editor-local folder created under the project:
  - `<project_root>/.editor/`
- Decide snapshot filename:
  - `<project_root>/.editor/world_snapshot.(ron|json)`

### Acceptance criteria
- Editor starts and logs configuration clearly.
- Editor resolves `assets_root` and can list files (even before UI).

---

## Milestone 1 — Backend contract (schema + reducers) (v0)

> This is implemented in the shared `backend` crate published on crates.io, then consumed by the end user’s SpacetimeDB server project.

### Goals
- Define the minimal DB contract for static world objects.

### Table: `world_objects` (minimal v0)
- `id`: stable unique identifier (client-generated recommended)
- `asset_path`: string (relative to `assets_root`)
- `translation`: (f32, f32, f32)
- `rotation`: (f32, f32, f32, f32) quaternion
- `scale`: (f32, f32, f32)
- `collision_shape`: optional `CollisionShape`

### `CollisionShape` enum (v0)
- `Box { half_extents: (f32, f32, f32) }`
- `Sphere { radius: f32 }`
- `CapsuleY { half_height: f32, radius: f32 }`
- `Heightfield { width: u32, height: u32, heights: Vec<f32>, scale: (f32, f32, f32) }` (may be deferred)
- `ConvexHull { points: Vec<(f32, f32, f32)> }` (stored once generated)

### Reducers (v0)
- `insert_object { id, asset_path, transform, collision_shape? }`
- `set_transform { id, transform }`
- `delete_object { id }`
- `set_collision_shape { id, collision_shape: Option<CollisionShape> }`
- Recovery:
  - `import_world_objects_replace { objects: Vec<WorldObjectRow> }`
    - Clears `world_objects` and inserts provided objects (destructive overwrite).

### Acceptance criteria
- End-user server can include the crate and compile.
- Reducers can be called from a client and mutate `world_objects`.

---

## Milestone 2 — SpacetimeDB client integration + authoritative rendering loop

### Goals
- Editor connects to SpacetimeDB and renders the world from `world_objects` subscription.

### Deliverables
- SpacetimeDB connection setup (url/db name).
- Subscription to `world_objects`.
- A stable mapping:
  - `ObjectId -> Entity`
- Spawn/update/despawn logic:
  - Insert row => spawn scene instance from `asset_path`
  - Update row => update `Transform`
  - Delete row => despawn
- Asset loading:
  - `.glb` via Bevy `AssetServer` load (scene root)
  - Use relative `asset_path` resolved by configured `assets_root`

### Acceptance criteria
- If `world_objects` has rows, the editor shows them.
- If a row is changed in DB, the editor updates.
- If a row is deleted, the editor removes it.

---

## Milestone 3 — Snapshot file format + atomic IO

### Goals
- Create a durable local snapshot file that can restore the DB world state.
- Snapshot is stored at `<project_root>/.editor/world_snapshot.(ron|json)`.

### Snapshot format (recommended)
- `schema_version: u32`
- `saved_at: optional timestamp`
- `world_objects: Vec<WorldObjectSnapshotRow>` (same fields as table rows needed to restore)
- Snapshot uses asset paths relative to `assets_root`.

### Atomic write requirement
- Write to a temp file, then rename over the snapshot file.
- (Optional) maintain one backup file:
  - `world_snapshot.prev` to reduce risk from bad writes.

### Acceptance criteria
- Editor can write a snapshot file successfully.
- Editor can read and parse snapshot file successfully.
- Snapshot file survives partial-write failure (atomic rename).

---

## Milestone 4 — Commit/ack-based snapshot updates (“on commit”)

### Goals
- Update snapshot only after editor-initiated operations are confirmed committed in the local DB cache/subscription.
- No timer-based autosave.

### Deliverables
- In-memory `PendingOps` buffer:
  - Insert/update/delete/collision ops keyed by `id` (and expected values/hashes as needed).
- Flow:
  1) Editor issues reducer call.
  2) Add to `PendingOps`.
  3) Observe subscription/cache events.
  4) When expected DB row state is observed, mark op committed.
  5) Trigger snapshot write.
- Debounce snapshot writes:
  - avoid rewriting snapshot on every tiny update (esp. future transform drags).

### Acceptance criteria
- Successful insert/move/delete/collision updates cause snapshot updates after confirmation.
- If DB does not reflect the change, snapshot is not updated.

---

## Milestone 5 — Sync detection + edit gating + resolution UI

### Goals
- Detect when snapshot and DB diverge and prevent world edits.
- Provide an explicit resolution choice:
  - Accept snapshot backup (overwrite DB)
  - Accept DB state (overwrite snapshot)

### Deliverables
- Hash/fingerprint function:
  - canonical ordering by `id`
  - stable serialization (or explicit hash of fields)
- Sync state machine:
  - `InSync | OutOfSync | Syncing` (optional)
- Interaction gating:
  - While `OutOfSync`, disallow:
    - insert/spawn
    - move/set_transform
    - delete
    - collision edits
  - Allow:
    - flycam, pan/zoom
    - grid toggle, collider debug toggle
    - browsing assets
- Resolution actions:
  1) **Accept snapshot backup**
     - Read snapshot file locally
     - Call reducer `import_world_objects_replace(snapshot_objects)`
     - Warning: destructive; replaces DB `world_objects`
  2) **Accept SpacetimeDB table**
     - Serialize current DB cache/table -> overwrite snapshot file
     - Warning: destructive; replaces local snapshot

### Acceptance criteria
- After an external DB wipe, editor enters OutOfSync and blocks edits.
- User can restore DB from snapshot reliably.
- User can accept DB and overwrite snapshot reliably.

---

## Milestone 6 — Content browser (GLB-only) + insert object UX

### Goals
- Let users browse `.glb` assets and insert them into the world through reducers.

### Deliverables
- UI panel listing `.glb` files under `assets_root`.
- Placement (start simple):
  - place at y=0 plane under mouse ray
  - or place at a default point in front of the camera
- Reducer call:
  - `insert_object` with client-generated `id`
- On DB ack, object appears and snapshot updates (Milestone 4).

### Acceptance criteria
- User can insert a GLB into the world.
- Object is stored in DB and appears via subscription.
- Snapshot updates after commit.

---

## Milestone 7 — Collision shapes (primitive) + Rapier debug visualization

### Goals
- Represent optional collision in DB and visualize/debug it in editor.

### Deliverables
- Convert `collision_shape` from DB row into Rapier collider components on the spawned entity:
  - `None` => no collider
  - `Box/Sphere/Capsule` => appropriate Rapier collider
- Add Rapier debug render plugin to the editor app.
- Add a hotkey and/or UI toggle for collider wireframes (recommended separate from grid).

### Acceptance criteria
- Colliders appear as wireframes when enabled.
- Changing collision in DB updates colliders in editor.

---

## Milestone 8 — Collision editing UI (minimal)

### Goals
- Allow editing collision shapes for an object and writing it back to DB.

### Deliverables
- Selection (can start simple: pick by click or select by list).
- Details panel:
  - choose collision type: None / Box / Sphere / Capsule
  - edit parameters
  - apply via `set_collision_shape` reducer
- Snapshot updates after commit/ack.

### Acceptance criteria
- User can set collision on an object and see wireframe update.
- Snapshot updates after commit.

---

## Milestone 9 — Convex hull generation (store points)

### Goals
- Generate convex hull collision data for a GLB and store it in DB.

### Deliverables
- A user action: “Generate convex hull collision”
- Implementation plan:
  1) Load the mesh data for the asset instance.
  2) Collect vertices (scene may contain multiple meshes).
  3) Compute convex hull points.
  4) Call `set_collision_shape` with `ConvexHull { points }`.
- Visualization:
  - Rapier debug render shows the hull collider.

### Acceptance criteria
- User can generate a convex hull collider and it persists via DB.
- Snapshot updates after commit/ack.

---

## Milestone 10 — Heightfield support (optional / later)

### Goals
- Support heightfield collision shapes in DB and editor.

### Notes
Heightfields can be large. Decide whether to:
- store height arrays in DB (potentially heavy), or
- store a reference to a heightmap asset and generate heights deterministically.

### Acceptance criteria
- Heightfield rows produce colliders and debug visualization in the editor.

---

## Future (beyond initial scope)
- Selection highlighting, transform gizmos, snapping.
- Undo/redo (likely implemented as a reducer/event log).
- Multi-user collaboration semantics beyond “choose snapshot vs DB”.
- Asset thumbnails and richer content browser.
- Play/preview launching the game binary, hot-reload, IPC.

---

## Immediate recommended starting point
1) Milestone 1: backend schema + reducers (v0).
2) Milestone 2: authoritative DB subscription rendering loop.
3) Milestone 3 + 4: snapshot file format + commit/ack snapshot updates.
4) Milestone 5: out-of-sync detection + gating + resolution UI.

Once those exist, the editor becomes resilient and trustworthy, and you can layer UX features safely.