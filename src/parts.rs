use std::{fmt::Display, iter::once, ops::Deref, path::Path};
use bevy::{asset::{AssetPath, RenderAssetUsages}, hierarchy::HierarchyEvent, log::tracing_subscriber::filter::combinator::And, prelude::*, reflect::List, render::{mesh::Indices, view::RenderLayers}, utils::HashMap};
use dirs::cache_dir;
use enum_collections::{EnumMap, Enumerated};
use crate::{asset_extractor::{get_builtin_parts, get_workshop_parts}, editor::Selected, editor_ui::{get_base_part_entity, Language}, editor_utils::{set_adjustable_hull_width, with_corner_adjacent_adjustable_hulls, AdjHullSide}, parsing::Turret};
use crate::parsing::{AdjustableHull, BasePart, Part};
use core::f32;
use std::{fs::create_dir_all, path::PathBuf};


#[derive(Resource)]
pub struct PartRegistry {
    pub parts: HashMap<i32,PartData>
}

#[derive(Debug, Clone)]
pub struct MultiLangString {
    pub texts: EnumMap<Language,Option<String>,{Language::SIZE}>,
}
impl Default for MultiLangString {
    fn default() -> Self {
        MultiLangString { texts: EnumMap::new_option() }
    }
}
impl MultiLangString {
    pub fn of(lang: Language, text: String) -> Self {
        MultiLangString::default().with(lang,text)
    }

    pub fn with(mut self, lang: Language, text: String) -> Self {
        self.texts[lang] = Some(text);
        return self;
    }
    pub fn get(&self, lang: Language) -> &str {
        return self.texts[lang].as_deref().unwrap_or(self.get_fallback());
    }
    pub fn get_fallback(&self) -> &str {
        for lang in Language::VARIANTS {
            if let Some(text) = self.texts[*lang].as_ref() {
                return text;
            }
        }
        return "NO TEXT FOUND";
    }
}

#[derive(Debug, Clone)]
pub struct PartData {
    pub id: i32,
    pub part_name: MultiLangString,
    pub part_description: MultiLangString,
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

#[derive(Debug, Copy, Clone)]
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

pub fn on_part_meshes_init(
    mut mesh_query: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), Added<Mesh3d>>,
    base_part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    mut base_part_meshes_query: Query<&mut BasePartMeshes>,
    layer_query: Query<&RenderLayers>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
){
    let mut temp = bevy::utils::HashMap::new();
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

            entity.1.0 = materials.add(colored_part_material(base_part_query.get(base_part_entity).unwrap().color));
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


#[derive(Enumerated, PartialEq, Debug, Copy, Clone)]
pub enum PartAttributes {
    //BasePart
    Id,
    IgnorePhysics,
    PositionX,
    PositionY,
    PositionZ,
    RotationX,
    RotationY,
    RotationZ,
    ScaleX,
    ScaleY,
    ScaleZ,
    Color,
    Armor,
    //AdjustableHull
    Length,
    Height,
    FrontWidth,
    BackWidth,
    FrontSpread,
    BackSpread,
    TopRoundness,
    BottomRoundness,
    HeightScale,
    HeightOffset,
    //Turret
    ManualControl,
    Elevator
}
impl PartAttributes {
    pub fn is_number(&self) -> bool{
        match self {
            PartAttributes::Id => true,
            PartAttributes::IgnorePhysics => false,
            PartAttributes::Color => false,
            PartAttributes::ManualControl => false,
            _ => true
        }
    }
    pub fn is_adjustable_hull(&self) -> bool{
        match self {
            PartAttributes::Length => true,
            PartAttributes::Height => true,
            PartAttributes::FrontWidth => true,
            PartAttributes::BackWidth => true,
            PartAttributes::FrontSpread => true,
            PartAttributes::BackSpread => true,
            PartAttributes::TopRoundness => true,
            PartAttributes::BottomRoundness => true,
            PartAttributes::HeightScale => true,
            PartAttributes::HeightOffset => true,
            _ => false
        }
    }

    pub fn get_field(&self, base_part: &BasePart, adjustable_hull: Option<&AdjustableHull>, turret: Option<&Turret>) -> Option<String>{
        let mut string: Option<String> = None;

        string = match self {
            PartAttributes::Id => Some(base_part.id.to_string()),
            PartAttributes::IgnorePhysics => Some(base_part.ignore_physics.to_string()),
            PartAttributes::PositionX => Some(base_part.position.x.to_string()),
            PartAttributes::PositionY => Some(base_part.position.y.to_string()),
            PartAttributes::PositionZ => Some(base_part.position.z.to_string()),
            PartAttributes::RotationX => Some(base_part.rotation.x.to_string()),
            PartAttributes::RotationY => Some(base_part.rotation.y.to_string()),
            PartAttributes::RotationZ => Some(base_part.rotation.z.to_string()),
            PartAttributes::ScaleX => Some(base_part.scale.x.to_string()),
            PartAttributes::ScaleY => Some(base_part.scale.y.to_string()),
            PartAttributes::ScaleZ => Some(base_part.scale.z.to_string()),
            PartAttributes::Color => Some(base_part.color.to_srgba().to_hex()),
            PartAttributes::Armor => Some(base_part.armor.to_string()),
            _ => None
        };
        if string.is_some() { return string };


        if let Some(adjustable_hull) = adjustable_hull {
            string = match self {
                PartAttributes::Length => Some(adjustable_hull.length.to_string()),
                PartAttributes::Height => Some(adjustable_hull.height.to_string()),
                PartAttributes::FrontWidth => Some(adjustable_hull.front_width.to_string()),
                PartAttributes::BackWidth => Some(adjustable_hull.back_width.to_string()),
                PartAttributes::FrontSpread => Some(adjustable_hull.front_spread.to_string()),
                PartAttributes::BackSpread => Some(adjustable_hull.back_spread.to_string()),
                PartAttributes::TopRoundness => Some(adjustable_hull.top_roundness.to_string()),
                PartAttributes::BottomRoundness => Some(adjustable_hull.bottom_roundness.to_string()),
                PartAttributes::HeightScale => Some(adjustable_hull.height_scale.to_string()),
                PartAttributes::HeightOffset => Some(adjustable_hull.height_offset.to_string()),
                _ => None
            };
            if string.is_some() { return string };
        }


        if let Some(turret) = turret {
            string = match self {
                PartAttributes::ManualControl=> Some(turret.manual_control.to_string()),
                PartAttributes::Elevator => Some(turret.elevator.unwrap_or(0.0).to_string()),
                _ => None
            };
            if string.is_some() { return string };
        }

        return string;
    }
    pub fn set_field(&self, base_part: Option<&mut BasePart>, adjustable_hull: Option<&mut AdjustableHull>, turret: Option<&mut Turret>, text: &str) -> Result<(),Box<dyn std::error::Error>>{
        if let Some(base_part) = base_part {
            match self {
                PartAttributes::Id => {base_part.id = text.parse()?},
                PartAttributes::IgnorePhysics => {base_part.id = text.parse()?},
                PartAttributes::PositionX => {base_part.position.x = text.parse()?},
                PartAttributes::PositionY => {base_part.position.y = text.parse()?},
                PartAttributes::PositionZ => {base_part.position.z = text.parse()?},
                PartAttributes::RotationX => {base_part.rotation.x = text.parse()?},
                PartAttributes::RotationY => {base_part.rotation.y = text.parse()?},
                PartAttributes::RotationZ => {base_part.rotation.z = text.parse()?},
                PartAttributes::ScaleX => {base_part.scale.x = text.parse()?},
                PartAttributes::ScaleY => {base_part.scale.y = text.parse()?},
                PartAttributes::ScaleZ => {base_part.scale.z = text.parse()?},
                PartAttributes::Color => {base_part.color = Color::Srgba(Srgba::hex(text)?)},
                PartAttributes::Armor => {base_part.armor = text.parse()?},
                _ => {}
            }
        }

        if let Some(adjustable_hull) = adjustable_hull {
            match self {
                PartAttributes::Length => adjustable_hull.length = text.parse()?,
                PartAttributes::Height => adjustable_hull.height = text.parse()?,
                PartAttributes::FrontWidth => adjustable_hull.front_width = text.parse()?,
                PartAttributes::BackWidth => adjustable_hull.back_width = text.parse()?,
                PartAttributes::FrontSpread => adjustable_hull.front_spread = text.parse()?,
                PartAttributes::BackSpread => adjustable_hull.back_spread = text.parse()?,
                PartAttributes::TopRoundness => adjustable_hull.top_roundness = text.parse()?,
                PartAttributes::BottomRoundness => adjustable_hull.bottom_roundness = text.parse()?,
                PartAttributes::HeightScale => adjustable_hull.height_scale = text.parse()?,
                PartAttributes::HeightOffset => adjustable_hull.height_offset = text.parse()?,
                _ => {}
            };
        }

        if let Some(turret) = turret{
            match self {
                PartAttributes::ManualControl => turret.manual_control = text.parse()?,
                PartAttributes::Elevator => turret.elevator = Some(text.parse()?),
                _ => {}
            }
        };

        return Ok(());
    }

    pub fn smart_set_field(
        &self,
        all_parts: &mut Query<(&mut BasePart, Option<&mut AdjustableHull>, Option<&mut Turret>, Entity)>,
        selected_parts: &Query<Entity, With<Selected>>,
        part_registry: &Res<PartRegistry>,
        value: &str
    ){
        let mut all_colliders: Vec<(Transform,AdjustableHull)> = Vec::new();
        let mut all_colliders_entities: Vec<Entity>= Vec::new();

        let mut all_orig_adjustable_hulls: HashMap<Entity,AdjustableHull> = HashMap::new();

        if self.is_adjustable_hull() && self.is_number() {
            for part in all_parts.iter() {
                let Some(adjustable_hull) = part.1.as_deref() else {continue;};
                all_colliders.push((get_collider(&part.0, Some(adjustable_hull), part_registry.parts.get(&part.0.id).unwrap()),(adjustable_hull.clone())));
                all_colliders_entities.push(part.3);
                all_orig_adjustable_hulls.insert(part.3,adjustable_hull.clone());
            }
        }

        let selff = *self;


        for selected_entity in selected_parts {
            let mut selected_part = all_parts.get_mut(selected_entity).unwrap();
            //let original_adjustable_hull = selected_part.1.as_ref().map(|x| x.as_ref().clone());

            selff.set_field(Some(&mut selected_part.0), selected_part.1.as_deref_mut(), selected_part.2.as_deref_mut(), value);

            //println!("is it the thing {:?} and {:?}", editor_data.edit_near, selff.is_adjustable_hull()); 

            if selff.is_number() && selff.is_adjustable_hull() && selected_part.1.is_some() {
                let Ok(value) = value.parse::<f32>() else {continue;};
                let value = &value;
                let Some(origin_hull) = selected_part.1.as_deref() else {return;};
                let origin_hull = origin_hull.clone();

                let original_adjustable_hull = 
                    //all_colliders[all_colliders_entities.iter().position(|x| x.clone()==selected_entity).unwrap()].1;
                    all_orig_adjustable_hulls.get(&selected_entity.clone()).unwrap();
                let collider = get_collider(selected_part.0.deref(), Some(&original_adjustable_hull), part_registry.parts.get(&selected_part.0.id).unwrap());
                println!("THE ADJUSTABLE HULL TO CHECK IS {:?}",origin_hull);
                let adjacents = with_corner_adjacent_adjustable_hulls((&collider,&original_adjustable_hull), &all_colliders/* , &mut gizmos_debug */);





                let changed_front = selff != PartAttributes::BackWidth && selff != PartAttributes::BackSpread;
                let changed_back = selff != PartAttributes::FrontWidth && selff != PartAttributes::FrontSpread;
                let changed_top = 
                    selff == PartAttributes::FrontWidth ||
                    selff == PartAttributes::BackWidth ||
                    selff == PartAttributes::FrontSpread ||
                    selff == PartAttributes::BackSpread ||

                    selff == PartAttributes::TopRoundness;
                let changed_bottom = 
                    selff == PartAttributes::FrontWidth ||
                    selff == PartAttributes::BackWidth ||
                    selff == PartAttributes::BottomRoundness;




                for origin_side in AdjHullSide::VARIANTS{
                    let Some(adjacent) = adjacents[*origin_side] else {continue;};
                    // gizmos_debug.to_display.push(GizmoDisplay::Cuboid(all_colliders[adjacent.0].0, Color::srgb_u8(255, 0, 255)));
                    // gizmos_debug.to_display.push(GizmoDisplay::Arrow(collider.translation,collider.translation+cuboid_face_normal(&collider, &adjacent.0), Color::srgb_u8(255, 0, 255)));

                    let mut adjacent_hull = all_parts.get_mut(all_colliders_entities[adjacent.0]).unwrap().1.unwrap();
                    let hori_flipped = adjacent.1;
                    let vert_flipped = adjacent.2;

                    //println!("we are now fixing up {:?} which is {:?} and {:?}",adjacent_hull,hori_flipped,vert_flipped);
                    

                    match origin_side {
                        AdjHullSide::Front => {
                            if !changed_front {continue;}

                            match selff {
                                PartAttributes::FrontWidth=>{
                                    set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &!vert_flipped, value);
                                    set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &vert_flipped, &(value+origin_hull.front_spread));
                                }
                                PartAttributes::FrontSpread=>{set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &vert_flipped, &(value+origin_hull.front_width));}
                                PartAttributes::TopRoundness|PartAttributes::BottomRoundness=>{if changed_top^vert_flipped {adjacent_hull.top_roundness=*value;}else{adjacent_hull.bottom_roundness=*value;}}
                                PartAttributes::Height=>{adjacent_hull.height=*value;}
                                _ => {}
                            }
                        }
                        AdjHullSide::FrontTop => {
                            if !(changed_front && changed_top) {continue;}

                            match selff {
                                PartAttributes::FrontWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &!vert_flipped, &(value+origin_hull.front_spread));}
                                PartAttributes::FrontSpread=>{set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &!vert_flipped, &(value+origin_hull.front_width));}
                                _ => {}
                            }
                        }
                        AdjHullSide::FrontBottom => {
                            if !(changed_front && changed_bottom) {continue;}

                            match selff {
                                PartAttributes::FrontWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &vert_flipped, value);}
                                _ => {}
                            }
                        }


                        AdjHullSide::Back => {
                            if !changed_back {continue;}

                            match selff {
                                PartAttributes::BackWidth=>{
                                    set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &!vert_flipped, value);
                                    set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &vert_flipped, &(value+origin_hull.back_spread));
                                }
                                PartAttributes::BackSpread=>{set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &vert_flipped, &(value+origin_hull.back_width));}
                                PartAttributes::TopRoundness|PartAttributes::BottomRoundness=>{if changed_top^vert_flipped {adjacent_hull.top_roundness=*value;}else{adjacent_hull.bottom_roundness=*value;}}
                                PartAttributes::Height=>{adjacent_hull.height=*value;}
                                _ => {}
                            }
                        }
                        AdjHullSide::BackTop => {
                            if !(changed_back && changed_top) {continue;}

                            match selff {
                                PartAttributes::BackWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &!vert_flipped, &(value+origin_hull.back_spread));}
                                PartAttributes::BackSpread=>{set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &!vert_flipped, &(value+origin_hull.back_width));}
                                _ => {}
                            }
                        }
                        AdjHullSide::BackBottom => {
                            if !(changed_back && changed_bottom) {continue;}

                            match selff {
                                PartAttributes::BackWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &vert_flipped, value);}
                                _ => {}
                            }
                        }


                        AdjHullSide::Top => {
                            if !changed_top {continue;}

                            match selff {
                                PartAttributes::FrontSpread=>{set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &!vert_flipped, &(value+origin_hull.front_width));}
                                PartAttributes::BackSpread=>{set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &!vert_flipped, &(value+origin_hull.back_width));}
                                PartAttributes::FrontWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &!vert_flipped, &(value+origin_hull.front_spread));}
                                PartAttributes::BackWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &!vert_flipped, &(value+origin_hull.back_spread));}
                                _ => {}
                            }
                        }
                        AdjHullSide::Bottom => {
                            if !changed_bottom {continue;}

                            match selff {
                                PartAttributes::FrontWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &hori_flipped, &vert_flipped, &value);}
                                PartAttributes::BackWidth=>{set_adjustable_hull_width(&mut adjacent_hull, &!hori_flipped, &vert_flipped, &value);}
                                _ => {}
                            }
                        }

                        _ => {}
                    }
                }
            }
        }
    }
}

impl Display for PartAttributes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{}",match self {
            PartAttributes::Id => "Id",
            PartAttributes::IgnorePhysics => "IgnorePhysics",
            PartAttributes::PositionX => "Position|位置 X",
            PartAttributes::PositionY => "Position|位置 Y",
            PartAttributes::PositionZ => "Position|位置 Z",
            PartAttributes::RotationX => "Rotation|旋转 X",
            PartAttributes::RotationY => "Rotation|旋转 Y",
            PartAttributes::RotationZ => "Rotation|旋转 Z",
            PartAttributes::ScaleX => "Scale|尺度 X",
            PartAttributes::ScaleY => "Scale|尺度 Y",
            PartAttributes::ScaleZ => "Scale|尺度 Z",
            PartAttributes::Color => "Color|颜色",
            PartAttributes::Armor => "Armor|装甲",
            PartAttributes::Length => "Length|长度",
            PartAttributes::Height => "Height|高度",
            PartAttributes::FrontWidth => "ForwardWidth|前段宽度",
            PartAttributes::BackWidth => "BackwardWidth|后段宽度",
            PartAttributes::FrontSpread => "ForwardSpread|前段扩散",
            PartAttributes::BackSpread => "BackwardSpread|后段扩散",
            PartAttributes::TopRoundness => "TopRoundness|上表面弧度",
            PartAttributes::BottomRoundness => "BottomRoundness|下表面弧度",
            PartAttributes::HeightScale => "HeightScale|高度缩放",
            PartAttributes::HeightOffset => "HeightOffset|高度偏移",
            PartAttributes::ManualControl => "ManualControl",
            PartAttributes::Elevator => "Elevator",
        })
    }
}
