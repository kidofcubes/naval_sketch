mod parsing;
mod cam_movement;

use bevy::{asset::RenderAssetUsages, color::{palettes::tailwind::{CYAN_300, GRAY_300, YELLOW_300}, Color}, core_pipeline::msaa_writeback::MsaaWritebackPlugin, input::mouse::AccumulatedMouseMotion, prelude::*, reflect::List, render::mesh::{Extrudable, Indices}, window::CursorGrabMode};
use cam_movement::{advance_physics, grab_mouse, handle_input, interpolate_rendered_transform, move_player, spawn_player, spawn_text};
use parsing::{load_save, AdjustableHull, BasePart, HasBasePart};
use core::f32;
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
        

        for mut pair in &mut query {
            materials.insert(pair.2.id(),
                StandardMaterial::from_color(base_part_query.get(parent_query.root_ancestor(pair.0)).unwrap().1.color)
            );
        }
    }

    if key.just_pressed(KeyCode::KeyJ) {
        for mut pair in &mut hull_query {

            let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList,RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);

            generate_adjustable_hull_mesh(
                &mut mesh,
                &pair.2
            );

            commands.entity(pair.0).insert((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(pair.1.color))
            ));
        }
    }
}




fn generate_adjustable_hull_mesh(mesh: &mut Mesh, /* base_part: BasePart, */ adjustable_hull: &AdjustableHull) {
    // mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec! [
    //     [0.0,0.0,0.0],
    //     [0.0,3.0,0.0],
    //     [3.0,0.0,0.0],
    //
    //     [3.0,3.0,0.0],
    //     [0.0,3.0,0.0],
    //     [3.0,0.0,0.0],
    // ]);
    //
    // mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 6]);
    // mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.]; 6]);
    //mesh.duplicate_vertices();
    //mesh.compute_flat_normals();


    // mesh.insert_indices(Indices::U32(vec![
    //         0, 1, 2,
    //         4, 3, 5,
    // ]));
    let resolution = 6*4;

    let mut vertices : Vec<[f32;3]> = Vec::with_capacity((resolution+1)*2);
    let mut indices: Vec<u32> = Vec::with_capacity(((3*resolution)*2)+(resolution*6));

    let front = adjustable_hull_side(adjustable_hull, resolution, true );
    let mut back  = adjustable_hull_side(adjustable_hull, resolution, false);

    let lengths = front.0.len() as u32;

    vertices.extend(front.0);
    indices.extend(front.1);

    vertices.extend(back.0);
    for num in &mut back.1 {
        *num+=lengths as u32;
    }
    back.1.reverse();
    //println!("back.1 is now {:?}",back.1);
    indices.extend(back.1);
    //println!("vertices is {:?}",vertices);
    //println!("indices is {:?}",indices);
    for i in 1..(resolution) {
        let index = (i) as u32;
        //indices.extend_from_slice(&[index+(lengths)-1,index,index-1]);
        //indices.extend_from_slice(&[index+lengths-1,index+lengths,index]);
    }

    let index = 0;
    //indices.extend_from_slice(&[(lengths)+(lengths-2),index,lengths-2]);
    //indices.extend_from_slice(&[(lengths)+(lengths-2),index+lengths,index]);








    //mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0,0.0,1.0]; vertices.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0,0.0]; vertices.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_indices(Indices::U32(indices));

    mesh.duplicate_vertices();
    mesh.compute_flat_normals();

}

fn adjustable_hull_side(adjustable_hull: &AdjustableHull, resolution: usize, front: bool) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(resolution+1);
    let half_width = (if front {adjustable_hull.front_width} else {adjustable_hull.back_width} )*0.5;
    let half_spread = (if front {adjustable_hull.front_spread} else {adjustable_hull.back_spread})*0.5;
    let height_multiplier = if front {adjustable_hull.height_scale*adjustable_hull.height*0.5} else {adjustable_hull.height*0.5};
    let max_height = adjustable_hull.height*0.5;
    let height_offset = adjustable_hull.height_offset*max_height;

    //println!("the adjustable_hull is {:?}",adjustable_hull);
    //len 1 height 4.7 forward width 0.25 backward width 2.65 forward spread 0.765 backwardspread 1.02 height scale 0.94175 height offset 0.01912 top roundness 0 bottom roundness 1
    //90 180 0

    for i in 0..resolution {
        let angle = f32::consts::TAU*((i as f32)/(resolution as f32));
        let cos_angle = f32::cos(angle);
        let sin_angle = f32::sin(angle);

        println!("calced the cos and sin of {} ({}), which was {} and {}",angle,(i as f32/resolution as f32),cos_angle,sin_angle);

        let multiplier: f32 = f32::lerp(
            1.0 / f32::max(f32::abs(sin_angle),f32::abs(cos_angle)),
            1.0,
            if sin_angle>0.0 {adjustable_hull.top_roundness} else {adjustable_hull.bottom_roundness}
        );

        //println!("we are at {} and {} which is multiplied by {} which multiplied by {}",sin_angle,height_fraction,cos_angle.signum(),half_spread);

        

        //vertices.push([(cos_angle*multiplier*half_width)+(cos_angle.signum()*(((sin_angle/2.0)+1.0)*half_spread)), f32::clamp((sin_angle*multiplier*height_multiplier)+height_offset,-max_height,max_height), (if front {0.5} else {-0.5})*adjustable_hull.length]);
        vertices.push([
            cos_angle*multiplier*f32::lerp(half_width,half_width+half_spread,((sin_angle/2.0)+0.5)),
            (sin_angle*multiplier*height_multiplier)+height_offset,
            (if front {0.5} else {-0.5})*adjustable_hull.length
        ]);
        //println!("adding the {}, {}, 0.0 to vertices",cos_angle*multiplier,sin_angle*multiplier);
    }

    vertices.push([0.0,0.0,(if front {0.5} else {-0.5})*adjustable_hull.length]);


    let mut indices: Vec<u32> = Vec::with_capacity(3 * (resolution));


    for i in 1..resolution {
        //println!("{:?}", [(i as u32)-1, i as u32, resolution as u32]);
        //println!("did for i {i}");
        indices.extend_from_slice(&[(i as u32)-1, i as u32, resolution as u32]);
    }

    //println!("{:?}", [(resolution as u32)-1, 0, resolution as u32]);
    //println!("manually did for i {resolution}");
    indices.extend_from_slice(&[(resolution as u32)-1, 0, resolution as u32]);

    return (vertices,indices);

}





#[derive(Resource)]
struct InitData {
    file_path: String
}

#[derive(Resource)]
struct BuildData{
    mesh_thing: Option<AssetId<Mesh>>,
}


fn unity_to_bevy(transform: &Transform) -> Transform{
    let new_quat = Quat::from_xyzw(
        -transform.rotation.x, 
        transform.rotation.y, 
        transform.rotation.z, 
        -transform.rotation.w
    );
    let mut new_transform = transform.clone();
    new_transform.translation.x = -new_transform.translation.x;
    new_transform = new_transform.with_rotation(new_quat);
    return new_transform;
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    init_data: Res<InitData>,
    mut build_data: ResMut<BuildData>,
    asset_server: Res<AssetServer>,
    mut ambient_light: ResMut<AmbientLight>,
    mut msaa_query: Query<&mut Msaa>
) {
    // // circular base
    // commands.spawn((
    //     Mesh3d(meshes.add(Circle::new(4.0))),
    //     MeshMaterial3d(materials.add(Color::WHITE)),
    //     Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    // ));
    // // cube
    // commands.spawn((
    //     Mesh3d(meshes.add(Cuboid::new(5.0, 5.0, 5.0))),
    //     MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    //     Transform::from_xyz(0.0, 0.5, 0.0),
    // ));

    let parts_result = load_save(&init_data.file_path);
    // let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList,RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    // mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec! [
    //     [0.0,0.0,0.0],
    //     [1.0,2.0,1.0],
    //     [2.0,0.0,0.0],
    // ]);
    //
    // mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 3]);
    // mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.]; 3]);
    //
    // mesh.insert_indices(Indices::U32(vec![
    //         0, 2, 1,
    // ]));
    // let handle = meshes.add(mesh);
    // build_data.mesh_thing = Some(handle.id());
    //
    //
    // let white_matl = materials.add(Color::WHITE);
    // let ground_matl = materials.add(Color::from(GRAY_300));
    // let hover_matl = materials.add(Color::from(CYAN_300));
    // let pressed_matl = materials.add(Color::from(YELLOW_300));
    //
    //
    // commands.spawn((
    //     Mesh3d(handle),
    //     MeshMaterial3d(materials.add(Color::srgb_u8(255, 255, 255))),
    //     Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3 { x: 10.0, y: 10.0, z: 10.0 }),
    // ))
    //     .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
    //     .observe(update_material_on::<Pointer<Out>>(white_matl.clone()))
    //     .observe(update_material_on::<Pointer<Down>>(pressed_matl.clone()))
    //     .observe(update_material_on::<Pointer<Up>>(hover_matl.clone()))
    // ;

    if let Ok(parts) = parts_result {
        for part in parts {



            let mut entity: &mut EntityCommands = &mut commands.spawn((
                // Mesh3d(meshes.add(Cuboid::new(
                //             part.base_part().scale.x*3.0,
                //             part.base_part().scale.y*3.0,
                //             part.base_part().scale.z*3.0,
                // ))),

/*                 MeshMaterial3d(materials.add(part.base_part().color)), */
                unity_to_bevy(
                &Transform::from_xyz(
                    part.base_part().position.x,
                    part.base_part().position.y,
                    part.base_part().position.z,
                ).with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    part.base_part().rotation.y.to_radians(),
                    part.base_part().rotation.x.to_radians(),
                    part.base_part().rotation.z.to_radians(),
                ))
                .with_scale(part.base_part().scale)
                ),
                part.base_part().clone()
            ));

            // entity = entity.observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
            //     .observe(update_material_on::<Pointer<Out>>(white_matl.clone()))
            //     .observe(update_material_on::<Pointer<Down>>(pressed_matl.clone()))
            //     .observe(update_material_on::<Pointer<Up>>(hover_matl.clone()))
            //     ;

            //307 172 180

            match part {
                parsing::Part::Normal(base_part) => entity.insert((base_part)),
                parsing::Part::AdjustableHull(base_part, adjustable_hull) => entity.insert((base_part, adjustable_hull)),
                parsing::Part::Turret(base_part, turret) => entity.insert((base_part, turret)),
            };

            if part.base_part().id!=0 {
                entity.insert(
                     SceneRoot(asset_server.load(
                         GltfAssetLabel::Scene(0).from_asset(
                             format!("/home/kidofcubes/Downloads/AssetRipper_linux_x64/NavalArtOut/PrefabHierarchyObject/{}.glb",part.base_part().id)
                         ),
                     ))
                );
                // entity.insert((
                // ))
            }

        }
    }else{
        println!("ERROR WAS {:?}",parts_result);

    }


    let mut light_transform = Transform::from_xyz(500.0, 500.0, 500.0);
    light_transform = light_transform.looking_at(Vec3 {x:0.0, y:0.0, z:0.0 }, Vec3::Y);
    // light
    commands.spawn((
        // PointLight {
        //     radius: 1000.0,
        //     shadows_enabled: true,
        //     range: 1000.0,
        //     intensity: 1000.0*1000.0*1000.0*100.0,
        //     ..default()
        // },
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        light_transform,
    ));
    ambient_light.brightness = 2000.0;

    //commands.spawn();



    //// camera
    //commands.spawn((
    //    Camera3d::default(),
    //    Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    //));
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
                MeshPickingPlugin
                ))
        .add_systems(Startup, (setup, /* spawn_text, */ spawn_player))
        .add_systems(FixedUpdate, advance_physics)
        .add_systems(Update, (move_player,grab_mouse,temp_test_update))
        .add_systems(
            // The `RunFixedMainLoop` schedule allows us to schedule systems to run before and after the fixed timestep loop.
            RunFixedMainLoop,
            (
                // The physics simulation needs to know the player's input, so we run this before the fixed timestep loop.
                // Note that if we ran it in `Update`, it would be too late, as the physics simulation would already have been advanced.
                // If we ran this in `FixedUpdate`, it would sometimes not register player input, as that schedule may run zero times per frame.
                handle_input.in_set(RunFixedMainLoopSystem::BeforeFixedMainLoop),
                // The player's visual representation needs to be updated after the physics simulation has been advanced.
                // This could be run in `Update`, but if we run it here instead, the systems in `Update`
                // will be working with the `Transform` that will actually be shown on screen.
                interpolate_rendered_transform.in_set(RunFixedMainLoopSystem::AfterFixedMainLoop),
            ),
        )

        .run();
}
