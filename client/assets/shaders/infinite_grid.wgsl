// Infinite grid shader inspired by bevy_infinite_grid, adapted for editor usage in this repo.
//
// An editor-style "infinite" ground grid rendered on a plane (XZ at y=0).
// There is no large grid mesh. Instead, we render a fullscreen quad and, per
// pixel, compute where that pixel's camera ray intersects the grid plane.
//
// Pipeline sketch
// ---------------
// Vertex:
// Emits a fullscreen quad & outputs two world-space points per vertex ("near" & "far"), so the fragment stage can reconstruct a ray.
//
// Fragment:
// Ray-plane intersection -> stable 2D plane coordinates -> procedural grid/axis evaluation (anti-aliased via derivatives).
//
// Why a fullscreen quad?
// ----------------------
// Constant geometry cost (4 vertices), no tiling seams, and stable line thickness via `fwidth(...)`-based anti-aliasing.
//
// Coordinate spaces used below
// ----------------------------
// - Clip space: quad is authored directly in clip space (x,y in [-1,1]).
// - View space: used for depth computation + distance fade.
// - World space: ray-plane intersection and plane definition.
//
// Design choice: anchored 10m grid
// --------------------------------
// The 10m grid is an always-on "anchor" layer (baseline readability).
// The 1m grid fades out as you move away from the plane.
// The 100m grid fades in as you move far away from the plane.
// This reduces clutter while preserving spatial reference.
//
// Naming conventions in this file
// -------------------------------
// Suffixes:
// - `_world`   : world space
// - `_view`    : view/camera space
// - `_clip`    : clip space
// - `_meters`  : conceptual meters (1 unit == 1m when `grid_settings.scale == 1`)
// - `_scaled`  : after applying `grid_settings.scale`
//
// Common terms:
// - "coverage": 0..1 coverage of a line in the current pixel (anti-aliased mask)
// - "weight"  : user/scale-driven multiplier applied to coverage

struct GridPlane {
    // We intersect the view ray with the plane to get a 3D point in world space.
    // To draw a grid, we want a stable 2D coordinate system on that plane.
    //
    // This matrix rotates a world-space offset on the plane into "grid local"
    // space. After applying it, we use .xz as our 2D coordinates.
    //
    // In the common case (grid is XZ plane at y=0), this ends up being close to
    // identity, but we keep it generic so the grid plane can be rotated.
    planar_rotation_matrix: mat3x3<f32>,

    // A point on the plane in world space. For a y=0 ground plane, origin is typically (0,0,0).
    origin: vec3<f32>,

    // Plane normal (unit vector) in world space. For the XZ plane (y=0), this is typically (0,1,0).
    normal: vec3<f32>,
};

struct GridSettings {
    // Think of this as the "base scale" applied to plane coordinates before we
    // evaluate grid lines. In this project we want 1 Bevy unit == 1 meter, so
    // the CPU typically sets this to 1.0.
    //
    // If you set scale higher, the grid becomes denser (more lines per meter).
    scale: f32,

    // This is stored as 1.0 / fadeout_distance on the CPU so the shader can
    // multiply instead of divide (cheaper).
    //
    // Used to fade the grid as it recedes away from the camera.
    dist_fadeout_const: f32,

    // Another constant stored as a reciprocal on the CPU.
    //
    // Used to reduce aliasing when the viewing angle is very shallow relative
    // to the plane (grazing angles). The closer you look along the plane, the
    // more we fade (or soften) to avoid noisy patterns.
    dot_fadeout_const: f32,

    // These are the colored axis lines you see in editors:
    // - X axis: typically red
    // - Z axis: typically blue
    //
    // These colors are always shown (independent of grid scale selection).
    x_axis_color: vec3<f32>,
    z_axis_color: vec3<f32>,

    // Shared grid line color used for all scales.
    //
    // - RGB: line color (usually white-ish / light gray)
    // - A  : base alpha (overall grid strength)
    //
    // Per-scale visibility is controlled in the fragment shader by multiplying
    // this base alpha by:
    // - per-layer "coverage" (anti-aliased mask from `fwidth`)
    // - per-layer "weight" (scale selection / fade in-out)
    // - per-scale brightness multiplier (artistic tuning)
    grid_line_color: vec4<f32>,

    // Opacity multiplier for the axis lines.
    // (Axis RGB comes from x_axis_color / z_axis_color; axis lines are always drawn.)
    axis_alpha: f32,
 };

struct View {
    // Transforms from view space -> clip space.
    //
    // Clip space is what the GPU rasterizer uses to decide where pixels land
    // on screen.
    projection: mat4x4<f32>,

    // Transforms from clip space -> view space (inverse of projection).
    // Used for ray reconstruction / unprojection.
    inverse_projection: mat4x4<f32>,

    // In this shader, `view` is the matrix used to bring points into view space.
    // (Bevy provides matrices in a specific convention; we follow what worked in
    // bevy_infinite_grid.)
    view: mat4x4<f32>,

    // Inverse of `view`. Lets us convert from view space -> world space.
    inverse_view: mat4x4<f32>,

    // Camera position in world space (Vec3). Used for distance-based fading and angle-based fading.
    world_position: vec3<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

@group(1) @binding(0) var<uniform> grid_plane: GridPlane;
@group(1) @binding(1) var<uniform> grid_settings: GridSettings;

struct VertexInput {
    @builtin(vertex_index) index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
};

// Emits a clip-space quad (triangle strip, 4 vertices) and provides two
// world-space points per vertex so the fragment stage can reconstruct a ray.
//
// Quad corners in clip space (triangle strip order):
//  0: (-1,-1)  1: (-1, 1)  2: ( 1,-1)  3: ( 1, 1)
//
// We intentionally pick two clip-space Z values:
// - z = 1.0   : "near-ish" endpoint
// - z = 0.001 : "far-ish" endpoint
// The exact values are not magical; they just need to be distinct and stable
// so `normalize(far - near)` produces a consistent ray direction.
@vertex
fn vertex(vertex_input: VertexInput) -> VertexOutput {
    var clip_space_corners = array<vec3<f32>, 4>(
        vec3<f32>(-1.0, -1.0, 1.0),
        vec3<f32>(-1.0,  1.0, 1.0),
        vec3<f32>( 1.0, -1.0, 1.0),
        vec3<f32>( 1.0,  1.0, 1.0),
    );

    let clip_space_corner_xyz = clip_space_corners[vertex_input.index];

    var vertex_output: VertexOutput;
    vertex_output.clip_position = vec4<f32>(clip_space_corner_xyz, 1.0);

    vertex_output.near_point = unproject_point(clip_space_corner_xyz);
    vertex_output.far_point = unproject_point(vec3<f32>(clip_space_corner_xyz.xy, 0.001));
    return vertex_output;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fragment(vertex_output: VertexOutput) -> FragmentOutput {
    // Every pixel runs this function.
    //
    // Step 1: Build a view ray (in WORLD SPACE)
    // -----------------------------------------
    // The vertex shader computed two world points per pixel:
    // - near_point (world space)
    // - far_point  (world space)
    //
    // We build a ray as:
    //   ray_origin_world = near_point
    //   ray_direction_world = normalize(far_point - near_point)
    let ray_origin_world = vertex_output.near_point;
    let ray_direction_world = normalize(vertex_output.far_point - vertex_output.near_point);

    // Step 2: Define the grid plane (in WORLD SPACE)
    // ----------------------------------------------
    let plane_normal_world = grid_plane.normal;
    let plane_origin_world = grid_plane.origin;

    // Step 3: Ray-plane intersection
    // ------------------------------
    // We solve for t in:
    //   ray_origin_world + ray_direction_world * t
    // which gives the intersection point along the ray.
    //
    // Derivation:
    // - A plane can be described by a point on the plane (plane_origin_world) and
    //   a normal vector (plane_normal_world).
    // - A point p is on the plane when:
    //       dot(plane_normal_world, (p - plane_origin_world)) = 0
    // - Substitute the ray equation p = ray_origin_world + ray_direction_world * t:
    //       dot(n, (ro + rd*t - o)) = 0
    //       dot(n, (ro - o)) + t*dot(n, rd) = 0
    //       t = dot(n, (o - ro)) / dot(n, rd)
    let ray_dot_plane_normal = dot(ray_direction_world, plane_normal_world);

    // If ray_dot_plane_normal is near 0, the ray is nearly parallel to the plane.
    // The intersection would be extremely far away and numerically unstable, so we
    // draw nothing (transparent).
    //
    // We also set depth to "far" (1.0) so this fragment doesn't occlude real geometry.
    if abs(ray_dot_plane_normal) < 1e-6 {
        return FragmentOutput(vec4<f32>(0.0), 1.0);
    }

    let ray_t_to_plane = dot(plane_normal_world, (plane_origin_world - ray_origin_world)) / ray_dot_plane_normal;

    // If ray_t_to_plane <= 0, the intersection is behind the camera/ray start.
    // We don't draw the grid "behind" the viewer.
    if ray_t_to_plane <= 0.0 {
        return FragmentOutput(vec4<f32>(0.0), 1.0);
    }

    // This is the 3D intersection point (WORLD SPACE) for this pixel.
    let intersection_world_pos = ray_origin_world + ray_direction_world * ray_t_to_plane;

    // Step 4: Convert the world-space intersection to 2D plane coordinates
    // --------------------------------------------------------------------
    // We need a stable 2D coordinate system on the plane so we can evaluate a grid.
    //
    // 1) Compute the world-space offset from the plane origin to the intersection:
    //      planar_offset_world = intersection_world_pos - plane_origin_world
    // 2) Rotate that offset into a stable "plane-local" coordinate system:
    //      planar_offset_plane_local = planar_rotation_matrix * planar_offset_world
    // 3) Use the XZ components as our 2D plane coordinates.
    let planar_offset_world = intersection_world_pos - plane_origin_world;
    let plane_local_coords_meters = (grid_plane.planar_rotation_matrix * planar_offset_world).xz;

    // Step 5: Compute depth for the intersection point
    // ------------------------------------------------
    // We output custom fragment depth so the grid is depth-tested like real geometry
    // (so scene objects can occlude it).
    //
    // Steps:
    // - world -> view -> clip
    // - depth = clip.z / clip.w
    //
    // Note: Bevy commonly uses reversed-Z in its 3D pipelines, so "near/far" behavior
    // depends on the pipeline state. Here we just output the projected clip depth.
    let intersection_view_space = view.inverse_view * vec4<f32>(intersection_world_pos, 1.0);
    let intersection_clip_space = view.projection * intersection_view_space;
    let clip_depth = intersection_clip_space.z / intersection_clip_space.w;

    // View-space distance along -Z (used for distance-based fading).
    let view_space_depth = -intersection_view_space.z;

    // Procedural grid evaluation.
    //
    // Target behavior:
    // - Supported scales: 1m, 10m, 100m
    // - At most TWO adjacent scales visible at a time
    // - Strict crossfade: when one scale is 100% visible, the other is 0% visible
    // - Crossfade is biased so the "dominant" scale wins at ~75/25 (instead of 50/50)
    // - Grid line RGB is "shared" (white-ish); visibility is driven by opacity weights
    // - Axis lines (X/Z) are always visible
    //
    // Scale ladder (meters per cell):
    // - 1.0  (1m)
    // - 10.0 (10m)
    // - 100.0 (100m)
    //
    // The active pair and crossfade are based on camera height above the plane.
    let base_grid_scale = grid_settings.scale;
    let plane_local_coords_scaled = plane_local_coords_meters * base_grid_scale; // 1.0 == 1 meter when scale=1

    // Step 6: Axis lines (always visible)
    // ----------------------------------
    // Axis lines are special: we always show them regardless of grid scale.
    // They help you orient yourself (X is red, Z is blue).
    //
    // How do we draw a line at x=0 or z=0?
    // - We look at the absolute coordinate value: abs(coord_m)
    // - We use fwidth(...) to anti-alias: fwidth gives an approximate pixel-width
    //   of the coordinate change across the screen. Dividing by fwidth makes the
    //   line have a roughly constant thickness in screen space.
    //
    // axis_metric becomes small near the axis, large away from it.
    // axis_line is the minimum distance to either axis.
    // Axis lines are computed in the "1 meter" coordinate system.
    // We use fwidth(...) to make the axis thickness stable in screen space.
    let axis_fwidth = fwidth(plane_local_coords_scaled);
    let axis_distance_metric = abs(plane_local_coords_scaled) / axis_fwidth;
    let axis_distance_to_nearest_axis = min(axis_distance_metric.x, axis_distance_metric.y);
    let axis_alpha_coverage = clamp(1.0 - min(axis_distance_to_nearest_axis, 1.0), 0.0, 1.0) * grid_settings.axis_alpha;

    // Choose which axis color (X or Z) based on which axis we're closer to.
    let axis_color = mix(
        grid_settings.x_axis_color,
        grid_settings.z_axis_color,
        step(axis_distance_metric.x, axis_distance_metric.y)
    );

    // Step 7: Select grid layers based on camera height
    // -------------------------------------------------
    // We use camera height above the plane as a simple proxy for "zoom level".
    // Close to the plane: show detail (1m).
    // Far from the plane: show context (100m).
    //
    // The 10m grid is always shown as an "anchor" layer for readability.
    let camera_height_above_plane_meters = abs(view.world_position.y - intersection_world_pos.y);

    // Layer selection outputs:
    // - `primary_*`  : always-on layer (10m)
    // - `secondary_*`: fades in/out (1m when close, 100m when far)
    var primary_layer_weight = 0.0;
    var secondary_layer_weight = 0.0;
    var primary_cell_size_meters = 10.0;
    var secondary_cell_size_meters = 1.0;

    // Tuning knobs:
    // - `smoothstep(a,b,h)` returns 0..1 as `h` moves from `a` to `b`.
    // - `sq` reduces the "both equally visible" look around the midpoint.
    //
    // Note: weights do not need to sum to 1.0 because 10m is intentionally always present.
    if camera_height_above_plane_meters < 120.0 {
        // Close: fade OUT 1m detail as you move away from the plane.
        let fade_out_1m = sq(smoothstep(2.0, 120.0, camera_height_above_plane_meters));
        primary_layer_weight = 1.0;             // 10m always on
        secondary_layer_weight = 1.0 - fade_out_1m;
        primary_cell_size_meters = 10.0;
        secondary_cell_size_meters = 1.0;
    } else {
        // Far: fade IN 100m context as you move away from the plane.
        let fade_in_100m = sq(smoothstep(120.0, 600.0, camera_height_above_plane_meters));
        primary_layer_weight = 1.0;             // 10m always on
        secondary_layer_weight = fade_in_100m;
        primary_cell_size_meters = 10.0;
        secondary_cell_size_meters = 100.0;
    }

    // Step 8: Procedurally evaluate grid lines (anti-aliased)
    // ------------------------------------------------------
    // This is the heart of "procedural grid" rendering.
    //
    // Goal:
    // - Given a 2D coordinate (coord_m), decide if we're close to a grid line.
    //
    // How grid lines are detected:
    // - A grid line occurs at integer boundaries.
    // - If we want "cell size = cell_a meters", we scale coordinates so that:
    //     coord_a = coord_m / cell_a
    //   Now, when coord_a is an integer, we are exactly on a grid line.
    //
    // How we get a smooth line instead of flickering aliasing:
    // - fwidth(coord_a) approximates how much coord_a changes across one pixel.
    // - Dividing by fwidth effectively converts "distance in coord space" into
    //   "distance in pixel space", producing stable thickness with distance.
    //
    // The expression:
    //   abs(fract(coord - 0.5) - 0.5)
    // yields the distance to the nearest integer grid line, centered around 0.
    //
    // Finally, we compute a coverage-like alpha:
    //   1.0 near the line, 0.0 away from it.
    // Evaluate the two active layers using the same procedure, just different cell sizes.
    // Terminology:
    // - `grid_coords`: plane coords expressed in "cells" (meters / cell_size)
    // - `distance_to_line`: ~0 near a line, ~1 away (normalized by `fwidth` for AA)
    // - `coverage`: 0..1 line coverage for this pixel
    let primary_grid_coords = plane_local_coords_scaled / primary_cell_size_meters;
    let primary_grid_coords_fwidth = fwidth(primary_grid_coords);
    let primary_distance_to_line = abs(fract(primary_grid_coords - 0.5) - 0.5) / primary_grid_coords_fwidth;
    let primary_nearest_line_metric = min(primary_distance_to_line.x, primary_distance_to_line.y);
    let primary_layer_coverage = clamp(1.0 - min(primary_nearest_line_metric, 1.0), 0.0, 1.0) * primary_layer_weight;

    let secondary_grid_coords = plane_local_coords_scaled / secondary_cell_size_meters;
    let secondary_grid_coords_fwidth = fwidth(secondary_grid_coords);
    let secondary_distance_to_line = abs(fract(secondary_grid_coords - 0.5) - 0.5) / secondary_grid_coords_fwidth;
    let secondary_nearest_line_metric = min(secondary_distance_to_line.x, secondary_distance_to_line.y);
    let secondary_layer_coverage = clamp(1.0 - min(secondary_nearest_line_metric, 1.0), 0.0, 1.0) * secondary_layer_weight;

    // Step 9: Apply style knobs (base alpha + per-scale brightness)
    // ------------------------------------------------------------
    // At this point:
    // - a_a and a_b are "coverage values" in [0..1], already weighted by w_a/w_b.
    //
    // Now we apply:
    // 1) Base alpha from grid_settings.grid_line_color.a
    // 2) Brightness multiplier depending on which scale the layer represents.
    //
    // Important: We apply brightness by scaling ALPHA, not RGB.
    // That keeps the grid color consistent and makes the grid appear stronger/weaker
    // in a physically intuitive way (opacity).
    //
    // If the grid is too faint overall:
    // - Increase grid_settings.grid_line_color.a on the CPU side
    // - Or increase the constants returned by grid_brightness(...)
    let primary_brightness = grid_brightness(primary_cell_size_meters);
    let secondary_brightness = grid_brightness(secondary_cell_size_meters);

    let primary_layer_alpha =
        primary_layer_coverage * grid_settings.grid_line_color.a * primary_brightness;

    let secondary_layer_alpha =
        secondary_layer_coverage * grid_settings.grid_line_color.a * secondary_brightness;

    // Step 10: Fade out the grid to avoid harsh edges / aliasing
    // ---------------------------------------------------------
    // We apply two fades:
    //
    // A) Distance fade (dist_fade)
    // - As the plane intersection point gets farther away in view space,
    //   the grid fades toward 0.
    //
    // This uses the reciprocal constant:
    //   dist_fadeout_const = 1 / fadeout_distance
    //
    // B) Angle fade (dot_fade)
    // - At very shallow angles, grid patterns can get noisy.
    // - We compute how aligned the camera-to-point vector is with the plane normal.
    //
    // dot_fade is 1 when looking straight down at the plane,
    // and approaches 0 when looking along the plane.
    let dist_fade = min(1.0, 1.0 - grid_settings.dist_fadeout_const * view_space_depth);
    let dot_fade = abs(dot(plane_normal_world, normalize(view.world_position - intersection_world_pos)));
    let fade = mix(dist_fade, 1.0, dot_fade) * min(grid_settings.dot_fadeout_const * dot_fade, 1.0);

    // Step 11: Composite axis + grid + fade into final output
    // ------------------------------------------------------
    // The axis lines should visually "win" over the grid lines.
    // So we mask grid alpha where the axis is present:
    //   grid_masked = grid_alpha * (1 - axis_alpha)
    let axis_mask = clamp(axis_alpha_coverage, 0.0, 1.0);
    let primary_layer_alpha_masked_by_axis = primary_layer_alpha * (1.0 - axis_mask);
    let secondary_layer_alpha_masked_by_axis = secondary_layer_alpha * (1.0 - axis_mask);

    // Final alpha is the sum of contributors, then faded by distance/angle.
    let alpha_out = clamp(
        (axis_alpha_coverage + primary_layer_alpha_masked_by_axis + secondary_layer_alpha_masked_by_axis) * fade,
        0.0,
        1.0
    );

    // RGB is "pre-multiplied-ish": each term is already scaled by its coverage/alpha.
    let rgb =
        axis_color * axis_alpha_coverage +
        grid_settings.grid_line_color.rgb * primary_layer_alpha_masked_by_axis +
        grid_settings.grid_line_color.rgb * secondary_layer_alpha_masked_by_axis;

    // Final output:
    // - out.depth: custom depth so the grid is depth-tested like real geometry
    // - out.color: premultiplied-ish composition (RGB already scaled by alpha contributors)
    //
    // Note: The render pipeline uses alpha blending, so alpha_out controls how strongly
    // the grid overlays the background.
    var out: FragmentOutput;
    out.depth = clip_depth;
    out.color = vec4<f32>(rgb, alpha_out);
    return out;
 }

 // Crossfade bias helper
 // ---------------------
 // A plain crossfade uses weights (1 - t) and t, which yields a 50/50 blend at
 // t=0.5. For grids this often looks too "busy" because two line sets are
 // equally strong.
 //
 // We bias the blend so one layer stays dominant longer:
 //   t' = t^2
 // This keeps endpoints (0 and 1) unchanged but shifts the midpoint:
 //   t=0.5 -> t'=0.25  (75/25 instead of 50/50)
fn sq(t: f32) -> f32 {
    return t * t;
}

// Per-scale alpha multiplier
// --------------------------
// Multiplies the *alpha* contribution of a layer based on its cell size.
// This is purely an artistic tuning knob (not physically based).
//
// Note: This is separate from "which layers are active" (weights). We use:
// - weights: decide when a layer should appear/disappear (based on camera height)
// - brightness: relative strength between 1m/10m/100m when they are present
//
// Anchored-grid intent:
// - 10m should read clearly as the always-on baseline.
// - 1m is detail (can be weaker).
// - 100m is context (can be weaker than 10m but still readable when enabled).
fn grid_brightness(cell_m: f32) -> f32 {
    if cell_m <= 1.0 {
        return 0.45;
    } else if cell_m <= 10.0 {
        return 0.6;
    } else {
        return 0.75;
    }
}

// Clip -> world helper used for ray reconstruction
// ------------------------------------------------
// Given a clip-space position (x,y in [-1,1], z chosen by us), return the
// corresponding world-space position.
//
// We use two z values later ("near-ish" and "far-ish") to form a ray
// direction per pixel.
//
// Note: The specific matrix order here matches what Bevy provides for this
// pipeline (and what bevy_infinite_grid uses).
fn unproject_point(clip_xyz: vec3<f32>) -> vec3<f32> {
    let unprojected = view.view * view.inverse_projection * vec4<f32>(clip_xyz, 1.0);
    return unprojected.xyz / unprojected.w;
}
