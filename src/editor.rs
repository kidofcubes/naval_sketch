use core::f32;
use std::{collections::VecDeque, iter::once, ops::{Deref, DerefMut}};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, Luminance}, ecs::{event::EventCursor, query}, gizmos::{gizmos, primitives::dim3::Plane3dBuilder}, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{bounding::{Aabb3d, AabbCast3d, Bounded3d, BoundedExtrusion, BoundingVolume}, Dir3, Direction3d, EulerRot, Isometry3d, Quat, Ray3d, Vec2, Vec3, Vec3A}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, GizmoConfig, GizmoPrimitive3d, Gizmos, GlobalTransform, HierarchyQueryExt, InfinitePlane3d, KeyCode, Local, Mesh3d, MeshRayCast, Out, Over, Parent, Plane3d, Pointer, PointerButton, Primitive3d, Query, RayCastSettings, Ref, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::{List, Map}, text::TextFont, ui::{BackgroundColor, Node, PositionType, Val}, utils::{default, HashMap}, window::Window};
use regex::Regex;
use smol_str::SmolStr;

use crate::{editor_ui::{on_click, on_hover, on_part_changed, on_unhover, render_gizmos, spawn_ui, update_command_text, update_selected, CommandDisplayData, Hovered}, editor_utils::{arrow, cuboid_face, cuboid_face_normal, cuboid_scale, get_nearby, get_relative_nearbys, round_to_axis, simple_closest_dist, to_touch}, parsing::{AdjustableHull, BasePart}, parts::{bevy_to_unity_translation, get_collider, unity_to_bevy_translation, BasePartMesh, PartRegistry}};

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.insert_resource(
            EditorData {
                action_history: Vec::new(),
                queued_commands: Vec::new(),
                floating: false,
            }
        );
        
    
        let mut command_tree = CommandTree::default();
        command_tree.add_command(b"w");
        command_tree.add_command(b"a");
        command_tree.add_command(b"s");
        command_tree.add_command(b"d");
        command_tree.add_command(b"W");
        command_tree.add_command(b"A");
        command_tree.add_command(b"S");
        command_tree.add_command(b"D");

        command_tree.add_command(b"f");
        command_tree.add_command(b"F");

        app.insert_resource(
            CommandData {
                command_history: VecDeque::new(),
                current_byte_index: 0,
                current_command: Vec::new(),
                commands: command_tree
            }
        );
        app.insert_resource(
            CommandDisplayData {
                mult: -1.0,
                font_size: -1.0,
                font_width: -1.0,
                input_text_display: None,
                flasher: None,
                history_text_display: None,
            }
        );
        app.add_observer(on_hover);
        app.add_observer(on_unhover);
        app.add_observer(on_click);
        app.add_systems(Startup, (spawn_ui));
        app.add_systems(Update, (
                translate_floatings,
                update_selected,
                on_part_changed,
                command_typing,
                update_command_text,
                execute_queued_commands,
                render_gizmos,
        ));
    }
}

#[derive(Resource)]
pub struct EditorData {
    action_history: Vec<Action>,
    queued_commands: Vec<QueuedCommand>, //use deque?
    floating: bool
}

#[derive(Resource)]
pub struct CommandData {
    pub command_history: VecDeque<String>,
    pub current_byte_index: usize,
    pub current_command: Vec<u8>,
    pub commands: CommandTree,
}

pub struct CommandTree {
    is_command: bool,
    continuations: HashMap<u8,Box<CommandTree>>
}

impl Default for CommandTree {
    fn default() -> Self {
        CommandTree {
            is_command: false,
            continuations: HashMap::new()

        }
    }
}

impl CommandTree {
    fn add_command(&mut self, command_string: &[u8]) {
        if command_string.is_empty() {
            self.is_command=true;
        }else{
            self.continuations.try_insert(command_string[0], Box::new(CommandTree::default()));
            self.continuations.get_mut(&command_string[0]).unwrap().add_command(&command_string[1..]);
        }
    }
    fn has_command(&self, command_string: &[u8]) -> (bool, bool){
        if command_string.is_empty() {
            return (true, self.is_command);
        }
        if let Some(next) = self.continuations.get(&command_string[0]) {
            return next.has_command(&command_string[1..]);
        }
        return (false, false);
    }
}



#[derive(Component)]
pub struct Selected {}

pub struct Action {
    affected_entities: Vec<u64>,
    

    
    //change: Change,
} 

struct QueuedCommand {
    multiplier: f32,
    command: String
}
// pub enum Change {
//     SetTranslation(Vec3),
// }


#[derive(Component, Debug, Copy, Clone)]
#[require(BasePart)]
struct EditorPart {
}



fn execute_queued_commands(
    mut editor_data: ResMut<EditorData>,
    mut command_data: ResMut<CommandData>,
    camera_query: Single<(&Camera, &GlobalTransform)>,
    mut selected: Query<Entity, With<Selected>>,
    mut all_parts: Query<(&mut BasePart,Option<&mut AdjustableHull>)>,
    part_registry: Res<PartRegistry>,
    mut gizmo: Gizmos,
    key: Res<ButtonInput<KeyCode>>,
){
    let mut flip_floating = false;
    for queued_command in &editor_data.queued_commands {
        match queued_command.command.as_str() {
            "W" => move_selected_relative_dir(&mut selected, &mut all_parts, &camera_query, &Dir3::NEG_Z, queued_command.multiplier),
            "A" => move_selected_relative_dir(&mut selected, &mut all_parts, &camera_query, &Dir3::NEG_X, queued_command.multiplier),
            "S" => move_selected_relative_dir(&mut selected, &mut all_parts, &camera_query, &Dir3::Z, queued_command.multiplier),
            "D" => move_selected_relative_dir(&mut selected, &mut all_parts, &camera_query, &Dir3::X, queued_command.multiplier),
            "w" => smart_move_selected_relative_dir(&mut selected, &camera_query, &mut all_parts, &part_registry, &mut gizmo, &Dir3::NEG_Z, queued_command.multiplier),
            "a" => smart_move_selected_relative_dir(&mut selected, &camera_query, &mut all_parts, &part_registry, &mut gizmo, &Dir3::NEG_X, queued_command.multiplier),
            "s" => smart_move_selected_relative_dir(&mut selected, &camera_query, &mut all_parts, &part_registry, &mut gizmo, &Dir3::Z, queued_command.multiplier),
            "d" => smart_move_selected_relative_dir(&mut selected, &camera_query, &mut all_parts, &part_registry, &mut gizmo, &Dir3::X, queued_command.multiplier),
            "F" => {flip_floating=true;}
            _ => {}
        }

        let mut history: String= String::new();
        if queued_command.multiplier!=1.0 {
            history.push_str(&queued_command.multiplier.to_string());
        }
        history.push_str(&queued_command.command);
        command_data.command_history.push_front(history);
    }
    command_data.command_history.truncate(100);
    editor_data.queued_commands.clear();

    if flip_floating {
        editor_data.floating=!editor_data.floating;
    }
}

pub fn translate_floatings(
    mut editor_data: ResMut<EditorData>,
    camera_query: Single<(&Camera, &GlobalTransform)>,
    windows: Single<&Window>,
    mut ray_cast: MeshRayCast,
    mut gizmos: Gizmos,
    mut selected_query: Query<Entity, With<Selected>>,
    mut part_query: Query<(&mut BasePart,Option<&AdjustableHull>,&mut Transform)>,
    base_part_mesh_query: Query<&BasePartMesh>,
    part_registry: Res<PartRegistry>,
    key: Res<ButtonInput<KeyCode>>,
) {
    let (camera, camera_transform) = *camera_query;

    let Some(cursor_position) = windows.cursor_position() else {
        return;
    };

    // Calculate a ray pointing from the camera into the world based on the cursor's position.
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };

    let Some((hit_entity, hit)) = ray_cast.cast_ray(ray, &RayCastSettings {
        filter: &|entity| -> bool {
            if editor_data.floating {
                if let Ok(base_part_mesh) = base_part_mesh_query.get(entity) {
                    if selected_query.contains(base_part_mesh.base_part) {
                        return false;
                    }
                }
            }
            return true;
        },
        ..default()
    }).first() else {
        return;
    };

    if editor_data.floating {
        if selected_query.is_empty() { return; }
        // let mut average_pos = Vec3::ZERO;
        //
        // for transform in &selected_query {
        //     average_pos+=transform.0.translation;
        // }
        // average_pos/=selected_query.iter().len() as f32;

        //let camera_translation = camera_transform.translation() + (camera_transform.forward()*10.0);
        let camera_translation = camera_transform.translation();
        let dir = Dir3::new_unchecked((hit.point-camera_translation).normalize());
        let mut dist=f32::INFINITY;

        let main_selected = part_query.get(selected_query.get_single().unwrap()).unwrap();
        let mut a = get_collider(main_selected.0, main_selected.1, part_registry.parts.get(&main_selected.0.id).unwrap());
        a.translation=camera_translation;
        gizmos.cuboid(a, Color::srgb_u8(0,0,255));

        let hit_base_entity_result = part_query.get(base_part_mesh_query.get(*hit_entity).unwrap().base_part).unwrap();

        //println!("collider main is {:?}",a);

        let b = get_collider(hit_base_entity_result.0, hit_base_entity_result.1, part_registry.parts.get(&hit_base_entity_result.0.id).unwrap());
        gizmos.cuboid(b, Color::srgb_u8(0,0,255));
        //gizmos.cuboid(b.with_scale(b.scale*3.0), Color::srgb_u8(0,0,255));
            //println!("collider secondary is {:?}",b);
        dist=dist.min(to_touch(&a, &b, dir/* , &mut gizmos */));
        if dist==f32::INFINITY { return; }
        println!("DIST IS {:?}",dist);
        let translation = (hit.point-camera_translation).normalize()*dist;
        

        //println!("TRANSOFMRMED EVERYTHING BY {:?}",translation);
        for mut selected in &mut selected_query {
            //transform.0.translation=camera_translation+translation;
            part_query.get_mut(selected).unwrap().0.position=bevy_to_unity_translation(&(camera_translation+translation));
            println!("TRANSOFMRMED EVERYTHING TO {:?}",part_query.get(selected).unwrap().0.position);
            // transform.0.translation.x+=translation.x;
            // transform.0.translation.y+=translation.y;
            // transform.0.translation.z+=translation.z;
        }
    }
}







fn command_typing(
    mut command_data: ResMut<CommandData>,
    mut editor_data: ResMut<EditorData>,
    input_events: Res<Events<KeyboardInput>>,
    input_reader: Local<EventCursor<KeyboardInput>>,
){
    for input in input_reader.clone().read(&input_events) {
        if !input.state.is_pressed() {
            continue;
        };
        match &input.logical_key {
            Key::Character(smol_str) => {
                if smol_str.len() > 1 {
                    return;
                }

                let index = command_data.current_byte_index;
                command_data.current_command.insert(index, smol_str.as_bytes()[0]); 
                command_data.current_byte_index+=1;

                let string = String::from_utf8(command_data.current_command.clone()).unwrap();

                let regex: Regex = Regex::new(r"^(\d+(\.\d*)?)?([a-zA-Z]+)?$").unwrap();
                if regex.is_match(&string) {
                    let captures = regex.captures(&string).unwrap();
                    let num = captures.get(1);
                    let command = captures.get(3);
                    if let Some(command_match) = command {
                        let mut mult: f32 = 1.0;
                        if let Some(num_match) = num{
                            if let Ok(num) = num_match.as_str().parse::<f32>() {
                                mult = num;
                            }
                        }

                        let is_command = command_data.commands.has_command(command_match.as_str().as_bytes());
                        
                        if is_command.0 {
                            if is_command.1 {
                                editor_data.queued_commands.push(
                                    QueuedCommand {
                                        multiplier: mult,
                                        command: command_match.as_str().to_string(),
                                    }
                                );
                                command_data.current_byte_index=0;
                                command_data.current_command.clear();
                            }
                        }else{
                            command_data.current_byte_index=0;
                            command_data.current_command.clear();
                        }
                    }
                }else{
                    command_data.current_byte_index=0;
                    command_data.current_command.clear();
                }
            },
            Key::Space => {
                // let index = command_data.current_byte_index;
                // command_data.current_command.insert(index, " ".as_bytes()[0]); 
                // command_data.current_byte_index+=1;
            },
            //Key::ArrowDown => todo!(),
            Key::ArrowLeft => {
                if command_data.current_byte_index != 0 {
                    command_data.current_byte_index-=1;
                }
            },
            Key::ArrowRight => {
                if command_data.current_byte_index != command_data.current_command.len() {
                    command_data.current_byte_index+=1;
                }
            },
            //Key::ArrowUp => todo!(),
            Key::Escape => {
                command_data.current_byte_index=0;
                command_data.current_command.clear();
            }
            Key::Backspace => {
                let index = command_data.current_byte_index;
                if index > 0 {
                    command_data.current_command.remove(index-1);
                    command_data.current_byte_index-=1;
                }
            }
            Key::Delete => {
                let index = command_data.current_byte_index;
                if index < command_data.current_command.len() {
                    command_data.current_command.remove(index);
                }
            },
            _ => {},
        }
    }
}





fn move_selected_relative_dir(
    mut selected: &mut Query<Entity, With<Selected>>,
    mut all_parts: &mut Query<(&mut BasePart, Option<&mut AdjustableHull>)>,
    camera_transform: &Single<(&Camera, &GlobalTransform)>,
    vector: &Vec3,
    multiplier: f32
){
    
    let mut rot = camera_transform.1.rotation().to_euler(EulerRot::XYZ);

    rot.0 = (rot.0/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.1 = (rot.1/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.2 = (rot.2/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;

    let translation = unity_to_bevy_translation(
        &Quat::from_euler(EulerRot::XYZ, rot.0, rot.1, rot.2).mul_vec3(*vector)
    ) * multiplier;

    for mut selected_part in selected {
        all_parts.get_mut(selected_part).unwrap().0.position+=translation;
    }
}

fn smart_move_selected_relative_dir(
    mut selected: &mut Query<Entity, With<Selected>>,
    //camera_transform: &Query<&Transform, With<Camera3d>>,
    camera_query: &Single<(&Camera, &GlobalTransform)>,
    all_parts: &mut Query<(&mut BasePart,Option<&mut AdjustableHull>)>,
    part_registry: &Res<PartRegistry>,
    gizmo: &mut Gizmos,
    vector: &Vec3,
    multiplier: f32
){

    let mut other_parts = Vec::new();
    for part in all_parts.iter() {
        other_parts.push(get_collider(part.0.deref(), part.1.as_deref(), part_registry.parts.get(&part.0.id).unwrap()))
    }

    for selected_entity in selected {

        let mut selected = all_parts.get_mut(selected_entity).unwrap();

        let selected_bounding_box = get_collider(selected.0.deref(), selected.1.as_deref(), part_registry.parts.get(&selected.0.id).unwrap());

        let dir_nearbys = get_nearby(&selected_bounding_box, &other_parts,false,false /* ,&mut gizmos */);

        let mut possible_positions: HashMap<u8,Vec<f32>> = HashMap::new();
        for i in 0..6 as u8 {
            let selected_shared_face = cuboid_face(&selected_bounding_box,i);


            for nearby in dir_nearbys.get(&i).unwrap() {

                if simple_closest_dist(&selected_bounding_box, nearby.0) > (1.0) {
                    continue;
                }
                //gizmo.cuboid(*nearby.0,Color::srgb_u8(0, 255, 255));

                let face = cuboid_face(nearby.0, nearby.1);
                let mut dotted_dist = (face.1-selected_shared_face.1);

                possible_positions.try_insert(i, Vec::new());
                possible_positions.get_mut(&i).unwrap().push((face.1-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()));
                possible_positions.get_mut(&i).unwrap().push((nearby.0.translation-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()));

                possible_positions.try_insert((i+3)%6, Vec::new());
                possible_positions.get_mut(&((i+3)%6)).unwrap().push(((face.1-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()))*-1.0);
                possible_positions.get_mut(&((i+3)%6)).unwrap().push(((nearby.0.translation-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize()))*-1.0);


                // println!("the diff is {:?}",((face.1-selected_bounding_box.translation).dot(selected_shared_face.0.0.normalize())));
                // println!("for {:?}-{:?} is {:?} and then dot {:?}",face.1,selected_bounding_box.translation,(face.1-selected_bounding_box.translation),selected_shared_face.0.0.normalize());
                dotted_dist = dotted_dist - (dotted_dist.dot(selected_shared_face.0.0.normalize())*selected_shared_face.0.0.normalize());
                
                    
                // cuboid_face_normal(&selected_bounding_box, &i)*
                // ((nearby.0.translation-selected_bounding_box.translation).dot(cuboid_face_normal(&selected_bounding_box, &i)));

                let face = cuboid_face(nearby.0, (nearby.1+(0*3))%6);
                //let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                thing.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, face.0.0.normalize());

                //gizmo.rect(thing, Vec2::ONE*5.0, Color::srgb_u8(255, 255, 0));


            }
        }

        let dir = round_to_axis(&selected_bounding_box, &Dir3::new_unchecked(camera_query.1.rotation().mul_vec3(*vector)));
        let mut distances = possible_positions.get(&dir).unwrap_or(&Vec::new()).clone();
        distances.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut moved = false;
        //println!("distances is {:?}",distances);
        let mut best_dist = f32::MAX;
        let mut the_dist = f32::MAX;
        for pos in distances {
            // if pos >0.0 {
            //     gizmo.arrow(
            //         selected_bounding_box.translation,
            //         selected_bounding_box.translation+(pos*cuboid_face_normal(&selected_bounding_box, &dir)),
            //         Color::srgb_u8(0, 255, 255)
            //     );
            // }
            for possible_pos in [(pos-(cuboid_scale(&selected_bounding_box,&dir)/2.0)),pos,pos+(cuboid_scale(&selected_bounding_box,&dir)/2.0)] {
                //println!("a possiblepos for {:?} is {:?}",pos,possible_pos);
                gizmo.sphere(Isometry3d::from_translation(selected_bounding_box.translation+(possible_pos*cuboid_face_normal(&selected_bounding_box, &dir))), 0.5, Color::srgb_u8(255, 255, 0));
                if possible_pos < best_dist && possible_pos > 0.00001 {
                    best_dist = possible_pos; 
                    the_dist = pos;
                }
            }
        }

        if best_dist!=f32::MAX {
            //println!("pos is {:?} and moved is {:?}",best_dist,(best_dist*cuboid_face_normal(&selected_bounding_box, &dir)));
            moved=true;
            arrow(gizmo,
                selected_bounding_box.translation-cuboid_face(&selected_bounding_box,dir).0.0,
                (best_dist*cuboid_face_normal(&selected_bounding_box, &dir)),
                Color::srgb_u8(0, 255, 255)
            );
            arrow(gizmo,
                selected_bounding_box.translation,
                (best_dist*cuboid_face_normal(&selected_bounding_box, &dir)),
                Color::srgb_u8(0, 255, 255)
            );
            arrow(gizmo,
                selected_bounding_box.translation+cuboid_face(&selected_bounding_box,dir).0.0,
                (best_dist*cuboid_face_normal(&selected_bounding_box, &dir)),
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

