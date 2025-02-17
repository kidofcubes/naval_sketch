use bevy::{asset::{AssetPath, RenderAssetUsages}, color::{palettes::tailwind::{CYAN_300, GRAY_300, YELLOW_300}, Color}, core_pipeline::msaa_writeback::MsaaWritebackPlugin, hierarchy::HierarchyEvent, input::mouse::AccumulatedMouseMotion, prelude::*, reflect::List, render::{mesh::{Extrudable, Indices}, view::RenderLayers}, utils::HashMap, window::CursorGrabMode};
use dirs::cache_dir;
use crate::{asset_extractor::{get_builtin_parts, get_workshop_parts}, cam_movement::{advance_physics, grab_mouse, handle_input, interpolate_rendered_transform, move_player, spawn_player, spawn_text}, editor_ui::get_base_part_entity};
use crate::parsing::{load_save, AdjustableHull, BasePart, HasBasePart, Part};
use core::f32;
use std::{fs::create_dir_all, path::{Path, PathBuf}};


#[derive(Resource)]
pub struct PartRegistry {
    pub parts: HashMap<i32,PartData>
}

pub struct PartData {
    pub id: i32,
    pub part_name: String,
    pub part_description: String,
    pub builder_class: i32, //-1 is not found/invalid
    pub weapon_type: i32, //-1 is not found/invalid
    pub nation: u32,
    pub armor: i32,
    pub density: f32,
    pub price: i32,
    pub volume: f32,
    pub center: Vec3,
    //collier is half lengths
    pub collider: Vec3,
    pub weapon: Option<WeaponData>,
    pub model: PathBuf,
    pub thumbnail: Option<PathBuf>
}
pub struct WeaponData {
}

pub fn register_all_parts(
    mut part_registry: ResMut<PartRegistry>
){
    let cache_folder = cache_dir().unwrap().join("naval_fart");
    let steam_folder = PathBuf::from("/home/kidofcubes/.local/share/Steam/");
    let workshop_folder = steam_folder.join("steamapps").join("workshop").join("content").join("842780");
    let game_folder = steam_folder.join("steamapps").join("common").join("NavalArt");
    create_dir_all(&cache_folder).unwrap();

    let workshop_parts = get_workshop_parts(&workshop_folder, &cache_folder);
    for workshop_port in workshop_parts {
        part_registry.parts.insert(workshop_port.id,workshop_port);
    }

    let builtin_parts = get_builtin_parts(&game_folder, &cache_folder);
    for builtin_part in builtin_parts {
        part_registry.parts.insert(builtin_part.id,builtin_part);
    }

    println!("all registered parts is {:?}",part_registry.parts.keys());

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
        let angle = f32::consts::TAU*((i as f32)/(resolution as f32));
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
#[derive(Component, Debug, Clone)]
pub struct BasePartMeshes {
    pub meshes: Vec<Entity>,
}

#[derive(Component, Debug, Copy, Clone)]
pub struct BasePartMesh{
    pub base_part: Entity,
}

pub fn on_part_meshes_init(
    mesh_query: Query<Entity, Added<Mesh3d>>,
    base_part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    mut base_part_meshes_query: Query<&mut BasePartMeshes>,
    layer_query: Query<&RenderLayers>,
    mut commands: Commands,
){
    let mut temp = bevy::utils::HashMap::new();
    for entity in &mesh_query {
        if let Some(base_part_entity) = get_base_part_entity(&parent_query, &base_part_query, entity) {
            if let Ok(base_part_meshes) = &mut base_part_meshes_query.get_mut(base_part_entity) {
                base_part_meshes.meshes.push(entity);
            }else{
                temp.try_insert(base_part_entity, BasePartMeshes {meshes:Vec::new()});
                temp.get_mut(&base_part_entity).unwrap().meshes.push(entity);
            }
            commands.get_entity(entity).unwrap().insert(BasePartMesh{base_part:base_part_entity});
            if let Ok(layer) = layer_query.get(base_part_entity) {
                commands.get_entity(entity).unwrap().insert(layer.clone());
            }
            commands.get_entity(entity).unwrap().insert(BasePartMesh{base_part:base_part_entity});
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
) {
    // let mut entity: EntityCommands = commands.spawn((
    //     
    // ));
    entity.insert((
        base_part_to_bevy_transform(part.base_part()),
        part.base_part().clone()
    ));


    match part {
        Part::Normal(base_part) => entity.insert(*base_part),
        Part::AdjustableHull(base_part, adjustable_hull) => entity.insert((*base_part, *adjustable_hull)),
        Part::Turret(base_part, turret) => entity.insert((*base_part, *turret)),
    };

    if let Part::AdjustableHull(base_part, adjustable_hull) = part {
        let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList,RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
        
        generate_adjustable_hull_mesh(
            &mut mesh,
            adjustable_hull
        );
        
        entity.insert((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(base_part.color))
        ));
    }else{
        //let asset_path = AssetPath::from(format!("/home/kidofcubes/Downloads/AssetRipper_linux_x64/NavalArtOut/PrefabHierarchyObject/{}.glb",part.base_part().id));
         // println!("looking for part with id {:?}",&part.base_part().id);
         // println!("loaded parts are {:?}",part_registry.parts.keys());
        let asset_path = AssetPath::from(part_registry.parts.get(&part.base_part().id).unwrap().model.clone());
        let mut handle = asset_server.get_handle(&asset_path);
        if handle.is_none() {
            handle = Some(asset_server.load(
                 GltfAssetLabel::Scene(0).from_asset(
                     asset_path
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
}

fn initalize_part_scene(trigger: Trigger<HierarchyEvent>, children: Query<&Children>){
    println!("TRIGGERED EVENT FOR {:?} which was {:?}",trigger.entity(),trigger.event());
}

pub fn base_part_to_bevy_transform(base_part: &BasePart) -> Transform{

    let mut transform = Transform::from_xyz(
                base_part.position.x,
                base_part.position.y,
                base_part.position.z,
            ).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                base_part.rotation.y.to_radians(),
                base_part.rotation.x.to_radians(),
                base_part.rotation.z.to_radians(),
            ))
            .with_scale(base_part.scale.abs()); //absed because yeah

    let new_quat = Quat::from_xyzw(
        -transform.rotation.x, 
        transform.rotation.y, 
        transform.rotation.z, 
        -transform.rotation.w
    );
    transform.translation.x = -transform.translation.x;
    transform = transform.with_rotation(new_quat);
    return transform;
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
