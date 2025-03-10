mod parsing;
mod cam_movement;
mod editor;
mod editor_ui;
mod parts;
mod asset_extractor;
mod editor_utils;
mod editor_actions;

use bevy::{color::Color, pbr::wireframe::{WireframeConfig, WireframePlugin}, prelude::*, reflect::List, render::{settings::{RenderCreation, WgpuFeatures, WgpuSettings}, RenderPlugin}, utils::HashMap};
use bevy_egui::EguiPlugin;
use cam_movement::CameraMovementPlugin;
use editor::{EditorPlugin};
use parsing::{load_save, AdjustableHull, BasePart, Part};
use parts::{on_part_meshes_init, place_part, register_all_parts, BasePartMesh, BasePartMeshes, PartRegistry};
use transform_gizmo_bevy::{GizmoHotkeys, GizmoOptions, TransformGizmoPlugin};
use std::{env, path::Path};



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
    //mut mesh_thing: ResMut<BuildData>,
    meshes: ResMut<Assets<Mesh>>,
    key: Res<ButtonInput<KeyCode>>,
    mut query: Query<(Entity, &mut Mesh3d, &mut MeshMaterial3d<StandardMaterial>, &BasePartMesh)>,
    hull_query: Query<(Entity, &BasePart, &mut AdjustableHull)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scenes: ResMut<Assets<Scene>>,
    parent_query: Query<&Parent>,
    base_part_query: Query<(Entity, &BasePart)>,
    commands: Commands,
    gizmo: Gizmos
) {
    if key.just_pressed(KeyCode::KeyH) {
        let hovered_mat = StandardMaterial::from_color(Color::WHITE);
        let hovered_mat_handle = materials.add(hovered_mat);
        

        for mut pair in &mut query {
            //let colored_mat = StandardMaterial::from_color(base_part_query.get(parent_query.root_ancestor(pair.0)).unwrap().1.color);

            let colored_mat = StandardMaterial::from_color(base_part_query.get(pair.3.base_part).unwrap().1.color);
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
    // gizmo.cuboid(Transform::from_translation(Vec3::new(0.0,30.0,0.0)), Color::srgb_u8(0, 255, 0));
    // gizmo.cuboid(Transform::from_translation(Vec3::new(0.0,30.0,1.0)), Color::srgb_u8(0, 255, 0));
    //
    // let up_plane = Thing::Plane(Vec3::ZERO, *Dir3::Y, *Dir3::Z, *Dir3::X);
    // let vertex = Thing::Vertex(Vec3 { x: 0.0, y: 3.5, z: 0.0 });
    // let line1 = Thing::Line(
    //     Vec3 { x: 0.0, y: 5.0, z: 0.0 },
    //     Vec3 { x: 0.01, y: 10.0, z: 3.0 }
    // );
    // let line2 = Thing::Line(
    //     Vec3 { x: -0.01, y: -4.0, z: -0.1 },
    //     Vec3 { x: 0.0, y: -8.0, z: 0.0 }
    // );
    //
    //
    //
    // let line3 = Thing::Line(
    //     Vec3::new(1.0,10.0,-1.0),
    //     Vec3::new(-1.0,5.0,1.0)
    // );
    //
    // let line4 = Thing::Line(
    //     Vec3::new(-1.0,-10.0,-1.0),
    //     Vec3::new(1.0,-5.0,1.0)
    // );
    //
    // if let Thing::Line(start,end) = line3 {gizmo.line(start, end, Color::srgb_u8(255, 0, 0));}
    // if let Thing::Line(start,end) = line4 {gizmo.line(start, end, Color::srgb_u8(0, 0, 255));}
    // gizmo.cuboid(Transform::from_scale(Vec3::new(2.0,20.0,2.0)), Color::WHITE);
    


    //println!("totouchthing {:?}",to_touch_thing(&vertex, &up_plane, &Dir3::NEG_Y,&mut gizmo));
    //println!("totouchthing {:?}",to_touch_thing(&line1, &line2, &Dir3::NEG_Y,&mut gizmo));
    //println!("totouchthing {:?}",to_touch_thing(&line3, &line4, &Dir3::NEG_Y,&mut gizmo));

    // let cube_1 = Transform::from_translation(Vec3::new(10.0, 10.0, 10.0)).with_scale(Vec3::new(5.0,1.0,5.0));
    // for cube_thing in all_things(&cube_1) {
    //     println!("a thing for cube_1 is {:?}",cube_thing);
    // }
    

    // if scenes.is_changed() {
    //     for scene in scenes.iter() {
    //         for entity in scene.1.world.iter_entities() {
    //             println!("the thing is {:?}",entity.id());
    //         }
    //     }
    //
    // }

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
    file_path: String,
    steam_path: String
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
    part_registry: Res<PartRegistry>,
    //mut scene_assets: ResMut<Assets<Scene>>,
    mut ambient_light: ResMut<AmbientLight>,
    mut config_store: ResMut<GizmoConfigStore>,
) {

    let path = init_data.file_path.clone();

    let parts_result = load_save(Path::new(&path));


    println!("PLACING PARTS");
    if let Ok(parts) = parts_result {
        for part in parts {
            let mut entity = commands.spawn_empty();
            place_part(
                &mut meshes,
                &mut materials,
                &asset_server,
                &part_registry,
                &mut entity,
                &part);
        }
    }else{
        println!("ERROR WAS {:?}",parts_result);
        place_part(
                &mut meshes,
                &mut materials,
                &asset_server,
                &part_registry,
                &mut commands.spawn_empty(),
                &Part::Normal(BasePart {
                    id: 5,
                    ignore_physics: false,
                    position: Vec3 {x:10.0,y:10.0,z:10.0},
                    rotation: Vec3::ZERO,
                    scale: Vec3 {x:5.0,y:1.0,z:5.0},
                    color: Color::WHITE,
                    armor: 0,
                }));


        place_part(
                &mut meshes,
                &mut materials,
                &asset_server,
                &part_registry,
                &mut commands.spawn_empty(),
                &Part::Normal(BasePart {
                    id: 5,
                    ignore_physics: false,
                    position: Vec3 {x:20.0,y:10.0,z:10.0},
                    rotation: Vec3::ZERO,
                    scale: Vec3 {x:1.0,y:1.0,z:1.0},
                    color: Color::srgb_u8(0, 255, 0),
                    armor: 0,
                }));
        

        // commands.spawn((
        //     Mesh3d(meshes.add(Cuboid::default())),
        //     MeshMaterial3d(materials.add(Color::srgb_u8(0, 0, 255))),
        //     Transform::from_translation(Vec3::new(0.0,30.0,0.0))
        // ));
        // commands.spawn((
        //     Mesh3d(meshes.add(Cuboid::default())),
        //     MeshMaterial3d(materials.add(Color::srgb_u8(255, 0, 0))),
        //     Transform::from_translation(Vec3::new(0.0,30.0,1.0))
        // ));


        

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

    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();

    config.depth_bias = -1.0;



}
//NOTE, SCALE IS THE FULL WIDTH

fn main() {
    let args: Vec<String> = env::args().collect();

    let steam_path = &args[1];
    let file_path = &args[2];

    if file_path == "test" {
        

        App::new()
            .insert_resource(InitData {
                file_path: file_path.to_string(),
                steam_path: steam_path.to_string()
            })
            .insert_resource(PartRegistry {parts: HashMap::new()})
            .add_plugins((
                    DefaultPlugins.build(),
                    CameraMovementPlugin,
                    MeshPickingPlugin,
                    EditorPlugin,
                    //OutlinePlugin,
                    ))
            .add_systems(Startup, (register_all_parts.before(setup),setup))
            .add_systems(Update, (temp_test_update, on_part_meshes_init))


            .run();

        return;
    }


    App::new()
        .insert_resource(InitData {
            file_path: file_path.to_string(),
            steam_path: steam_path.to_string()
        })
        .insert_resource(PartRegistry {parts: HashMap::new()})
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: Color::WHITE,
        })

        .insert_resource(GizmoOptions {
            hotkeys: Some(GizmoHotkeys::default()),
            ..default()
        })

        .add_plugins((
                DefaultPlugins.set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        // WARN this is a native only feature. It will not work with webgl or webgpu
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }),
                    ..default()
                }).set(
                        AssetPlugin {
                            watch_for_changes_override: Some(false),
                            mode: AssetMode::Unprocessed,
                            meta_check: bevy::asset::AssetMetaCheck::Never,
                            ..default()
                        }
                ),
                WireframePlugin,
                CameraMovementPlugin,
                MeshPickingPlugin,
                EditorPlugin,
                //OutlinePlugin,
                EguiPlugin,
                TransformGizmoPlugin
                ))
        .add_systems(Startup, (register_all_parts,setup).chain())
        .add_systems(Update, (temp_test_update, on_part_meshes_init))


        .run();
}
