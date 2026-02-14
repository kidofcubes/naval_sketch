use std::collections::HashMap;
use std::iter::once;
use std::ops::Deref;
use std::sync::Arc;
use anyhow::anyhow;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use crate::parts::{AdjustableHull, BasePart, Part, PartRegistry};
use crate::parts_loader::{get_all_parts, LocalPaths, PartData};

///todo split bevy specific data into its own feature
///
#[derive(Resource)]
struct ReadyMutexResource(
    Arc<
        std::sync::Mutex<
            Option<Vec<PartData>>,
        >,
    >,
);


pub struct NavalSketchPlugin{
    pub init_paths: LocalPaths,
}

impl Plugin for NavalSketchPlugin {
    fn build(&self, app: &mut App) {
        // let data_paths = app.world().resource::<InitData>().data_paths.clone();
        // structure taken from RenderPlugin
        app.add_systems(Update, on_part_meshes_init);
        let lock = Arc::new(std::sync::Mutex::new(None));
        app.insert_resource(ReadyMutexResource(lock.clone()));

        let part_retriever = async move {
            let stuff = get_all_parts(Some(&self.init_paths)).await.unwrap();
            let mut thingy = lock.lock().unwrap();
            *thingy = Some(stuff);
        };



        #[cfg(target_arch = "wasm32")]
        bevy::tasks::IoTaskPool::get()
            .spawn_local(part_retriever)
            .detach();
        // Otherwise, just block for it to complete
        #[cfg(not(target_arch = "wasm32"))]
        futures_lite::future::block_on(part_retriever);
    }

    fn ready(&self, app: &App) -> bool {

        app.world()
            .get_resource::<ReadyMutexResource>()
            .and_then(|frr| frr.0.try_lock().map(|locked| locked.is_some()).ok())
            .unwrap_or(true)
    }
    fn cleanup(&self, app: &mut bevy::app::App) {
        let thing = app.world_mut().remove_resource::<ReadyMutexResource>().unwrap();
        let result = thing.0.lock().unwrap().take().unwrap();


        // #[cfg(target_arch = "wasm32")]
        // //let mut part_registry = PartRegistry { path_prefix: Some(std::path::PathBuf::new()), parts: HashMap::new() };
        // let mut part_registry = PartRegistry { path_prefix: None, parts: HashMap::new() };
        // #[cfg(not(target_arch = "wasm32"))]
        // let mut part_registry = PartRegistry { path_prefix: Some(app.world().resource::<InitData>().data_paths.as_ref().unwrap().cache_folder.clone()), parts: HashMap::new() };
        let mut part_registry = PartRegistry { parts: HashMap::new() };

        for part in result {
            part_registry.parts.insert(part.id,part);
        }
        app.world_mut().insert_resource(part_registry);

    }
}

pub fn generate_part_registry(paths: &LocalPaths) -> PartRegistry{
    let mut registry = PartRegistry {
        parts: HashMap::new(),
    };


    // get_all_parts(None);
    // info!("DONE DOING THE THING 1");
    // let result = futures::executor::block_on(get_all_parts(init_data.data_paths.as_ref()));

    #[cfg(target_arch = "wasm32")]
    {
        // can't get the value out, or pass my resource in (also doesn't seem to block)
        // wasm_bindgen_futures::spawn_local(async {
        //     let result = get_all_parts(None).await;
        //     info!("DONE DOING THE THING 3 ");
        // });

        // fails with a panic
        // ComputeTaskPool::get().scope(|s| {
        //     s.spawn(async {
        //         let result = get_all_parts(None).await;
        //         info!("DONE DOING THE THING 3 ");
        //     });
        // });

        // hangs forever
        // futures::executor::block_on(async{
        //         let result = get_all_parts(None).await;
        //         info!("DONE DOING THE THING 4 ");
        // });

        // let result = pollster::FutureExt::block_on(get_all_parts(None));
        // info!("DONE DOING THE THING 3 ");


        // bevy::tasks::block_on(async {
        //         let result = get_all_parts(None).await;
        //         info!("DONE DOING THE THING 3 ");
        //         if let Ok(parts) = result {
        //             for part in parts {
        //                 part_registry.parts.insert(part.id,part);
        //             }
        //         }
        //         info!("DONE DOING THE THING 4 ");
        //
        //
        // });
        info!("DONE DOING THE THING 5 ");
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let result = futures_lite::future::block_on(get_all_parts(Some(paths)));
        if let Ok(parts) = result {
            for part in parts {
                registry.parts.insert(part.id,part);
            }
        }

    }
    // info!("DONE DOING THE THING 2 {:?}",result.unwrap().len());


    // let workshop_parts = get_workshop_parts(&workshop_folder, &cache_folder);
    // for workshop_port in workshop_parts {
    //     part_registry.parts.insert(workshop_port.id,workshop_port);
    // }
    //
    // let builtin_parts = get_builtin_parts(&game_folder, &cache_folder);
    // for builtin_part in builtin_parts {
    //     part_registry.parts.insert(builtin_part.id,builtin_part);
    // }

    // println!("all registered parts is {:?}",part_registry.parts.keys());
    return registry;

}

pub fn get_collider(
    base_part: &BasePart,
    adjustable_hull: Option<&AdjustableHull>,
    part_data: &PartData
) -> Transform{
    let mut transform = base_part_to_bevy_transform(base_part);
    transform.translation += unity_to_bevy_translation(&part_data.center);
    transform.scale = part_data.collider * base_part.scale;
    //let mut transform = Transform::from_translation(unity_to_bevy_translation(&(hovered.0.position+part_data.center))).with_scale(part_data.collider);
    if let Some(adjustable_hull) = adjustable_hull {
        transform.scale = transform.scale * Vec3 {
            x: f32::max(adjustable_hull.back_width+adjustable_hull.back_spread,adjustable_hull.front_width+adjustable_hull.front_spread),
            y: adjustable_hull.height,
            z: adjustable_hull.length
        }/6.0;
    }
    return transform;
}

pub fn generate_adjustable_hull_mesh(mesh: &mut Mesh, adjustable_hull: &AdjustableHull) {
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
        indices.extend_from_slice(&[index+(lengths)-1, index,         index-1]);
        indices.extend_from_slice(&[index+(lengths)-1, index+lengths, index  ]);
    }

    let index = 0;
    indices.extend_from_slice(&[(lengths-1)+(lengths)-1, index,         (lengths-1)-1]);
    indices.extend_from_slice(&[(lengths-1)+(lengths)-1, index+lengths, index]);








    //mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0,0.0,1.0]; vertices.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0,0.0]; vertices.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_indices(Indices::U32(indices));

    mesh.duplicate_vertices();
    mesh.compute_flat_normals();

}

pub fn adjustable_hull_side(adjustable_hull: &AdjustableHull, resolution: usize, front: bool) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(resolution+1);
    let half_width = (if front {adjustable_hull.front_width} else {adjustable_hull.back_width} )*0.5;
    let half_spread = (if front {adjustable_hull.front_spread} else {adjustable_hull.back_spread})*0.5;
    let height_multiplier = if front {adjustable_hull.height_scale*adjustable_hull.height*0.5} else {adjustable_hull.height*0.5};
    let max_half_height = adjustable_hull.height*0.5;
    let height_offset = if front {adjustable_hull.height_offset*adjustable_hull.height} else {0.0};

    //println!("=======================================the adjustable_hull is {:?}",adjustable_hull);
    //println!("=======================================the frontness is {:?}",front);
    //len 1 height 4.7 forward width 0.25 backward width 2.65 forward spread 0.765 backwardspread 1.02 height scale 0.94175 height offset 0.01912 top roundness 0 bottom roundness 1
    //90 180 0

    let mut sum_x: f32 = 0.0;
    let mut sum_y: f32 = 0.0;

    for i in 0..resolution {
        let angle = std::f32::consts::TAU*((i as f32)/(resolution as f32));
        let cos_angle = f32::cos(angle);
        let sin_angle = f32::sin(angle);

        //println!("calced the cos and sin of {} ({}), which was {} and {}",angle,(i as f32/resolution as f32),cos_angle,sin_angle);

        let multiplier: f32 = f32::lerp(
            1.0 / f32::max(f32::abs(sin_angle),f32::abs(cos_angle)),
            1.0,
            if sin_angle>0.0 {adjustable_hull.top_roundness} else {adjustable_hull.bottom_roundness}
        );

        //println!("the height {} percentage is {} and the cosangle is {} but times multiplier is {} and the lerp is from {} to {} ", sin_angle*multiplier,((sin_angle*multiplier)/2.0)+0.5,cos_angle,cos_angle*multiplier,half_width,half_width+half_spread);



        //vertices.push([(cos_angle*multiplier*half_width)+(cos_angle.signum()*(((sin_angle/2.0)+1.0)*half_spread)), f32::clamp((sin_angle*multiplier*height_multiplier)+height_offset,-max_height,max_height), (if front {0.5} else {-0.5})*adjustable_hull.length]);
        vertices.push([
            cos_angle*multiplier*f32::lerp(half_width,half_width+half_spread,((sin_angle*multiplier)/2.0)+0.5),
            f32::clamp((sin_angle*multiplier*height_multiplier)+height_offset,-max_half_height,max_half_height),
            (if front {0.5} else {-0.5})*adjustable_hull.length
        ]);
        sum_x+=vertices.last().unwrap()[0];
        sum_y+=vertices.last().unwrap()[1];
        //println!("the result is {:?}",vertices.last().unwrap());
        //println!("adding the {}, {}, 0.0 to vertices",cos_angle*multiplier,sin_angle*multiplier);
    }

    vertices.push([sum_x/(resolution as f32),sum_y/(resolution as f32),(if front {0.5} else {-0.5})*adjustable_hull.length]);


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

pub fn colored_part_material(color: Color) -> StandardMaterial {
    let mut material = StandardMaterial::from_color(color);
    material.reflectance=0.1;
    material.double_sided=true;
    return material;
}



#[derive(Component, Debug, Clone)]
pub struct BasePartMeshes {
    pub meshes: Vec<Entity>,
}

#[derive(Component, Debug, Copy, Clone)]
pub struct BasePartMesh{
    pub base_part: Entity,
}

pub fn get_base_part_entity(parent_query: &Query<&ChildOf>, part_query: &Query<&BasePart>, entity: Entity) -> Option<Entity>{
    // i'm assuming iter_ancestors loops it in order of nearest parent hopfully
    for base_entity in once(entity).chain(parent_query.iter_ancestors(entity)) {
        if part_query.get(base_entity).is_ok() {
            return Some(base_entity);
        }
    };
    return None;
}
pub fn argb_slice_to_color(color: &[u8; 4]) -> Color {
    return Color::srgba_u8(color[1],color[2],color[3],color[0]);
}

pub fn on_part_meshes_init(
    mut mesh_query: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), Added<Mesh3d>>,
    base_part_query: Query<&BasePart>,
    parent_query: Query<&ChildOf>,
    mut base_part_meshes_query: Query<&mut BasePartMeshes>,
    layer_query: Query<&RenderLayers>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
){
    let mut temp = HashMap::new();
    for mut entity in &mut mesh_query {
        if let Some(base_part_entity) = get_base_part_entity(&parent_query, &base_part_query, entity.0) {
            if let Ok(base_part_meshes) = &mut base_part_meshes_query.get_mut(base_part_entity) {
                base_part_meshes.meshes.push(entity.0);
            }else{
                temp.insert(base_part_entity, BasePartMeshes {meshes:Vec::new()});
                temp.get_mut(&base_part_entity).unwrap().meshes.push(entity.0);
            }
            commands.get_entity(entity.0).unwrap().insert(BasePartMesh{base_part:base_part_entity});

            // if let Ok(layer) = layer_query.get(base_part_entity) {
            //     commands.get_entity(entity.0).unwrap().insert(layer.clone());
            // }

            entity.1.0 = materials.add(colored_part_material(argb_slice_to_color(&base_part_query.get(base_part_entity).unwrap().color)));
        }
    }
    for pair in temp {
        commands.get_entity(pair.0).unwrap().insert(pair.1);
    }
}

pub fn place_part<'a>(
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    asset_server: &Res<AssetServer>,
    part_registry: &Res<PartRegistry>,
    entity: &mut EntityCommands,
    part: &Part
) -> anyhow::Result<()> {

    let Some(part_data) = part_registry.parts.get(&part.base_part().id) else {
        return Err(anyhow!("oh noooo"));
    };
    // let mut entity: EntityCommands = commands.spawn((
    //
    // ));
    entity.insert(base_part_to_bevy_transform(part.base_part()));
    match part {
        Part::Normal(base_part) => entity.insert(*base_part),
        Part::AdjustableHull(base_part, adjustable_hull) => entity.insert((*base_part, *adjustable_hull)),
        Part::Turret(base_part, turret) => entity.insert((*base_part, *turret)),
    };




    if let Part::AdjustableHull(base_part, adjustable_hull) = part {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);

        generate_adjustable_hull_mesh(
            &mut mesh,
            adjustable_hull
        );

        entity.insert((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(argb_slice_to_color(&base_part.color)))
        ));
    }else{
        // println!("looking for part with id {:?}",&part.base_part().id);
        // println!("loaded parts are {:?}",part_registry.parts.keys());
        //let asset_path = AssetPath::from(part_data.model_asset_path(part_registry.path_prefix.as_deref()));
        //let asset_path = AssetPath::from(part_data.model_asset_path(part_registry.path_prefix.as_deref()));
        let mut handle = asset_server.get_handle(part_data.model.clone());
        if handle.is_none() {
            handle = Some(asset_server.load_override(
                GltfAssetLabel::Scene(0).from_asset(
                    part_data.model.clone()
                )
            ));
        }
        let unwrapped = handle.unwrap();

        // let thing = scene_assets.get(unwrapped.id()).unwrap();
        //
        // for scene_entity in thing.world.iter_entities() {
        //     println!("wtfrick theres a {:?}",scene_entity.id());
        // }


        entity.insert(
            SceneRoot(unwrapped)
        );
    }




    match part {
        Part::Normal(base_part) => {},
        Part::AdjustableHull(base_part, adjustable_hull) => {

        },
        Part::Turret(base_part, turret) => {},
    }
    return Ok(());
}

// fn initalize_part_scene(trigger: dyn Trigger<HierarchyEvent>, children: Query<&Children>){
//     println!("TRIGGERED EVENT FOR {:?} which was {:?}",trigger.entity(),trigger.event());
// }

pub fn base_part_to_bevy_transform(base_part: &BasePart) -> Transform{

    let mut transform = Transform::from_xyz(
        base_part.position.x,
        base_part.position.y,
        base_part.position.z,
    ).with_rotation(
        unity_to_bevy_quat(&base_part.rotation)
    )
        .with_scale(base_part.scale.abs()); //absed because yeah

    transform.translation.x = -transform.translation.x;
    return transform;
}

pub fn unity_to_bevy_quat(a: &Vec3) -> Quat{
    let initial_quat = (Quat::from_euler(
        EulerRot::YXZ,
        a.y.to_radians(),
        a.x.to_radians(),
        a.z.to_radians(),
    ));
    return Quat::from_xyzw(
        -initial_quat.x,
        initial_quat.y,
        initial_quat.z,
        -initial_quat.w
    );
}

pub fn bevy_quat_to_unity(a: &Quat) -> Vec3{
    let quat = Quat::from_xyzw(
        -a.x,
        a.y,
        a.z,
        -a.w
    );
    let vec = Vec3::from(quat.to_euler(EulerRot::YXZ));
    return Vec3::new(vec.y.to_degrees(), vec.x.to_degrees(), vec.z.to_degrees());
}



pub fn unity_to_bevy_translation(pos: &Vec3) -> Vec3{
    let mut new_pos = pos.clone();
    new_pos.x=-new_pos.x;
    return new_pos;
}
pub fn bevy_to_unity_translation(pos: &Vec3) -> Vec3{
    let mut new_pos = pos.clone();
    new_pos.x=-new_pos.x;
    return new_pos;
}