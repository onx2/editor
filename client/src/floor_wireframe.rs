use bevy::{
    pbr::{Material, MaterialPlugin},
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct InfiniteGridMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub grid_scale: f32, // world-units per cell
    #[uniform(2)]
    pub line_width: f32, // line thickness (shader-dependent units)
}

impl Material for InfiniteGridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // Make it fully opaque so it renders strongly and isn't affected by blending/sort issues.
        AlphaMode::Opaque
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<InfiniteGridMaterial>::default());
    app.add_systems(Startup, setup);
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<InfiniteGridMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(2000.0, 2000.0))),
        MeshMaterial3d(materials.add(InfiniteGridMaterial {
            // Fully opaque to ensure visibility regardless of blending.
            color: LinearRgba::new(0.8, 0.8, 0.8, 1.0),
            grid_scale: 1.0,
            // Thicker lines so you can immediately confirm it's rendering.
            line_width: 2.0,
        })),
    ));
}
