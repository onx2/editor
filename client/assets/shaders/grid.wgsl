#import bevy_pbr::forward_io::VertexOutput

// IMPORTANT (Bevy 0.17):
// Do NOT define your own `Material` struct or manual @group/@binding layout.
// Bevy generates the actual bind group layout for your Rust `AsBindGroup` material.
//
// You need BOTH:
// - the generated `Material` type
// - the generated `material` binding instance
//
// Otherwise you'll get: "no definition in scope for identifier: `material`".
#import bevy_pbr::material::{Material, material}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // World-space infinite grid on the XZ plane.
    // NOTE: field names here MUST match what Bevy generates for your Rust material.
    // If your Rust struct fields are:
    //   color: LinearRgba
    //   grid_scale: f32
    //   line_width: f32
    //
    // then Bevy's generated `Material` should expose:
    //   material.color
    //   material.grid_scale
    //   material.line_width
    //
    // If you changed the Rust side (e.g. wrapped uniforms in a nested struct),
    // these field names will differ.

    let scale = max(material.grid_scale, 1e-6);
    let coord = in.world_position.xz / scale;

    // Distance to nearest grid line in cell space (0 at line center)
    let cell = abs(fract(coord) - vec2<f32>(0.5, 0.5));

    // Derivative-based anti-aliasing in "pixel-ish" distance
    let fw = max(fwidth(coord), vec2<f32>(1e-6, 1e-6));
    let d = cell / fw;

    let dist_to_line = min(d.x, d.y);
    let thickness = max(material.line_width, 1e-6);

    // Alpha is 1 at the line center, falling off away from the line
    let a = 1.0 - smoothstep(thickness, thickness + 1.0, dist_to_line);

    let out = vec4<f32>(material.color.rgb, material.color.a * a);

    // Skip writing fully transparent fragments
    if (out.a <= 0.001) {
        discard;
    }

    return out;
}
