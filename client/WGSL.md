# WGSL Notes (for this project)

This file is a practical “what to know next” guide for working on the grid shader in this repo (and WGSL in general). It’s written with the intent that you can come back later and still understand *why* things are done the way they are.

---

## 1) Mental model: how the grid shader works

The grid is drawn as a **fullscreen quad**. There is no world-space grid mesh.

High level flow:

1. **Vertex stage** emits 4 vertices (triangle strip) in **clip space** so the fragment shader runs for every pixel.
2. Vertex stage also computes two **world-space points** per vertex by “unprojecting” the clip-space corner at two chosen depth values (a “near-ish” and “far-ish” point).
3. **Fragment stage** interpolates those points, reconstructs a **world-space ray** per pixel, and intersects it with a configured **plane**.
4. The intersection point is converted into stable **2D plane coordinates** (so the grid doesn’t swim when rotated).
5. The grid lines are computed **procedurally** using `fract` and anti-aliased using `fwidth`.

If you remember nothing else: *fullscreen quad + per-pixel ray/plane intersection + derivative-based line AA*.

---

## 2) Coordinate spaces (common source of confusion)

You’ll see these spaces used:

- **Clip space**: Post-projection coordinates. X/Y are in `[-1, 1]`. The fullscreen quad lives here.
- **View space**: Camera space. Used for depth computations and “distance from camera” style fades.
- **World space**: Scene space. The plane is defined here, the ray-plane intersection happens here.

Practical gotcha:
- When you compute fades based on distance, be explicit about *which* distance you’re using:
  - View-space depth along the camera’s forward axis is not the same as Euclidean world distance.

---

## 3) Anti-aliasing procedural lines (the key trick)

The “stable line thickness” trick uses **screen-space derivatives**:

- `fwidth(x)` ≈ `abs(ddx(x)) + abs(ddy(x))`
- It estimates how much `x` changes across a pixel.
- If you compute distance-to-line in “grid coordinate units” and divide by `fwidth`, you normalize by pixel footprint.

In the grid shader:
- Convert plane coords to **grid coords**: `grid_coords = plane_coords / cell_size`
- Distance to nearest integer line is derived from `fract(...)`.
- Normalize the distance by `fwidth(grid_coords)` to get stable coverage.

Gotchas:
- Derivatives (`fwidth`, `dpdx`, `dpdy`) are only valid in **fragment** (and some compute) contexts with uniform control flow. Avoid calling them inside diverging branches that vary per pixel.
- If `fwidth` is extremely small, you can get huge ratios and hard edges. Clamping the final coverage helps.

---

## 4) Layering multiple grid scales

The repo’s grid shader supports multiple “cell sizes” (ex: 1m / 10m / 100m).

Two important concepts:

- **Coverage**: A per-pixel 0..1 mask that says “this pixel is on the line”.
- **Weight**: A user/scale-driven multiplier that says “how visible should this layer be”.

In this project’s current behavior:
- The **10m layer is an anchor** (always visible).
- A **secondary layer** is chosen based on camera height:
  - Close: fade out 1m as you move away
  - Far: fade in 100m as you move away

Why this design works:
- It preserves a constant spatial reference (10m).
- It avoids clutter from showing all scales equally at once.

---

## 5) Alpha vs RGB scaling

The grid shader generally scales **alpha**, not RGB, when changing “strength”.

Why:
- Scaling RGB changes perceived color; scaling alpha changes visibility/opacity.
- When you have a shared grid color, alpha scaling gives more consistent results.

Blending gotcha:
- Most pipelines use straight alpha blending (`src_alpha`, `one_minus_src_alpha`), but some use premultiplied alpha. If you ever change blend state, revisit how you compute output color.
- In the grid shader, RGB is computed in a “premultiplied-ish” way (terms multiplied by coverage/alpha contributors). This is fine as long as you understand you’re composing contributions manually.

---

## 6) Depth output and “reversed Z” concerns

The shader writes `@builtin(frag_depth)` so the grid participates in depth testing like real geometry.

Important:
- Bevy commonly uses **reversed Z** in 3D pipelines (depth compare is often `Greater` rather than `Less`).
- The shader computes clip depth as `clip.z / clip.w` from the projected intersection point.

Gotchas:
- If the depth test function or depth range changes in your render pipeline, the grid might appear in front/behind incorrectly.
- If you see z-fighting or the grid “popping” through geometry, check:
  - Depth compare function
  - Whether the grid is rendered in the correct phase/pass
  - The computed intersection depth precision

---

## 7) Ray-plane intersection stability

Ray-plane intersection is:

- `t = dot(n, (plane_origin - ray_origin)) / dot(n, ray_dir)`

Gotchas:
- If `dot(n, ray_dir)` is ~0, the ray is nearly parallel => huge `t` and unstable results. Early-out with transparent output is normal.
- If `t <= 0`, the intersection is behind the camera. Early-out.

Numerical tip:
- The epsilon threshold (`1e-6`) is an artistic/engineering tradeoff; too large and you’ll incorrectly cull near-parallel rays, too small and you’ll get noisy far intersections.

---

## 8) Uniform layout and alignment (critical WGSL/host gotcha)

WGSL uniform buffer layouts follow alignment rules similar to std140-ish constraints.

Common pitfalls when updating structs:
- `vec3<f32>` has 16-byte alignment in uniforms (it pads like a vec4).
- Reordering fields can change layout requirements; match your host-side struct layout exactly.
- Prefer grouping/scalars to reduce padding, but only if the CPU side matches.

Rule of thumb:
- If you change a WGSL `struct` used in a `var<uniform>`, update the corresponding Rust uniform struct and verify alignment/padding.

---

## 9) Naming and comment style (recommended)

When writing shaders, clarity beats cleverness:

- Use suffixes for spaces: `_world`, `_view`, `_clip`
- Use suffixes for meaning: `_meters`, `_scaled`, `_weight`, `_coverage`
- Keep comments focused on:
  - What the block computes
  - Why the formula is shaped that way (esp. non-obvious math like `fract` tricks)
  - Constraints/gotchas (derivatives, depth conventions)

Avoid:
- Long “tutorial” paragraphs inline with code once you understand it; prefer a short intent comment and keep deep notes in files like this one.

---

## 10) Pointers / references

WGSL / WebGPU specs:
- WGSL spec: https://www.w3.org/TR/WGSL/
- WebGPU spec: https://www.w3.org/TR/webgpu/

Derivatives:
- `dpdx`, `dpdy`, `fwidth` in WGSL: see the WGSL spec “Derivative Built-in Functions”.

Procedural grid references / ideas:
- The classic “infinite grid” technique is used in many engines (often as “grid plane in screen space”).
- Search terms worth using:
  - “ray plane intersection fullscreen quad grid shader”
  - “fwidth anti aliasing grid lines”
  - “procedural grid fract fwidth”

Bevy-specific context:
- Bevy’s render pipeline and depth conventions (including reversed-Z) are important when you output `frag_depth`. If the grid behavior changes after a Bevy upgrade, revisit those conventions first.

---

## 11) Practical debugging tips (what I do)

- Add temporary output modes:
  - Output `vec4(grid_coords fract stuff)` as color to visualize patterns.
  - Output `coverage` as grayscale to see line thickness/AA.
- Clamp aggressively while debugging:
  - Clamp intermediate values to see if you’re blowing up due to small `fwidth`.
- Move math out of branches when derivatives are involved:
  - If you suspect derivative issues, compute values unconditionally and select with `mix`/`select`.

---

## 12) “Safe edit checklist” for the grid shader

Before you ship a shader edit:

1. Did you change any uniform structs? If yes:
   - Update host-side struct + alignment/padding.
2. Did you add derivatives (`fwidth`) inside non-uniform control flow?
3. Did you change depth math? Verify the grid:
   - Does it occlude correctly?
   - Is it occluded correctly?
4. Check scale selection at multiple heights:
   - Close (detail visible)
   - Mid (anchor readable)
   - Far (context visible)