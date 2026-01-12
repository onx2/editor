// Infinite grid shader (WGSL)
// Inspired by bevy_infinite_grid, adapted for editor usage in this repo.
//
// -----------------------------------------------------------------------------
// BEGINNER-FRIENDLY OVERVIEW
// -----------------------------------------------------------------------------
//
// What this shader draws
// ----------------------
// This shader draws an "infinite" ground grid (like Unreal/Unity editors) on a
// plane (usually the XZ plane at y=0). The grid is "infinite" because we do NOT
// build a big mesh. Instead, we render a full-screen quad and, for each pixel,
// we compute where that pixel's view-ray intersects the grid plane.
//
// In other words:
//   - Vertex shader: draws a fullscreen quad (4 vertices) and prepares two world
//     points per pixel (near + far) so the fragment shader can build a ray.
//   - Fragment shader: does ray-plane intersection to find the world position
//     on the plane for this pixel, then procedurally evaluates grid lines.
//
// Why fullscreen quad?
// --------------------
// Rendering a big mesh grid leads to:
//   - huge meshes or tiling
//   - aliasing (shimmering) when far away
//   - many triangles
//
// This approach is:
//   - constant geometry cost (always 4 vertices)
//   - crisp lines via GPU derivative-based anti-aliasing (fwidth)
//   - naturally "infinite" (limited only by depth/fade settings)
//
// Coordinate spaces (important mental model)
// ------------------------------------------
// You'll see several spaces used here:
//
// 1) Clip space (post-projection)
//    - Coordinates in range x,y ∈ [-1, 1] after projection.
//    - We render the quad directly in clip space.
// 2) View space (camera space)
//    - Camera at origin, looking down -Z in conventional setups.
// 3) World space
//    - Your actual scene coordinates (Bevy units; we treat 1.0 as 1 meter).
//
// Bind groups / uniforms (how CPU talks to the shader)
// ----------------------------------------------------
// The Rust side sets up two bind groups:
//
// @group(0) @binding(0): View uniform
//   - projection, inverse_projection
//   - view, inverse_view
//   - camera world_position
//
// @group(1) @binding(0): GridPlane uniform
//   - plane origin, plane normal
//   - planar rotation matrix used to get stable 2D plane coordinates
//
// @group(1) @binding(1): GridSettings uniform
//   - scale and fade constants
//   - axis colors
//   - base grid color/alpha and axis alpha multiplier
//
// Entry points
// ------------
// - `@vertex fn vertex(...)`
// - `@fragment fn fragment(...)`
//
// Naming conventions used in this file
// ------------------------------------
// This file avoids single-letter variable names (like r/o/n/t/d) except in
// mathematical formulas shown in comments. In code, we use descriptive names:
// - ray_origin_world, ray_direction_world
// - plane_normal_world, plane_origin_world
// - intersection_world_pos
// - plane_local_coords_meters
// - camera_height_above_plane_meters
// - active_cell_size_a_meters, active_cell_size_b_meters
// - active_weight_a, active_weight_b
//
// -----------------------------------------------------------------------------
// END BEGINNER-FRIENDLY OVERVIEW
// -----------------------------------------------------------------------------

struct GridPlane {
    // planar_rotation_matrix:
    // -----------------------
    // We intersect the view ray with the plane to get a 3D point in world space.
    // To draw a grid, we want a stable 2D coordinate system on that plane.
    //
    // This matrix rotates a world-space offset on the plane into "grid local"
    // space. After applying it, we use .xz as our 2D coordinates.
    //
    // In the common case (grid is XZ plane at y=0), this ends up being close to
    // identity, but we keep it generic so the grid plane can be rotated.
    planar_rotation_matrix: mat3x3<f32>,

    // origin:
    // -------
    // A point on the plane in world space. For a y=0 ground plane, origin is
    // typically (0,0,0).
    origin: vec3<f32>,

    // normal:
    // -------
    // Plane normal (unit vector) in world space. For the XZ plane (y=0), this
    // is typically (0,1,0).
    normal: vec3<f32>,
};

struct GridSettings {
    // scale:
    // ------
    // Think of this as the "base scale" applied to plane coordinates before we
    // evaluate grid lines. In this project we want 1 Bevy unit == 1 meter, so
    // the CPU typically sets this to 1.0.
    //
    // If you set scale higher, the grid becomes denser (more lines per meter).
    scale: f32,

    // dist_fadeout_const:
    // -------------------
    // This is stored as 1.0 / fadeout_distance on the CPU so the shader can
    // multiply instead of divide (cheaper).
    //
    // Used to fade the grid as it recedes away from the camera.
    dist_fadeout_const: f32,

    // dot_fadeout_const:
    // ------------------
    // Another constant stored as a reciprocal on the CPU.
    //
    // Used to reduce aliasing when the viewing angle is very shallow relative
    // to the plane (grazing angles). The closer you look along the plane, the
    // more we fade (or soften) to avoid noisy patterns.
    dot_fadeout_const: f32,

    // Axis colors:
    // ------------
    // These are the colored axis lines you see in editors:
    // - X axis: typically red
    // - Z axis: typically blue
    //
    // These colors are always shown (independent of grid scale selection).
    x_axis_color: vec3<f32>,
    z_axis_color: vec3<f32>,

    // Base grid line color + alpha.
    //
    // We still treat this as the "shared" base color, but we additionally apply a per-scale
    // brightness multiplier in the shader so that:
    //   1m < 10m < 100m
    // (increasing brightness with increasing cell size).
    // grid_line_color:
    // ----------------
    // Base grid line color used for ALL grid scales (1m/10m/100m).
    //
    // - RGB: the shared color (usually white-ish or light gray)
    // - A:   base alpha before we apply:
    //        - scale weights (based on camera height)
    //        - per-scale brightness multiplier
    //
    // Per-scale brightness:
    // ---------------------
    // Even though RGB is shared, we make larger cell sizes visually brighter
    // by multiplying alpha with a per-scale factor:
    //   1m < 10m < 100m
    grid_line_color: vec4<f32>,

    // Axis line opacity multiplier (axis RGB comes from x_axis_color/z_axis_color).
    // axis_alpha:
    // -----------
    // Multiplies axis line opacity (axis RGB comes from x_axis_color/z_axis_color).
    // Axis lines are always visible; this controls how strong they are.
    axis_alpha: f32,
 };

struct View {
    // projection:
    // -----------
    // Transforms from view space -> clip space.
    //
    // Clip space is what the GPU rasterizer uses to decide where pixels land
    // on screen.
    projection: mat4x4<f32>,

    // inverse_projection:
    // -------------------
    // Transforms from clip space -> view space (inverse of projection).
    // Used for ray reconstruction / unprojection.
    inverse_projection: mat4x4<f32>,

    // view:
    // -----
    // In this shader, `view` is the matrix used to bring points into view space.
    // (Bevy provides matrices in a specific convention; we follow what worked in
    // bevy_infinite_grid.)
    view: mat4x4<f32>,

    // inverse_view:
    // -------------
    // Inverse of `view`. Lets us convert from view space -> world space.
    inverse_view: mat4x4<f32>,

    // world_position:
    // ---------------
    // Camera position in world space (Vec3).
    // Used for distance-based fading and angle-based fading.
    world_position: vec3<f32>,
};

fn bias_75_25(t: f32) -> f32 {
    // Crossfade bias helper
    // ---------------------
    // We frequently crossfade between two grid scales (example: 1m and 10m).
    // A standard crossfade uses:
    //   w_a = 1 - t
    //   w_b = t
    // where t goes 0..1 as you move through the transition band.
    //
    // That gives a 50/50 blend at t=0.5, which can look "busy" because both
    // grids are equally strong.
    //
    // This function biases t so the currently-dominant grid stays dominant
    // longer. Squaring is a simple bias:
    //   t' = t^2
    // so:
    //   t=0.5 -> t'=0.25  (=> 75/25 instead of 50/50)
    //
    // Note: This does not change endpoints: 0 stays 0 and 1 stays 1.
    //
    // If you want the opposite bias (favor the larger scale earlier), you can
    // use something like: t' = 1 - (1 - t)^2
    return t * t;
 }

fn grid_brightness(cell_m: f32) -> f32 {
    // Per-scale brightness helper
    // ---------------------------
    // We want larger grid cells to be visually stronger so that when you zoom
    // out (camera higher above the plane), the coarser grid remains readable.
    //
    // This function returns a multiplier applied to *alpha* (opacity).
    //
    // Why alpha and not RGB?
    // - Because we want a "shared" grid color but different visibility.
    // - Alpha-based changes preserve the hue (white/gray stays white/gray).
    //
    // cell_m is "meters per cell" (cell size):
    // - 1.0  = 1m
    // - 10.0 = 10m
    // - 100.0 = 100m
    //
    // These constants are artistic knobs. Increase them if the grid is too faint.
    if cell_m <= 1.0 {
        return 0.55;
    } else if cell_m <= 10.0 {
        return 0.75;
    } else {
        return 0.85;
    }
 }

@group(0) @binding(0) var<uniform> view: View;

@group(1) @binding(0) var<uniform> grid_plane: GridPlane;
@group(1) @binding(1) var<uniform> grid_settings: GridSettings;

struct VertexInput {
    @builtin(vertex_index) index: u32,
};

fn unproject_point(clip_xyz: vec3<f32>) -> vec3<f32> {
    // Unprojection / ray reconstruction helper
    // ----------------------------------------
    // clip_xyz is in clip space:
    //   x, y in [-1, 1] cover the viewport
    //   z is a chosen depth value (we'll use 1.0-ish for near and 0.001-ish for far)
    //
    // We want to reconstruct a world-space ray for each pixel.
    // A common way:
    //   1) Convert from clip space back into view space using inverse_projection.
    //   2) Convert from view space into world space.
    //
    // In this codebase we follow the conventions from bevy_infinite_grid where:
    //   - view.view and view.inverse_projection are arranged to produce a stable world point.
    //
    // After multiplying, we divide by w to get a proper 3D point (perspective divide).
    let unprojected = view.view * view.inverse_projection * vec4<f32>(clip_xyz, 1.0);
    return unprojected.xyz / unprojected.w;
 }

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
};

@vertex
fn vertex(vertex_input: VertexInput) -> VertexOutput {
    // Vertex shader: fullscreen quad + per-pixel ray endpoints
    // --------------------------------------------------------
    //
    // We render exactly 4 vertices using a triangle strip. The geometry is a
    // fullscreen quad expressed directly in CLIP SPACE, so it covers the whole
    // screen regardless of camera.
    //
    // Fullscreen quad corners (triangle strip order):
    //   0: (-1,-1) bottom-left
    //   1: (-1, 1) top-left
    //   2: ( 1,-1) bottom-right
    //   3: ( 1, 1) top-right
    //
    // Clip space Z:
    // -------------
    // We set z=1.0 for the base point we rasterize, and we unproject a second
    // point at a smaller z (0.001) to get a stable "far-ish" point. Together,
    // these two points define a world-space ray for the current pixel.
    var clip_space_corners = array<vec3<f32>, 4>(
        vec3<f32>(-1.0, -1.0, 1.0),
        vec3<f32>(-1.0,  1.0, 1.0),
        vec3<f32>( 1.0, -1.0, 1.0),
        vec3<f32>( 1.0,  1.0, 1.0),
    );

    let clip_space_corner_xyz = clip_space_corners[vertex_input.index];

    var vertex_output: VertexOutput;

    // This is the actual position the GPU uses to rasterize the quad.
    vertex_output.clip_position = vec4<f32>(clip_space_corner_xyz, 1.0);

    // Compute two world-space points for this pixel:
    // - near_point_world: unproject at z=1.0 (stable near endpoint)
    // - far_point_world:  unproject at z=0.001 (stable far-ish endpoint)
    //
    // In the fragment shader we build:
    //   ray_origin_world = near_point_world
    //   ray_direction_world = normalize(far_point_world - near_point_world)
    vertex_output.near_point = unproject_point(clip_space_corner_xyz);

    // 0.001 is a stable "far-ish" depth value used by bevy_infinite_grid. It does not
    // literally correspond to the camera's far plane; it is simply used to derive a
    // direction vector that points outward through the pixel.
    vertex_output.far_point = unproject_point(vec3<f32>(clip_space_corner_xyz.xy, 0.001));
    return vertex_output;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fragment(vertex_output: VertexOutput) -> FragmentOutput {
    // Fragment shader: ray-plane intersection + procedural grid
    // ---------------------------------------------------------
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

    // Step 5: Compute depth
    // ---------------------
    // We output custom fragment depth so the grid participates in the depth buffer
    // like real geometry (so objects can occlude the grid).
    //
    // Bevy 3D uses reversed-Z: larger depth values are closer.
    // The render pipeline uses CompareFunction::Greater accordingly.
    // Step 5: Compute depth for the grid plane intersection
    // -----------------------------------------------------
    // We output a custom depth so the grid participates in depth-testing like a real mesh.
    //
    // Process:
    // - Convert the world-space intersection into view space.
    // - Project it into clip space.
    // - Convert to normalized clip depth (z / w).
    //
    // Bevy 3D uses reversed-Z by default, and the pipeline is configured accordingly.
    let intersection_view_space = view.inverse_view * vec4<f32>(intersection_world_pos, 1.0);
    let intersection_clip_space = view.projection * intersection_view_space;
    let clip_depth = intersection_clip_space.z / intersection_clip_space.w;

    // real_depth is distance along the view direction (used for distance fading).
    let real_depth = -intersection_view_space.z;

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

    // Step 7: Choose which grid scale(s) to show based on camera height
    // ---------------------------------------------------------------
    // We use camera distance to the plane as a simple "zoom level" proxy.
    // For the ground plane at y=0, this is approximately abs(camera_y).
    //
    // Why this works:
    // - When you're close to the plane, you want the standard 1m grid.
    // - When you're far away, the 1m grid becomes too dense/noisy, so we switch to coarser grids.
    // Camera height above the plane is our "zoom level" proxy.
    // For a y=0 plane, this is approximately abs(camera_y).
    let camera_height_above_plane_meters = abs(view.world_position.y - intersection_world_pos.y);

    // Transition bands (tunable artistic knobs):
    // ------------------------------------------
    // We crossfade between adjacent scales across wide height ranges:
    //   1m  -> 10m   between 5m and 90m
    //   10m -> 100m  between 40m and 600m
    //
    // Only TWO scales are active at any time (the pair we're currently crossfading).
    // That reduces visual clutter.
    //
    // active_weight_a / active_weight_b are the crossfade weights for the two active scales.
    // active_cell_size_a_meters / active_cell_size_b_meters are the corresponding cell sizes.
    var active_weight_a = 0.0;
    var active_weight_b = 0.0;
    var active_cell_size_a_meters = 1.0;
    var active_cell_size_b_meters = 10.0;

    // Pick the active pair and weights:
    // -------------------------------
    // We pick (cell_a, cell_b) as adjacent scales and compute weights (w_a, w_b).
    //
    // `smoothstep(edge0, edge1, h)` returns a smooth 0..1 transition:
    // - 0 when h <= edge0
    // - 1 when h >= edge1
    // - smooth curve in between
    //
    // Then we bias it with bias_75_25 to avoid a 50/50 blend (which looks too busy).
    if camera_height_above_plane_meters < 90.0 {
        // Active pair: 1m -> 10m
        let transition_1m_to_10m = bias_75_25(smoothstep(5.0, 90.0, camera_height_above_plane_meters));
        active_weight_a = 1.0 - transition_1m_to_10m; // 1m
        active_weight_b = transition_1m_to_10m;       // 10m
        active_cell_size_a_meters = 1.0;
        active_cell_size_b_meters = 10.0;
    } else {
        // Active pair: 10m -> 100m
        let transition_10m_to_100m = bias_75_25(smoothstep(40.0, 600.0, camera_height_above_plane_meters));
        active_weight_a = 1.0 - transition_10m_to_100m; // 10m
        active_weight_b = transition_10m_to_100m;       // 100m
        active_cell_size_a_meters = 10.0;
        active_cell_size_b_meters = 100.0;
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
    let grid_coords_a = plane_local_coords_scaled / active_cell_size_a_meters;
    let grid_coords_a_fwidth = fwidth(grid_coords_a);
    let grid_coords_a_distance_to_line = abs(fract(grid_coords_a - 0.5) - 0.5) / grid_coords_a_fwidth;
    let grid_coords_a_nearest_line_metric = min(grid_coords_a_distance_to_line.x, grid_coords_a_distance_to_line.y);
    let grid_layer_a_coverage = clamp(1.0 - min(grid_coords_a_nearest_line_metric, 1.0), 0.0, 1.0) * active_weight_a;

    let grid_coords_b = plane_local_coords_scaled / active_cell_size_b_meters;
    let grid_coords_b_fwidth = fwidth(grid_coords_b);
    let grid_coords_b_distance_to_line = abs(fract(grid_coords_b - 0.5) - 0.5) / grid_coords_b_fwidth;
    let grid_coords_b_nearest_line_metric = min(grid_coords_b_distance_to_line.x, grid_coords_b_distance_to_line.y);
    let grid_layer_b_coverage = clamp(1.0 - min(grid_coords_b_nearest_line_metric, 1.0), 0.0, 1.0) * active_weight_b;

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
    let brightness_multiplier_a = grid_brightness(active_cell_size_a_meters);
    let brightness_multiplier_b = grid_brightness(active_cell_size_b_meters);
    let grid_layer_a_alpha = grid_layer_a_coverage * grid_settings.grid_line_color.a * brightness_multiplier_a;
    let grid_layer_b_alpha = grid_layer_b_coverage * grid_settings.grid_line_color.a * brightness_multiplier_b;

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
    let dist_fade = min(1.0, 1.0 - grid_settings.dist_fadeout_const * real_depth);
    let dot_fade = abs(dot(plane_normal_world, normalize(view.world_position - intersection_world_pos)));
    let fade = mix(dist_fade, 1.0, dot_fade) * min(grid_settings.dot_fadeout_const * dot_fade, 1.0);

    // Step 11: Composite axis + grid + fade into final output
    // ------------------------------------------------------
    // The axis lines should visually "win" over the grid lines.
    // So we mask grid alpha where the axis is present:
    //   grid_masked = grid_alpha * (1 - axis_alpha)
    let axis_mask = clamp(axis_alpha_coverage, 0.0, 1.0);
    let grid_layer_a_alpha_masked_by_axis = grid_layer_a_alpha * (1.0 - axis_mask);
    let grid_layer_b_alpha_masked_by_axis = grid_layer_b_alpha * (1.0 - axis_mask);

    // Combine alphas for output and apply fade (distance/angle).
    // clamp to [0..1] because alpha is a coverage-like value.
    let alpha_out = clamp((axis_alpha_coverage + grid_layer_a_alpha_masked_by_axis + grid_layer_b_alpha_masked_by_axis) * fade, 0.0, 1.0);

    // Compute RGB contribution:
    // - Axis uses its own RGB (red/blue), scaled by its alpha coverage
    // - Grid uses shared RGB, scaled by each layer's alpha (masked by axis)
    let rgb =
        axis_color * axis_alpha_coverage +
        grid_settings.grid_line_color.rgb * grid_layer_a_alpha_masked_by_axis +
        grid_settings.grid_line_color.rgb * grid_layer_b_alpha_masked_by_axis;

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
