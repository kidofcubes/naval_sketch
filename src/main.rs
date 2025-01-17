mod parsing;
mod cam_movement;
mod editor;
mod parts;

use bevy::{asset::{AssetPath, RenderAssetUsages}, color::{palettes::tailwind::{CYAN_300, GRAY_300, YELLOW_300}, Color}, core_pipeline::msaa_writeback::MsaaWritebackPlugin, hierarchy::HierarchyEvent, input::mouse::AccumulatedMouseMotion, prelude::*, reflect::List, render::mesh::{Extrudable, Indices}, window::CursorGrabMode};
use bevy_mod_outline::OutlinePlugin;
use cam_movement::{advance_physics, grab_mouse, handle_input, interpolate_rendered_transform, move_player, spawn_player, spawn_text, CameraMovementPlugin};
use editor::EditorPlugin;
use parsing::{load_save, AdjustableHull, BasePart, HasBasePart, Part};
use parts::place_part;
use std::{cmp::{max, min}, env, f32::consts::FRAC_PI_2};



/// Returns an observer that updates the entity's material to the one specified.
fn update_material_on<E>(
    new_material: Handle<StandardMaterial>,
) -> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
    // An observer closure that captures `new_material`. We do this to avoid needing to write four
    // versions of this observer, each triggered by a different event and with a different hardcoded
    // material. Instead, the event type is a generic, and the material is passed in.
    move |trigger, mut query| {
        if let Ok(mut material) = query.get_mut(trigger.entity()) {
            material.0 = new_material.clone();
        }
    }
}



fn temp_test_update(
    mut mesh_thing: ResMut<BuildData>,
    mut meshes: ResMut<Assets<Mesh>>,
    key: Res<ButtonInput<KeyCode>>,
    mut query: Query<(Entity, &mut Mesh3d, &mut MeshMaterial3d<StandardMaterial>)>,
    mut hull_query: Query<(Entity, &BasePart, &mut AdjustableHull)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parent_query: Query<&Parent>,
    mut base_part_query: Query<(Entity, &BasePart)>,
    mut commands: Commands
) {
    if key.just_pressed(KeyCode::KeyH) {
        let hovered_mat = StandardMaterial::from_color(Color::WHITE);
        let hovered_mat_handle = materials.add(hovered_mat);
        

        for mut pair in &mut query {
            let colored_mat = StandardMaterial::from_color(base_part_query.get(parent_query.root_ancestor(pair.0)).unwrap().1.color);
            let colored_mat_handle = materials.add(colored_mat);
            // materials.insert(pair.2.id(),
            //     colored_mat.clone()
            // );
            pair.2.0 = colored_mat_handle.clone();
            // commands.entity(pair.0)
            //     .observe(update_material_on::<Pointer<Over>>(hovered_mat_handle.clone()))
            //     .observe(update_material_on::<Pointer<Out>>(colored_mat_handle))
            // ;
        }
    }

    //if key.just_pressed(KeyCode::KeyJ) {
    //    for mut pair in &mut hull_query {

    //        let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList,RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);

    //        generate_adjustable_hull_mesh(
    //            &mut mesh,
    //            &pair.2
    //        );

    //        commands.entity(pair.0).insert((
    //            Mesh3d(meshes.add(mesh)),
    //            MeshMaterial3d(materials.add(pair.1.color))
    //        ));
    //    }
    //}
}









#[derive(Resource)]
struct InitData {
    file_path: String
}

#[derive(Resource)]
struct BuildData{
    mesh_thing: Option<AssetId<Mesh>>,
}




/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    init_data: Res<InitData>,
    asset_server: Res<AssetServer>,
    mut ambient_light: ResMut<AmbientLight>,
) {


    let parts_result = load_save(&init_data.file_path);


    if let Ok(parts) = parts_result {
        for part in parts {
            place_part(
                &mut meshes,
                &mut materials,
                &asset_server,
                &mut commands,
                &part);
        }
    }else{
        println!("ERROR WAS {:?}",parts_result);

    }


    let mut light_transform = Transform::from_xyz(500.0, 500.0, 500.0);
    light_transform = light_transform.looking_at(Vec3 {x:0.0, y:0.0, z:0.0 }, Vec3::Y);
    // light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        light_transform,
    ));
    ambient_light.brightness = 2000.0;

    //commands.spawn();



}

fn main() {
    let args: Vec<String> = env::args().collect();

    let file_path = &args[1];

    println!("wtfric {file_path}");


    App::new()
        .insert_resource(InitData {file_path: file_path.to_string()})
        .insert_resource(BuildData {mesh_thing: None})
        .add_plugins((
                DefaultPlugins.build(),
                //OutlinePlugin,
                CameraMovementPlugin,
                MeshPickingPlugin,
                EditorPlugin
                ))
        .add_systems(Startup, (setup))
        .add_systems(Update, (temp_test_update))


        .run();
}
