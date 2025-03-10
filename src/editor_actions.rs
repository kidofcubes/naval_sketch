use core::f32;
use std::ops::Deref;

use bevy::{app::App, asset::{AssetServer, Assets}, color::Color, ecs::{event::Event, system::Commands}, math::{Dir3, EulerRot, Isometry3d, Quat, Vec3}, pbr::StandardMaterial, picking::mesh_picking::ray_cast::{MeshRayCast, RayCastSettings}, prelude::{Camera, Entity, Gizmos, GlobalTransform, Query, Res, ResMut, Single, Transform, Trigger, With}, render::mesh::Mesh, state::commands, utils::HashMap, window::Window};
use enum_collections::Enumerated;

use crate::{cam_movement::EditorCamera, editor::{DebugGizmo, EditorData, Selected}, editor_ui::{Hovered, Language, PropertiesDisplayData}, editor_utils::{arrow, cuboid_face, cuboid_face_normal, cuboid_scale, get_nearby, round_to_axis, set_adjustable_hull_width, simple_closest_dist, to_touch, with_corner_adjacent_adjustable_hulls, AdjHullSide}, parsing::{AdjustableHull, BasePart, Part, Turret}, parts::{base_part_to_bevy_transform, bevy_quat_to_unity, bevy_to_unity_translation, get_collider, place_part, unity_to_bevy_quat, unity_to_bevy_translation, PartAttributes, PartRegistry}};


#[derive(Event)]
pub enum EditorActionEvent {
    MoveRelativeDir {vector: Vec3, mult: f32},
    SmartMoveRelativeDir {dir: Dir3, mult: f32},
    SwitchSelectedAttribute {offset: i32, do_loop: bool},
    SetAttribute {attribute: Option<PartAttributes>, value: String},
    SetEditorSetting {change: EditorSettingChange},
    SpawnNewPart {part_id: i32, selected: bool, part: Option<Part>},
    Copy {},
    Paste {selected: bool},
}

pub fn add_actions(app: &mut App) {
    app.add_observer(move_selected_relative_dir);
    app.add_observer(smart_move_selected_relative_dir);
    app.add_observer(switch_selected_attribute);
    app.add_observer(modify_selected_attribute);
    app.add_observer(set_editor_settings);
    app.add_observer(spawn_new_part);
    app.add_observer(copy);
    app.add_observer(paste);
}

pub fn modify_selected_attribute(
    trigger: Trigger<EditorActionEvent>,
    editor_data: Res<EditorData>,
    part_registry: Res<PartRegistry>,
    display_properties: Res<PropertiesDisplayData>,
    mut gizmos_debug: ResMut<DebugGizmo>,
    mut all_parts: Query<(&mut BasePart, Option<&mut AdjustableHull>, Option<&mut Turret>, Entity)>,
    selected_parts: Query<Entity, With<Selected>>,
    gizmo: Gizmos,
){
    let EditorActionEvent::SetAttribute{attribute, value} = trigger.event() else {return;};
    let attribute = if attribute.is_some() {attribute.unwrap()}else{display_properties.selected};

    gizmos_debug.to_display.clear();
    if editor_data.group_edit_attributes {
        if attribute.is_number() {
            let Ok(value) = value.parse::<f32>() else {return;};

            if 
                (
                attribute == PartAttributes::RotationX ||
                attribute == PartAttributes::RotationY ||
                attribute == PartAttributes::RotationZ
                ) && editor_data.latest_selected.is_some()
            { //rotate around most recently selected
                let latest_selected = editor_data.latest_selected.unwrap();

                let mut rotation_vector = Vec3::new(0.0,0.0,0.0);
                if attribute == PartAttributes::RotationX {
                    rotation_vector.x = value;
                }else if attribute == PartAttributes::RotationY {
                    rotation_vector.y = value;
                }else{
                    rotation_vector.z = value;
                }

                let rotation = unity_to_bevy_quat(&rotation_vector);
                let origin = unity_to_bevy_translation(&all_parts.get(latest_selected).unwrap().0.position);



                for selected_entity in &selected_parts {
                    let mut selected_part = all_parts.get_mut(selected_entity).unwrap();
                    let mut new_transform = base_part_to_bevy_transform(&selected_part.0);
                    new_transform.rotate_around(origin, rotation);
                    selected_part.0.position = bevy_to_unity_translation(&new_transform.translation);
                    selected_part.0.rotation = bevy_quat_to_unity(&new_transform.rotation);
                    
                }
                


            } else {
                let mut average = 0.0;
                for selected_entity in &selected_parts {
                    let selected_part = all_parts.get_mut(selected_entity).unwrap();
                    average += attribute.get_field(selected_part.0.deref(), selected_part.1.as_deref(), selected_part.2.as_deref()).unwrap().parse::<f32>().unwrap();
                }
                average /= selected_parts.iter().len() as f32;
                let difference = value-average;

                for selected_entity in &selected_parts {
                    let mut selected_part = all_parts.get_mut(selected_entity).unwrap();
                    let orig = attribute.get_field(selected_part.0.deref(), selected_part.1.as_deref(), selected_part.2.as_deref()).unwrap().parse::<f32>().unwrap();

                    attribute.set_field(Some(selected_part.0.as_mut()), selected_part.1.as_deref_mut(), selected_part.2.as_deref_mut(), &(orig+difference).to_string());
                }
            }
        }else{
            for selected_entity in &selected_parts {
                let mut selected_part = all_parts.get_mut(selected_entity).unwrap();
                attribute.set_field(Some(selected_part.0.as_mut()), selected_part.1.as_deref_mut(), selected_part.2.as_deref_mut(), &value);
            }
        }
    }else{
        if editor_data.edit_near {
            attribute.smart_set_field(&mut all_parts, &selected_parts, &part_registry, &value);
        }else{
            for selected_entity in &selected_parts {
                let mut selected_part = all_parts.get_mut(selected_entity).unwrap();
                attribute.set_field(Some(selected_part.0.as_mut()), selected_part.1.as_deref_mut(), selected_part.2.as_deref_mut(), &value);
            }
        }
    }

    // let mut all_colliders: Vec<(Transform,AdjustableHull)> = Vec::new();
    // let mut all_colliders_entities: Vec<Entity>= Vec::new();
    //
    // let mut all_orig_adjustable_hulls: HashMap<Entity,AdjustableHull> = HashMap::new();
    //
    // if editor_data.edit_near && display_properties.selected.is_adjustable_hull() {
    //     for part in &all_parts {
    //         let Some(adjustable_hull) = part.1.as_deref() else {continue;};
    //         all_colliders.push((get_collider(part.0, Some(adjustable_hull), part_registry.parts.get(&part.0.id).unwrap()),(adjustable_hull.clone())));
    //         all_colliders_entities.push(part.3);
    //         all_orig_adjustable_hulls.insert(part.3,adjustable_hull.clone());
    //     }
    // }
    //
    //
    // if display_properties.selected.is_number() {
    //     for selected_entity in &selected_parts {
    //         let mut selected_part = all_parts.get_mut(selected_entity).unwrap();
    //         //let original_adjustable_hull = selected_part.1.as_ref().map(|x| x.as_ref().clone());
    //
    //         
    //     }
    // }
}


pub fn switch_selected_attribute(
    trigger: Trigger<EditorActionEvent>,
    mut display_properties: ResMut<PropertiesDisplayData>,
){
    let EditorActionEvent::SwitchSelectedAttribute{offset, do_loop} = trigger.event() else {return;};

    let variants: Vec<&PartAttributes> = PartAttributes::VARIANTS.iter().collect();
    let mut position = variants.iter().position(|&a| *a == display_properties.selected).unwrap();
    if *do_loop {
        position=(((position as i32+offset) as i32).rem_euclid(variants.len() as i32)) as usize;
    }else{
        position = ((position as i32+offset).clamp(0,variants.len() as i32-1)) as usize;
    }
    display_properties.selected = *variants[position];


    //display_properties.selected
}





pub fn move_selected_relative_dir(
    trigger: Trigger<EditorActionEvent>,
    selected: Query<Entity, With<Selected>>,
    mut all_parts: Query<(&mut BasePart, Option<&mut AdjustableHull>)>,
    camera_transform: Single<(&Camera, &GlobalTransform, &EditorCamera)>,
){
    let EditorActionEvent::MoveRelativeDir{vector, mult} = trigger.event() else {return;};
    let mut rot = camera_transform.1.rotation().to_euler(EulerRot::XYZ);

    rot.0 = (rot.0/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.1 = (rot.1/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.2 = (rot.2/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;

    let translation = unity_to_bevy_translation(
        &Quat::from_euler(EulerRot::XYZ, rot.0, rot.1, rot.2).mul_vec3(*vector)
    ) * mult;

    for selected_part in &selected {
        all_parts.get_mut(selected_part).unwrap().0.position+=translation;
    }
}


//need to figure out what to do when multiple parts

pub fn smart_move_selected_relative_dir(
    trigger: Trigger<EditorActionEvent>,
    selected: Query<Entity, With<Selected>>,
    //camera_transform: &Query<&Transform, With<Camera3d>>,
    camera_query: Single<(&Camera, &GlobalTransform, &EditorCamera)>,
    mut all_parts: Query<(&mut BasePart,Option<&mut AdjustableHull>)>,
    part_registry: Res<PartRegistry>,
    mut gizmo: Gizmos,
){

    let EditorActionEvent::SmartMoveRelativeDir{dir, mult: multiplier} = trigger.event() else {return;};

    let mut other_parts = Vec::new();
    for part in all_parts.iter() {
        other_parts.push(get_collider(part.0, part.1, part_registry.parts.get(&part.0.id).unwrap()))
    }

    for selected_entity in &selected {

        let mut selected = all_parts.get_mut(selected_entity).unwrap();

        let selected_bounding_box = get_collider(selected.0.deref(), selected.1.as_deref(), part_registry.parts.get(&selected.0.id).unwrap());

        let dir_nearbys = get_nearby(&selected_bounding_box, &other_parts,false,false /* ,&mut gizmos */);

        let mut possible_positions: HashMap<u8,Vec<f32>> = HashMap::new();
        for i in 0..6 as u8 {
            let selected_shared_face = cuboid_face(&selected_bounding_box,i);


            for nearby in dir_nearbys.get(&i).unwrap() {
                let nearby_transform = &other_parts[nearby.0];

                if simple_closest_dist(&selected_bounding_box, nearby_transform) > (1.0) {
                    continue;
                }
                //gizmo.cuboid(*nearby.0,Color::srgb_u8(0, 255, 255));

                let face = cuboid_face(nearby_transform, nearby.1);
                let mut dotted_dist = face.1-selected_shared_face.1;

                possible_positions.try_insert(i, Vec::new());
                possible_positions.get_mut(&i).unwrap().push((face.1-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()));
                possible_positions.get_mut(&i).unwrap().push((nearby_transform.translation-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()));

                possible_positions.try_insert((i+3)%6, Vec::new());
                possible_positions.get_mut(&((i+3)%6)).unwrap().push(((face.1-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()))*-1.0);
                possible_positions.get_mut(&((i+3)%6)).unwrap().push(((nearby_transform.translation-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()))*-1.0);


                // println!("the diff is {:?}",((face.1-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize())));
                // println!("for {:?}-{:?} is {:?} and then dot {:?}",face.1,selected_bounding_box.translation,(face.1-selected_bounding_box.translation),selected_shared_face.0.0.normalize());
                dotted_dist = dotted_dist - (dotted_dist.dot(selected_shared_face.0.0.normalize())*selected_shared_face.0.0.normalize());
                
                    
                // cuboid_face_normal(&selected_bounding_box, &i)*
                // ((nearby.0.translation-selected_bounding_box.translation).dot(cuboid_face_normal(&selected_bounding_box, &i)));

                let face = cuboid_face(nearby_transform, (nearby.1+(0*3))%6);
                //let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                thing.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, face.0.0.normalize());

                //gizmo.rect(thing, Vec2::ONE*5.0, Color::srgb_u8(255, 255, 0));


            }
        }

        let dir = round_to_axis(&selected_bounding_box, &Dir3::new_unchecked(camera_query.1.rotation().mul_vec3(**dir)));
        // let mut distances = possible_positions.get(&dir).unwrap_or(&Vec::new()).clone();
        // distances.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let max_moves = *multiplier as usize;

        // let mut moved = 0;
        // println!("distances is {:?}",distances);
        // let mut best_dist = f32::MAX;
        // let mut the_dist = f32::MAX;

        let mut all_distances = Vec::new();
        for pos in possible_positions.get(&dir).unwrap_or(&Vec::new()){
            // if pos >0.0 {
            //     gizmo.arrow(
            //         selected_bounding_box.translation,
            //         selected_bounding_box.translation+(pos*cuboid_face_normal(&selected_bounding_box, &dir)),
            //         Color::srgb_u8(0, 255, 255)
            //     );
            // }
            for possible_pos in [(pos-(cuboid_scale(&selected_bounding_box,&dir)/2.0)),*pos,pos+(cuboid_scale(&selected_bounding_box,&dir)/2.0)] {
                //println!("a possiblepos for {:?} is {:?}",pos,possible_pos);
                gizmo.sphere(Isometry3d::from_translation(selected_bounding_box.translation+(possible_pos*cuboid_face_normal(&selected_bounding_box, &dir))), 0.5, Color::srgb_u8(255, 255, 0));
                if possible_pos > 0.00001 {
                    all_distances.push(possible_pos);
                }
                // if possible_pos < best_dist && possible_pos > 0.00001 {
                //     best_dist = possible_pos; 
                //     the_dist = pos;
                // }
            }
        }
        all_distances.sort_by(|a, b| a.partial_cmp(b).unwrap());

        if !all_distances.is_empty() {
            //println!("pos is {:?} and moved is {:?}",best_dist,(best_dist*cuboid_face_normal(&selected_bounding_box, &dir)));
            //moved=true;

            let best_dist = all_distances[max_moves.min(all_distances.len()-1)];

            arrow(&mut gizmo,
                selected_bounding_box.translation-cuboid_face(&selected_bounding_box,dir).0.0,
                best_dist*cuboid_face_normal(&selected_bounding_box, &dir),
                Color::srgb_u8(0, 255, 255)
            );
            arrow(&mut gizmo,
                selected_bounding_box.translation,
                best_dist*cuboid_face_normal(&selected_bounding_box, &dir),
                Color::srgb_u8(0, 255, 255)
            );
            arrow(&mut gizmo,
                selected_bounding_box.translation+cuboid_face(&selected_bounding_box,dir).0.0,
                best_dist*cuboid_face_normal(&selected_bounding_box, &dir),
                Color::srgb_u8(0, 255, 255)
            );
            gizmo.sphere(Isometry3d::from_translation(selected_bounding_box.translation+(best_dist*cuboid_face_normal(&selected_bounding_box, &dir))), 1.0, Color::srgb_u8(255, 0, 255));
            
            selected.0.position+=bevy_to_unity_translation(&(best_dist*cuboid_face_normal(&selected_bounding_box, &dir)));
        }


        // for pair in possible_positions.iter() {
        //     for num in pair.1 {
        //         gizmo.arrow(
        //             selected_bounding_box.translation,
        //             selected_bounding_box.translation+(num*cuboid_face_normal(&selected_bounding_box, pair.0)),
        //             Color::srgb_u8(0, 255, 255)
        //         );
        //     }
        //
        // }
    }
}


#[derive(Default)]
pub struct EditorSettingChange {
    pub floating: Option<bool>,
    pub edit_near: Option<bool>,
    pub language: Option<Language>,
}

pub fn set_editor_settings(
    trigger: Trigger<EditorActionEvent>,
    mut editor_data: ResMut<EditorData>,
){
    let EditorActionEvent::SetEditorSetting{change} = trigger.event() else {return;};
    if let Some(value) = change.floating { editor_data.floating = value; };
    if let Some(value) = change.edit_near { editor_data.edit_near = value; };
    if let Some(value) = change.language{ editor_data.language = value; };
    
}


pub fn spawn_new_part(
    trigger: Trigger<EditorActionEvent>,
    mut editor_data: ResMut<EditorData>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    part_registry: Res<PartRegistry>,
    camera_query: Single<(&Camera, &GlobalTransform, &EditorCamera)>,
    windows: Single<&Window>,
    hovered: Query<Entity, With<Hovered>>,
    selected_entities: Query<Entity, With<Selected>>,
    //mut ray_cast: MeshRayCast,
    part_query: Query<(&mut BasePart,Option<&AdjustableHull>,&mut Transform)>,
    mut commands: Commands,
){
    let EditorActionEvent::SpawnNewPart{part_id, selected, part} = trigger.event() else {return;};
    let mut part_data;
    if part.is_some() {
        part_data = part.clone().unwrap();
    } else {
        let Some(part_entry) = part_registry.parts.get(part_id) else {return;}; 

        if *part_id==0 { //adjustable hull
            part_data = Part::AdjustableHull(BasePart::default(),AdjustableHull::default());
            part_data.base_part_mut().id = *part_id;
        }else if part_entry.weapon_type == 7 { //basic part
            part_data = Part::Normal(BasePart::default());
            part_data.base_part_mut().id = *part_id;
        }else{ //weapon
            part_data = Part::Turret(BasePart::default(), Turret { manual_control: false, elevator: None });
            part_data.base_part_mut().id = *part_id;
        }
    };

    let (camera, camera_transform, _) = *camera_query;
    let camera_translation = camera_transform.translation();
    let Some(cursor_position) = windows.cursor_position() else { return; };
    // let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
    //     return;
    // };
    let mut dist:f32 = 100.0;
    // let dir = Dir3::new_unchecked((ray.direction).normalize());
    let dir = camera_transform.forward();

    //let Some((hit_entity, hit)) = ray_cast.cast_ray(ray, &RayCastSettings::default()).first() else {return;};
    
    if let Ok(hovered_entity) = hovered.get_single() {
        if let Ok(hit_entity_components) = part_query.get(hovered_entity) {
            let a_adj_hull = if let Part::AdjustableHull(_,adj_hull) = part_data { Some(adj_hull) }else{None};
            let mut a = get_collider(part_data.base_part(), a_adj_hull.as_ref(), part_registry.parts.get(part_id).unwrap());
            a.translation=camera_translation+part_registry.parts.get(part_id).unwrap().center;

            let b = get_collider(hit_entity_components.0, hit_entity_components.1, part_registry.parts.get(&hit_entity_components.0.id).unwrap());


            dist=dist.min(to_touch(&a, &b, dir/* , &mut gizmo */));
        }
    }

    let camera_translation = camera_transform.translation();
    part_data.base_part_mut().position=bevy_to_unity_translation(&(camera_translation+((*dir)*dist)));
    
    if *selected {
        for selected_entity in &selected_entities {
            commands.entity(selected_entity).remove::<Selected>();
        }
    }
    


    let mut placed_part = commands.spawn_empty();
    
    place_part(
        &mut meshes,
        &mut materials,
        &asset_server,
        &part_registry,
        &mut placed_part,
        &part_data,
    );
    

    if *selected {
        placed_part.insert(Selected{});
        editor_data.floating = true;
    }


}

pub fn copy(
    trigger: Trigger<EditorActionEvent>,
    mut editor_data: ResMut<EditorData>,
    selected_parts: Query<(&BasePart, Option<&AdjustableHull>, Option<&Turret>), With<Selected>>,
){
    let EditorActionEvent::Copy{} = trigger.event() else {return;};

    editor_data.clipboard.clear();
    for selected_part in &selected_parts {
        editor_data.clipboard.push(Part::from_optionals(selected_part));
    }
}


pub fn paste(
    trigger: Trigger<EditorActionEvent>,
    mut editor_data: ResMut<EditorData>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    part_registry: Res<PartRegistry>,
    camera_query: Single<(&Camera, &GlobalTransform, &EditorCamera)>,
    windows: Single<&Window>,
    hovered: Query<Entity, With<Hovered>>,
    part_query: Query<(&mut BasePart,Option<&AdjustableHull>,&mut Transform)>,
    selected_entities: Query<Entity, With<Selected>>,
    mut commands: Commands,
){
    let EditorActionEvent::Paste{selected} = trigger.event() else {return;};

    let mut translation: Vec3 = Vec3::new(0.0,0.0,0.0);
    for to_paste in &editor_data.clipboard {
        translation = translation + to_paste.base_part().position;
    }
    translation = translation / (editor_data.clipboard.len() as f32);

    let mut dist:f32 = 100.0;
    //let mut dir = camera_query.1.forward();

    let (camera, camera_transform, _) = *camera_query;
    let camera_translation = camera_transform.translation();

    let Some(cursor_position) = windows.cursor_position() else { return; };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };
    let dir = Dir3::new_unchecked((ray.direction).normalize());

    if editor_data.clipboard.len() == 1 {
        let single_paste = editor_data.clipboard.first().unwrap();


        if let Ok(hovered_entity) = hovered.get_single() {
            if let Ok(hit_entity_components) = part_query.get(hovered_entity) {
                let thing = single_paste.to_optionals();
                let mut a = get_collider(thing.0, thing.1, part_registry.parts.get(&single_paste.base_part().id).unwrap());
                a.translation=camera_translation+part_registry.parts.get(&single_paste.base_part().id).unwrap().center;

                let b = get_collider(hit_entity_components.0, hit_entity_components.1, part_registry.parts.get(&hit_entity_components.0.id).unwrap());

                dist=dist.min(to_touch(&a, &b, dir/* , &mut gizmo */));
            }
        }

    }

    

    translation = bevy_to_unity_translation(&(&camera_translation + (dir*dist))) - translation;

    if *selected {
        for selected_entity in &selected_entities {
            commands.entity(selected_entity).remove::<Selected>();
        }
    }

    for to_paste in &editor_data.clipboard {
        let mut to_place = to_paste.clone();
        to_place.base_part_mut().position += translation;
    
        let mut placed_part = commands.spawn_empty();
        place_part(
            &mut meshes,
            &mut materials,
            &asset_server,
            &part_registry,
            &mut placed_part,
            &to_place,
        );
        

        if *selected {
            placed_part.insert(Selected{});
        }

    }

    
}
