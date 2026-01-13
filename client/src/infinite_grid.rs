use bevy::{
    asset::uuid_handle,
    camera::visibility::{self, NoFrustumCulling, VisibilityClass},
    core_pipeline::core_3d::Transparent3d,
    ecs::{
        query::ROQueryItem,
        system::SystemParamItem,
        system::lifetimeless::{Read, SRes},
    },
    image::BevyDefault,
    pbr::MeshPipelineKey,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
            RenderCommandResult, SetItemPipeline, ViewSortedRenderPhases,
        },
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, BlendState,
            ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState,
            DynamicUniformBuffer, FragmentState, MultisampleState, PipelineCache, PolygonMode,
            PrimitiveState, RenderPipelineDescriptor, ShaderStages, ShaderType,
            SpecializedRenderPipeline, SpecializedRenderPipelines, StencilFaceState, StencilState,
            TextureFormat, VertexState, binding_types::uniform_buffer,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::{RenderEntity, SyncToRenderWorld},
        view::{ExtractedView, RenderVisibleEntities, ViewTarget},
    },
};
use std::borrow::Cow;

#[derive(Resource, Copy, Clone, Debug)]
pub struct InfiniteGridEnabled(pub bool);

impl Default for InfiniteGridEnabled {
    fn default() -> Self {
        Self(true)
    }
}

const GRID_SHADER_HANDLE: Handle<Shader> = uuid_handle!("c7f0c7a8-03a2-4c25-9b31-17c7f02b99b7");
const GRID_SHADER_ASSET_PATH: &str = "infinite_grid.wgsl";
const X_AXIS_COLOR: Color = Color::srgb(1.0, 0.2, 0.2);
const Z_AXIS_COLOR: Color = Color::srgb(0.2, 0.2, 1.0);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<InfiniteGridEnabled>();
    app.add_plugins(InfiniteGridPlugin);
    app.add_systems(Startup, (spawn_infinite_grid, spawn_grid_scale_overlay));
    app.add_systems(Update, (toggle_grid_hotkey, update_grid_scale_overlay));
}

fn spawn_infinite_grid(mut commands: Commands) {
    commands.spawn(InfiniteGridBundle::default());
}

fn toggle_grid_hotkey(keys: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<InfiniteGridEnabled>) {
    if keys.just_pressed(KeyCode::KeyG) {
        enabled.0 = !enabled.0;
    }
}

struct InfiniteGridPlugin;
impl Plugin for InfiniteGridPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        // Register the shader under a stable handle.
        let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(
            GRID_SHADER_HANDLE.id(),
            Shader::from_wgsl(
                include_str!("../assets/shaders/infinite_grid.wgsl"),
                GRID_SHADER_ASSET_PATH,
            ),
        );

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<InfiniteGridEnabled>()
            .init_resource::<GridViewUniforms>()
            .init_resource::<GridPlaneUniforms>()
            .init_resource::<GridDisplaySettingsUniforms>()
            .init_resource::<InfiniteGridPipeline>()
            .init_resource::<SpecializedRenderPipelines<InfiniteGridPipeline>>()
            .add_render_command::<Transparent3d, DrawInfiniteGrid>()
            .add_systems(
                ExtractSchedule,
                (extract_infinite_grid_enabled, extract_grids),
            )
            .add_systems(
                Render,
                (prepare_plane_and_settings_uniforms, prepare_view_uniforms)
                    .in_set(RenderSystems::PrepareResources),
            )
            .add_systems(
                Render,
                (prepare_grid_bind_group, prepare_view_bind_groups)
                    .in_set(RenderSystems::PrepareBindGroups),
            )
            .add_systems(Render, queue_grids.in_set(RenderSystems::Queue));
    }
}

/// Marker component for a grid entity.
#[derive(Component, Default)]
pub struct InfiniteGrid;

#[derive(Component, Copy, Clone)]
#[require(VisibilityClass)]
#[component(on_add = visibility::add_visibility_class::<InfiniteGridSettings>)]
pub struct InfiniteGridSettings {
    /// How far the grid fades out as it recedes from the camera (in meters).
    pub fadeout_distance: f32,
    /// Minor line alpha (0..1).
    pub minor_alpha: f32,
    /// Major line alpha (0..1).
    pub major_alpha: f32,
}

impl Default for InfiniteGridSettings {
    fn default() -> Self {
        Self {
            fadeout_distance: 200.0,
            minor_alpha: 0.55,
            major_alpha: 0.85,
        }
    }
}

/// By default it renders as the XZ plane through the entity's transform.
/// If you want it at y=0, use the default transform (identity).
#[derive(Bundle, Default)]
pub struct InfiniteGridBundle {
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub settings: InfiniteGridSettings,
    pub grid: InfiniteGrid,
    pub visibility: Visibility,
    pub view_visibility: ViewVisibility,
    pub inherited_visibility: InheritedVisibility,
    pub visible_entities: RenderVisibleEntities,
    pub no_frustum_culling: NoFrustumCulling,
    pub sync_to_render_world: SyncToRenderWorld,
}

// ----------------------
// Extracted components
// ----------------------
#[derive(Component)]
struct ExtractedGrid {
    transform: GlobalTransform,
    settings: InfiniteGridSettings,
}

#[derive(Component)]
struct GridUniformOffsets {
    plane_offset: u32,
    settings_offset: u32,
}

#[derive(Component)]
pub struct GridViewUniformOffset {
    pub offset: u32,
}

#[derive(Component)]
struct GridViewBindGroup {
    value: BindGroup,
}

#[derive(Resource)]
struct GridBindGroup {
    value: BindGroup,
}

// ----------------------
// Uniforms
// ----------------------
#[derive(Debug, ShaderType)]
struct GridPlaneUniform {
    /// Rotation matrix that maps world-space planar offsets onto a canonical plane.
    /// (We use only xz afterwards.)
    planar_rotation_matrix: Mat3,
    origin: Vec3,
    normal: Vec3,
}

#[derive(Debug, ShaderType)]
struct GridDisplaySettingsUniform {
    // WGSL expects this exact layout (see `assets/shaders/infinite_grid.wgsl`):
    // struct GridSettings {
    //   scale: f32,
    //   dist_fadeout_const: f32,
    //   dot_fadeout_const: f32,
    //   x_axis_color: vec3,
    //   z_axis_color: vec3,
    //   grid_line_color: vec4,
    //   axis_alpha: f32,
    // }
    //
    // If this struct's fields don't match the WGSL struct, the shader will read garbage
    // and the grid can disappear entirely.
    scale: f32,
    // 1 / fadeout_distance
    dist_fadeout_const: f32,
    // 1 / dot_fadeout_strength (keep in sync with WGSL)
    dot_fadeout_const: f32,
    x_axis_color: Vec3,
    z_axis_color: Vec3,
    // Shared grid line color (RGB) + base alpha. All grid scales use this; opacity is scale-weighted in WGSL.
    grid_line_color: Vec4,
    // Axis opacity multiplier; axis RGB comes from x_axis_color/z_axis_color.
    axis_alpha: f32,
}

impl GridDisplaySettingsUniform {
    fn from_settings(settings: &InfiniteGridSettings) -> Self {
        Self {
            // 1m == 1 Bevy unit, so scale is just 1.0.
            scale: 1.0,
            dist_fadeout_const: 1.0 / settings.fadeout_distance.max(0.0001),
            // Keep the existing WGSL behavior: angle-based fade factor.
            dot_fadeout_const: 1.0 / 0.25,
            x_axis_color: X_AXIS_COLOR.to_linear().to_vec3(),
            z_axis_color: Z_AXIS_COLOR.to_linear().to_vec3(),
            // "Shared" grid line color. Make it brighter than before; opacity is controlled by the WGSL weights.
            // Use the previous major/minor alphas as a reasonable base.
            grid_line_color: LinearRgba::new(
                1.0,
                1.0,
                1.0,
                settings.major_alpha.max(settings.minor_alpha),
            )
            .to_vec4(),
            // Always show axes; you can tune this if they feel too strong.
            axis_alpha: 1.0,
        }
    }
}

#[derive(Clone, ShaderType)]
struct GridViewUniform {
    projection: Mat4,
    inverse_projection: Mat4,
    view: Mat4,
    inverse_view: Mat4,
    world_position: Vec3,
}

#[derive(Resource, Default)]
struct GridPlaneUniforms {
    uniforms: DynamicUniformBuffer<GridPlaneUniform>,
}

#[derive(Resource, Default)]
struct GridDisplaySettingsUniforms {
    uniforms: DynamicUniformBuffer<GridDisplaySettingsUniform>,
}

#[derive(Resource, Default)]
struct GridViewUniforms {
    uniforms: DynamicUniformBuffer<GridViewUniform>,
}

// ----------------------
// Render commands
// ----------------------
struct SetGridViewBindGroup<const I: usize>;

impl<const I: usize, P: PhaseItem> RenderCommand<P> for SetGridViewBindGroup<I> {
    type Param = ();
    type ViewQuery = (Read<GridViewUniformOffset>, Read<GridViewBindGroup>);
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        (view_offset, view_bg): ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut bevy::render::render_phase::TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &view_bg.value, &[view_offset.offset]);
        RenderCommandResult::Success
    }
}

struct SetGridBindGroup<const I: usize>;

impl<const I: usize, P: PhaseItem> RenderCommand<P> for SetGridBindGroup<I> {
    type Param = SRes<GridBindGroup>;
    type ViewQuery = ();
    type ItemQuery = Read<GridUniformOffsets>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        item_offsets: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut bevy::render::render_phase::TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(item_offsets) = item_offsets else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(
            I,
            &bind_group.into_inner().value,
            &[item_offsets.plane_offset, item_offsets.settings_offset],
        );

        RenderCommandResult::Success
    }
}

struct DrawFullscreenQuad;

impl<P: PhaseItem> RenderCommand<P> for DrawFullscreenQuad {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut bevy::render::render_phase::TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // Triangle strip with 4 vertices.
        pass.draw(0..4, 0..1);
        RenderCommandResult::Success
    }
}

type DrawInfiniteGrid = (
    SetItemPipeline,
    SetGridViewBindGroup<0>,
    SetGridBindGroup<1>,
    DrawFullscreenQuad,
);

// ----------------------
// Pipeline
// ----------------------
#[derive(Resource)]
struct InfiniteGridPipeline {
    view_layout: BindGroupLayout,
    grid_layout: BindGroupLayout,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct GridPipelineKey {
    mesh_key: MeshPipelineKey,
    sample_count: u32,
}

impl FromWorld for InfiniteGridPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let view_layout = render_device.create_bind_group_layout(
            "simple-infinite-grid-view-layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                uniform_buffer::<GridViewUniform>(true),
            ),
        );

        // Two dynamic UBOs:
        // - plane/orientation
        // - display settings
        let grid_layout = render_device.create_bind_group_layout(
            "simple-infinite-grid-layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    uniform_buffer::<GridPlaneUniform>(true),
                    uniform_buffer::<GridDisplaySettingsUniform>(true),
                ),
            ),
        );

        Self {
            view_layout,
            grid_layout,
        }
    }
}

impl SpecializedRenderPipeline for InfiniteGridPipeline {
    type Key = GridPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let format = if key.mesh_key.contains(MeshPipelineKey::HDR) {
            ViewTarget::TEXTURE_FORMAT_HDR
        } else {
            TextureFormat::bevy_default()
        };

        RenderPipelineDescriptor {
            label: Some(Cow::Borrowed("simple-infinite-grid-pipeline")),
            layout: vec![self.view_layout.clone(), self.grid_layout.clone()],
            push_constant_ranges: Vec::new(),
            vertex: VertexState {
                shader: GRID_SHADER_HANDLE,
                shader_defs: vec![],
                entry_point: Some(Cow::Borrowed("vertex")),
                buffers: vec![],
            },
            primitive: PrimitiveState {
                topology: bevy::render::render_resource::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: bevy::render::render_resource::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            // Bevy 0.17 uses reversed-z by default for 3D.
            // This matches the crate: `Greater`, no depth write, Depth32Float.
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Greater,
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: GRID_SHADER_HANDLE,
                shader_defs: vec![],
                entry_point: Some(Cow::Borrowed("fragment")),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

// ----------------------
// Systems (extract/prepare/queue)
// ----------------------
fn extract_infinite_grid_enabled(
    mut commands: Commands,
    enabled: Extract<Res<InfiniteGridEnabled>>,
) {
    // Mirror the toggle into the render world so render systems can read it.
    // `enabled` is a `Res<InfiniteGridEnabled>`; we want to insert the *inner* resource.
    commands.insert_resource(**enabled);
}

fn extract_grids(
    mut commands: Commands,
    grids: Extract<
        Query<(
            RenderEntity,
            &InfiniteGridSettings,
            &GlobalTransform,
            &RenderVisibleEntities,
        )>,
    >,
) {
    let extracted: Vec<_> = grids
        .iter()
        .map(|(entity, settings, transform, visible_entities)| {
            (
                entity,
                (
                    ExtractedGrid {
                        transform: *transform,
                        settings: *settings,
                    },
                    visible_entities.clone(),
                ),
            )
        })
        .collect();

    commands.try_insert_batch(extracted);
}

fn prepare_view_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut view_uniforms: ResMut<GridViewUniforms>,
    views: Query<(Entity, &ExtractedView)>,
) {
    view_uniforms.uniforms.clear();

    for (entity, view) in views.iter() {
        let projection = view.clip_from_view;
        let view_mat = view.world_from_view.to_matrix();
        let inverse_view = view_mat.inverse();

        let offset = view_uniforms.uniforms.push(&GridViewUniform {
            projection,
            inverse_projection: projection.inverse(),
            view: view_mat,
            inverse_view,
            world_position: view.world_from_view.translation(),
        });

        commands
            .entity(entity)
            .insert(GridViewUniformOffset { offset });
    }

    view_uniforms
        .uniforms
        .write_buffer(&render_device, &render_queue);
}

fn prepare_view_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    view_uniforms: Res<GridViewUniforms>,
    pipeline: Res<InfiniteGridPipeline>,
    views: Query<Entity, With<GridViewUniformOffset>>,
) {
    let Some(binding) = view_uniforms.uniforms.binding() else {
        return;
    };

    for view_entity in views.iter() {
        let bg = render_device.create_bind_group(
            "simple-infinite-grid-view-bind-group",
            &pipeline.view_layout,
            &BindGroupEntries::single(binding.clone()),
        );
        commands
            .entity(view_entity)
            .insert(GridViewBindGroup { value: bg });
    }
}

fn prepare_plane_and_settings_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    grids: Query<(Entity, &ExtractedGrid)>,
    mut plane_uniforms: ResMut<GridPlaneUniforms>,
    mut settings_uniforms: ResMut<GridDisplaySettingsUniforms>,
) {
    plane_uniforms.uniforms.clear();
    settings_uniforms.uniforms.clear();

    for (entity, grid) in grids.iter() {
        let gt = grid.transform;
        let t = gt.compute_transform();

        let origin = gt.translation();
        let normal = *gt.up();
        let planar_rotation_matrix = Mat3::from_quat(t.rotation.inverse());

        let plane_offset = plane_uniforms.uniforms.push(&GridPlaneUniform {
            planar_rotation_matrix,
            origin,
            normal,
        });

        let settings_offset = settings_uniforms
            .uniforms
            .push(&GridDisplaySettingsUniform::from_settings(&grid.settings));

        commands.entity(entity).insert(GridUniformOffsets {
            plane_offset,
            settings_offset,
        });
    }

    plane_uniforms
        .uniforms
        .write_buffer(&render_device, &render_queue);
    settings_uniforms
        .uniforms
        .write_buffer(&render_device, &render_queue);
}

fn prepare_grid_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    plane_uniforms: Res<GridPlaneUniforms>,
    settings_uniforms: Res<GridDisplaySettingsUniforms>,
    pipeline: Res<InfiniteGridPipeline>,
) {
    let Some((plane_binding, settings_binding)) = plane_uniforms
        .uniforms
        .binding()
        .zip(settings_uniforms.uniforms.binding())
    else {
        return;
    };

    let bg = render_device.create_bind_group(
        "simple-infinite-grid-bind-group",
        &pipeline.grid_layout,
        &BindGroupEntries::sequential((plane_binding.clone(), settings_binding.clone())),
    );

    commands.insert_resource(GridBindGroup { value: bg });
}

fn queue_grids(
    grid_enabled: Option<Res<InfiniteGridEnabled>>,
    pipeline_cache: Res<PipelineCache>,
    draw_functions: Res<DrawFunctions<Transparent3d>>,
    pipeline: Res<InfiniteGridPipeline>,
    mut specialized: ResMut<SpecializedRenderPipelines<InfiniteGridPipeline>>,
    grids: Query<&ExtractedGrid>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut views: Query<(&ExtractedView, &RenderVisibleEntities, &Msaa)>,
) {
    // Be robust: if the toggle isn't present yet for some reason, default to enabled.
    if matches!(grid_enabled.as_deref(), Some(InfiniteGridEnabled(false))) {
        return;
    }

    let draw_function_id = draw_functions
        .read()
        .get_id::<DrawInfiniteGrid>()
        .expect("DrawInfiniteGrid should be registered");

    for (view, visible_entities, msaa) in views.iter_mut() {
        let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        let mesh_key = MeshPipelineKey::from_hdr(view.hdr);
        let pipeline_id = specialized.specialize(
            &pipeline_cache,
            &pipeline,
            GridPipelineKey {
                mesh_key,
                sample_count: msaa.samples(),
            },
        );

        // RenderVisibleEntities contains the list of entities visible for this view for each `VisibilityClass`.
        // Because our grid entities include `VisibilityClass` via `InfiniteGridSettings`, we can iterate them.
        for &entity in visible_entities.iter::<InfiniteGridSettings>() {
            if grids.get(entity.0).is_err() {
                continue;
            }

            phase.items.push(Transparent3d {
                pipeline: pipeline_id,
                entity,
                draw_function: draw_function_id,
                // Ensures it sorts "behind" other transparent items.
                distance: f32::NEG_INFINITY,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: false,
            });
        }
    }
}

#[derive(Component)]
struct GridScaleOverlay;

fn spawn_grid_scale_overlay(mut commands: Commands) {
    commands.spawn((
        GridScaleOverlay,
        Text::new("Grid: ?"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.92, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            ..default()
        },
    ));
}

fn update_grid_scale_overlay(
    mut q_text: Query<&mut Text, With<GridScaleOverlay>>,
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
) {
    let Ok(mut text) = q_text.single_mut() else {
        return;
    };
    let Ok(cam_gt) = q_cam.single() else {
        return;
    };

    // Plane is y=0 in this editor.
    let h = cam_gt.translation().y.abs();
    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    // Shader bias: t' = t^2
    fn bias_75_25(t: f32) -> f32 {
        t * t
    }

    // Dominance cutoff: show the "next" scale only when it is >= ~75%.
    let dominance_cutoff = 0.75;

    let scale_label = if h < 90.0 {
        let t = bias_75_25(smoothstep(5.0, 90.0, h));
        if t >= dominance_cutoff { "10m" } else { "1m" }
    } else {
        let t = bias_75_25(smoothstep(40.0, 600.0, h));
        if t >= dominance_cutoff { "100m" } else { "10m" }
    };

    text.0 = format!("Grid: {}", scale_label);
}
