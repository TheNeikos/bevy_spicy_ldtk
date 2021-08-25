use bevy::{
    math::vec2,
    prelude::*,
    render::texture::{Extent3d, FilterMode, TextureDimension, TextureFormat},
};
use bevy_spicy_ldtk::{ldtk, DeserializeLdtk, SpecialLayerDefinitions, SpecialValues};

ldtk! {pub levels, "assets/int_grid.ldtk"}

fn main() {
    let data = ldtk2::Ldtk::from_path(levels::FILEPATH).unwrap();
    let project = levels::Project::deserialize_ldtk(&data).unwrap();

    App::new()
        .insert_resource(ClearColor(project.levels[0].background_color))
        .insert_resource(project)
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_camera)
        .add_startup_system(generate_texture)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
}

fn generate_texture(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut textures: ResMut<Assets<Texture>>,
    project: Res<levels::Project>,
) {
    let level = &project.levels[0];
    let layer = &level.layers.int_grid_example_layer;
    let values = match &layer.special {
        SpecialValues::IntGrid {
            values,
            auto_layer: _,
        } => values,
        _ => panic!("Unexpected layer type"),
    };

    let value_definitions = match &project.layer_definitions[&layer.layer_definition].special {
        SpecialLayerDefinitions::IntGrid { value_definitions } => value_definitions,
        _ => panic!("Unexpected layer definition type"),
    };

    // 0 means "empty" tile
    let mut colors = vec![[0, 0, 0, 0]];
    colors.extend(value_definitions.iter().map(|def| {
        let c = def.color;
        [c.r(), c.g(), c.b(), c.a()].map(|v| (v * 255.) as u8)
    }));

    let buffer = values.iter().flat_map(|i| colors[*i as usize]).collect();

    let dimension = layer.dimensions_cell.as_u32();
    let mut texture = Texture::new(
        Extent3d::new(dimension.x, dimension.y, 1),
        TextureDimension::D2,
        buffer,
        TextureFormat::Rgba8Unorm,
    );
    texture.sampler.min_filter = FilterMode::Nearest;

    let texture = textures.add(texture);
    let material = materials.add(texture.into());

    commands.spawn_bundle(SpriteBundle {
        sprite: Sprite {
            size: vec2(500., 500.),
            resize_mode: SpriteResizeMode::Manual,
            ..Default::default()
        },
        material,
        ..Default::default()
    });
}
