// Infinite grid shader (WGSL)
// Inspired by bevy_infinite_grid, simplified for editor usage.
//
// Renders a full-screen quad (triangle strip, 4 vertices). For each fragment,
// casts a ray from the camera, intersects a plane, and procedurally computes
// grid lines with anti-aliasing via fwidth.
//
// Expected bind groups:
//
// @group(0) @binding(0): View uniform (matrices + camera world position)
// @group(1) @binding(0): GridPlane uniform (plane origin/normal + planar rotation matrix)
// @group(1) @binding(1): GridSettings uniform (colors, scale, fadeout)
//
// Entry points: `vertex`, `fragment`

struct GridPlane {
    planar_rotation_matrix: mat3x3<f32>,
    origin: vec3<f32>,
    normal: vec3<f32>,
};

struct GridSettings {
    // Grid density: world units are multiplied by `scale` prior to line eval.
    // If you think in "cell size", use: scale = 1.0 / cell_size.
    scale: f32,
    // 1.0 / fadeout_distance
    dist_fadeout_const: f32,
    // 1.0 / dot_fadeout_strength
    dot_fadeout_const: f32,

    x_axis_color: vec3<f32>,
    z_axis_color: vec3<f32>,

    // rgba; alpha is used as an intensity multiplier for the line class
    minor_line_color: vec4<f32>,
    major_line_color: vec4<f32>,
};

struct View {
    projection: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    inverse_view: mat4x4<f32>,
    world_position: vec3<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

@group(1) @binding(0) var<uniform> grid_plane: GridPlane;
@group(1) @binding(1) var<uniform> grid_settings: GridSettings;

struct VertexInput {
    @builtin(vertex_index) index: u32,
};

fn unproject_point(clip_xyz: vec3<f32>) -> vec3<f32> {
    // `clip_xyz` is in clip space (-1..1, -1..1, z).
    // We use the same unprojection trick as bevy_infinite_grid:
    // unproject into view space, then into world space.
    let unprojected = view.view * view.inverse_projection * vec4<f32>(clip_xyz, 1.0);
    return unprojected.xyz / unprojected.w;
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
};

@vertex
fn vertex(v: VertexInput) -> VertexOutput {
    // Fullscreen quad via triangle strip (4 verts).
    // Clip-space z=1 to get corners on the near plane when unprojecting using Bevy matrices.
    var corners = array<vec3<f32>, 4>(
        vec3<f32>(-1.0, -1.0, 1.0),
        vec3<f32>(-1.0,  1.0, 1.0),
        vec3<f32>( 1.0, -1.0, 1.0),
        vec3<f32>( 1.0,  1.0, 1.0),
    );

    let p = corners[v.index];

    var out: VertexOutput;
    out.clip_position = vec4<f32>(p, 1.0);

    // World-space points for ray construction.
    out.near_point = unproject_point(p);

    // Unproject a point "towards" the far plane. The 0.001 is a stable value used by the crate.
    out.far_point = unproject_point(vec3<f32>(p.xy, 0.001));
    return out;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let ro = in.near_point;
    let rd = normalize(in.far_point - in.near_point);

    let n = grid_plane.normal;
    let o = grid_plane.origin;

    // Ray-plane intersection: t = (o - ro)·n / (rd·n)
    let denom = dot(rd, n);

    // If looking near-parallel to plane, bail out transparent.
    // Depth is set to far (1.0) so it won't occlude anything.
    if abs(denom) < 1e-6 {
        return FragmentOutput(vec4<f32>(0.0), 1.0);
    }

    let t = dot(n, (o - ro)) / denom;

    // If intersection is behind the ray, don't draw.
    if t <= 0.0 {
        return FragmentOutput(vec4<f32>(0.0), 1.0);
    }

    let world_pos = ro + rd * t;

    // Compute coordinates on the plane in a stable orientation.
    let planar_offset = world_pos - o;
    let plane_coords = (grid_plane.planar_rotation_matrix * planar_offset).xz;

    // Compute depth for the plane intersection point.
    // Bevy 3D uses reversed-Z; the pipeline should use CompareFunction::Greater.
    let view_space = view.inverse_view * vec4<f32>(world_pos, 1.0);
    let clip_space = view.projection * view_space;
    let clip_depth = clip_space.z / clip_space.w;
    let real_depth = -view_space.z;

    // Procedural grid evaluation.
    let scale = grid_settings.scale;
    let coord = plane_coords * scale;

    // Minor lines (every 1 cell)
    let d = fwidth(coord);
    let g = abs(fract(coord - 0.5) - 0.5) / d;
    let minor_line = min(g.x, g.y);

    // Major lines (every 10 cells)
    let d2 = fwidth(coord * 0.1);
    let g2 = abs(fract((coord * 0.1) - 0.5) - 0.5) / d2;
    let major_line = min(g2.x, g2.y);

    // Axis lines (x/z axes in plane space)
    let g3 = abs(coord) / d;
    let axis_line = min(g3.x, g3.y);

    // Alpha for each "class" of line. Order: axis, major, minor.
    var alpha = vec3<f32>(1.0) - min(vec3<f32>(axis_line, major_line, minor_line), vec3<f32>(1.0));
    alpha.y *= (1.0 - alpha.x) * grid_settings.major_line_color.a;
    alpha.z *= (1.0 - (alpha.x + alpha.y)) * grid_settings.minor_line_color.a;

    // Fadeout with distance, and with view angle (dot fade) to soften grazing angles.
    let dist_fade = min(1.0, 1.0 - grid_settings.dist_fadeout_const * real_depth);
    let dot_fade = abs(dot(n, normalize(view.world_position - world_pos)));
    let fade = mix(dist_fade, 1.0, dot_fade) * min(grid_settings.dot_fadeout_const * dot_fade, 1.0);

    // Normalize weights so colors blend without darkening.
    let a0 = alpha.x + alpha.y + alpha.z;
    var w = alpha / max(a0, 1e-6);
    w = clamp(w, vec3<f32>(0.0), vec3<f32>(1.0));

    // Pick axis color based on which axis line we're closer to (in plane coords)
    let axis_color = mix(grid_settings.x_axis_color, grid_settings.z_axis_color, step(g3.x, g3.y));

    let rgb =
        axis_color * w.x +
        grid_settings.major_line_color.rgb * w.y +
        grid_settings.minor_line_color.rgb * w.z;

    var out: FragmentOutput;
    out.depth = clip_depth;
    out.color = vec4<f32>(rgb, max(a0 * fade, 0.0));
    return out;
}
