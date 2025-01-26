use core::f32;
use std::{collections::VecDeque, iter::once, ops::DerefMut};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, Luminance}, ecs::{event::EventCursor, query}, gizmos::primitives::dim3::Plane3dBuilder, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{bounding::{Aabb3d, AabbCast3d, Bounded3d, BoundedExtrusion, BoundingVolume}, Dir3, Direction3d, EulerRot, Isometry3d, Quat, Ray3d, Vec2, Vec3, Vec3A}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, GizmoConfig, GizmoPrimitive3d, Gizmos, GlobalTransform, HierarchyQueryExt, InfinitePlane3d, KeyCode, Local, Mesh3d, MeshRayCast, Out, Over, Parent, Plane3d, Pointer, PointerButton, Primitive3d, Query, RayCastSettings, Ref, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, text::TextFont, ui::{BackgroundColor, Node, PositionType, Val}, utils::{default, HashMap}, window::Window};
use regex::Regex;
use smol_str::SmolStr;

use crate::{editor_ui::{on_click, on_hover, on_part_changed, on_unhover, render_gizmos, spawn_ui, update_command_text, update_selected, CommandDisplayData, Hovered}, parsing::{AdjustableHull, BasePart}, parts::{get_collider, unity_to_bevy_translation, BasePartMesh, PartRegistry}};

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
    mut selected: Query<&mut BasePart, With<Selected>>,
    camera_transform: Query<&Transform, With<Camera3d>>,
    key: Res<ButtonInput<KeyCode>>,
){
    let mut flip_floating = false;
    for queued_command in &editor_data.queued_commands {
        match queued_command.command.as_str() {
            "w" => move_selected_relative_dir(&mut selected, &camera_transform, &Dir3::NEG_Z, queued_command.multiplier),
            "a" => move_selected_relative_dir(&mut selected, &camera_transform, &Dir3::NEG_X, queued_command.multiplier),
            "s" => move_selected_relative_dir(&mut selected, &camera_transform, &Dir3::Z, queued_command.multiplier),
            "d" => move_selected_relative_dir(&mut selected, &camera_transform, &Dir3::X, queued_command.multiplier),
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
    mut selected_query: Query<(&mut Transform,&BasePart,Option<&AdjustableHull>), With<Selected>>,
    part_query: Query<(&BasePart,Option<&AdjustableHull>)>,
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

        let main_selected = selected_query.get_single().unwrap();
        let mut a = get_collider(main_selected.1, main_selected.2, part_registry.parts.get(&main_selected.1.id).unwrap());
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
        for mut transform in &mut selected_query {
            transform.0.translation=camera_translation+translation;
            println!("TRANSOFMRMED EVERYTHING TO {:?}",transform.0.translation);
            // transform.0.translation.x+=translation.x;
            // transform.0.translation.y+=translation.y;
            // transform.0.translation.z+=translation.z;
        }
    }
}







#[derive(Debug, Copy, Clone)]
pub enum Thing {
    Vertex(Vec3),
    Line(Vec3,Vec3),
    Plane(Vec3,Vec3,Vec3,Vec3)
}

///how far a has to move in direction dir to touch b
pub fn to_touch_thing(a: &Thing, b: &Thing, dir: &Dir3/* , draw_gizmo: bool, gizmo: &mut Gizmos */) -> Option<f32>{
    match a {
        Thing::Vertex(a_pos) => {
            match b {
                Thing::Vertex(vec3) => None,
                Thing::Line(vec3, vec4) => None,
                Thing::Plane(plane_center, normal, normal2, normal3) => {
                    // if true { return None; }
                    let ray = Ray3d{ origin: *a_pos, direction: *dir};
                    let hit = ray.intersect_plane(*plane_center, InfinitePlane3d { normal: Dir3::new_unchecked(normal.normalize())});
                    // println!("vertex hit was {:?}",hit);

                    if hit == None {return None;}
                    let hit = hit.unwrap();
                    let hit_pos = a_pos + ((*dir)*hit);
                    if 
                        ((hit_pos-plane_center).dot(normal2.normalize())).abs() <= normal2.length() &&
                        ((hit_pos-plane_center).dot(normal3.normalize())).abs() <= normal3.length()
                    {
                        //gizmo.arrow(*a_pos, hit_pos, Color::srgb_u8(255, 0, 0));
                        println!("we hit on {:?} on plane {:?} from {:?} with {:?}",hit_pos,b,a_pos,hit);
                        return Some(hit);
                    }else{
                        return None;
                    }
                },
            }
        },
        Thing::Line(a_start, a_end) => {
            match b {
                Thing::Vertex(vec3) => None,
                Thing::Line(b_start, b_end) => {
                    // if true { return None; }
                    // if(draw_gizmo){gizmo.line(*a_start, *a_end, Color::srgb_u8(255, 255, 0));}
                    // if(draw_gizmo){gizmo.line(*b_start, *b_end, Color::srgb_u8(0, 255, 255));}
                    //gizmo.arrow((a_start+a_end)/2.0, ((a_start+a_end)/2.0)+(dir.as_vec3()), Color::srgb_u8(0, 255, 255));
                    let b_dir = (b_end-b_start);
                    let b_ray = Ray3d{ origin: *b_start, direction: Dir3::new_unchecked(b_dir.normalize())};
                    let b_middle = ((b_end+b_start)/2.0);
                    //println!("b_start is {:?} b_middle is {:?}",b_start,b_middle);
                    // if(draw_gizmo){gizmo.arrow(*b_start, b_middle, Color::srgb_u8(255, 0, 255));}
                    let a_dir = (a_end-a_start);
                    // println!("b_ray is {:?}",b_ray);
                    let a_plane_normal = Dir3::new_unchecked(dir.cross(a_dir).normalize());
                    // println!("dir is {:?} and the a_line dir is {:?}",dir,(a_end-a_start));
                    // println!("a_start is {:?} and a_plane_normal is {:?}",a_start,a_plane_normal);
                    // if(draw_gizmo){gizmo.primitive_3d(&Plane3d::new(*a_plane_normal, Vec2::new(5.0,5.0)), (*a_start), Color::srgb_u8(0, 255, 0));}
                    // if(draw_gizmo){gizmo.arrow(*a_start, (*a_start)+(dir.as_vec3()), Color::srgb_u8(255, 0, 0));}
                    // if(draw_gizmo){gizmo.arrow(*a_start, (*a_start)+(a_dir.normalize()), Color::srgb_u8(0, 0, 255));}

                    let hit = b_ray.intersect_plane(*a_start, InfinitePlane3d { normal: a_plane_normal });
                    // println!("the dot of ray and normal is {:?}",b_ray.direction.dot(*a_plane_normal));
                    // println!("line hit was {:?}",hit);

                    if hit == None {return None;}
                    let hit = hit.unwrap();
                    if hit > b_dir.length() { return None; }

                    let hit_pos = b_start + (b_ray.direction.normalize()*hit);
                    // if(draw_gizmo){gizmo.sphere(hit_pos, 0.5, Color::srgb_u8(255, 255, 255));}
                    // if(draw_gizmo){gizmo.arrow(*a_start, hit_pos, Color::srgb_u8(255, 255, 255));}
                    // if(draw_gizmo){gizmo.arrow(*b_start, hit_pos, Color::srgb_u8(255, 255, 255));}

                    let hit_offset = (hit_pos-a_start);

                    let a_dir_dirs = a_dir.dot(dir.as_vec3());
                    let perp_a_dir = a_dir - (a_dir_dirs*dir.as_vec3());
                    //println!("a_dir_dirs is {:?} and perp_a_dir is {:?}",a_dir_dirs,perp_a_dir);
                    //println!("the cross is {:?}",perp_a_dir.normalize().cross(dir.as_vec3()).length());
                    //gizmo.arrow(*a_start, a_start+perp_a_dir, Color::srgb_u8(255, 0, 255));

                    let hit_offset_perp_a_dirs = hit_offset.dot(perp_a_dir.normalize()); //raw length

                    //let offset1 = (perp_a_dir.normalize()*hit_offset_perp_a_dirs);
                    // let offset1 = ((hit_offset_perp_a_dirs/perp_a_dir.length())*a_dir);
                    // if(draw_gizmo){gizmo.arrow(*a_start, a_start+offset1, Color::srgb_u8(255, 0, 0));}
                    // if(draw_gizmo){println!("the thing is {:?}",(hit_offset-offset1).cross(**dir));}
                    // let offset2 = dir.as_vec3()*(hit_offset-offset1).length();
                    // if(draw_gizmo){gizmo.arrow(*a_start+offset1, a_start+offset1+offset2, Color::srgb_u8(255, 0, 255));}
                    // if(draw_gizmo){println!("hit offset was {:?} which had {:?} perp_a_dir in it ",hit_offset,hit_offset_perp_a_dirs);}

                    //gizmo.sphere(a_start+(perp_a_dir*hit_offset_perp_a_dirs), 0.2, Color::srgb_u8(255, 0, 255));

                    if 
                        hit_offset_perp_a_dirs > (perp_a_dir).length()
                            ||
                        hit_offset_perp_a_dirs < 0.0
                    {
                        return None;
                    }
                    //println!("removing {:?} from {:?}hitoffset makes it",(hit_offset_perp_a_dirs*a_dir_dirs)*dir.as_vec3(),hit_offset);
                    //let all_dir = (hit_offset-(((hit_offset_perp_a_dirs/perp_a_dir.length())*a_dir_dirs)*dir.as_vec3()));
                    let all_dir = (hit_offset-((hit_offset_perp_a_dirs/perp_a_dir.length())*a_dir));
                    //println!("all dir is {:?}",all_dir);
                    let moved_dist = all_dir.length();
                    
                    if moved_dist < 0.0 { return None; }
                    return Some(moved_dist);
                    
                },
                Thing::Plane(vec3, vec4, vec5, vec6) => None,
            }
        },
        Thing::Plane(vec3, vec4, vec5, vec6) => {
            None
        },
    }

}
pub fn all_things(a :&Transform) -> Vec<Thing> {
    let mut things: Vec<Thing> = Vec::new();


    for i in 0..6 { let temp = cuboid_face(a, i); things.push(Thing::Plane(temp.1, temp.0.0, temp.0.1, temp.0.2)); }
    for i in 0..8 { things.push(Thing::Vertex(cuboid_vertex(a, i))); }
    for i in 0..12 { let temp = cuboid_edge(a, i); things.push(Thing::Line(temp.0, temp.1)); }
    return things;
}

pub fn to_touch(a: &Transform, b: &Transform, mut dir: Dir3/* , gizmo: &mut Gizmos */) -> f32{
    
    
    //let new_a = Transform::from_matrix(a.compute_matrix()*a.compute_matrix().inverse());
    // let new_a = Transform::IDENTITY;
    // let new_b = Transform::from_matrix(b.compute_matrix()*(a.compute_matrix().inverse()));
    let mut new_a = a;
    let mut new_b = b;
    // println!("the a is {:?} new its {:?}",a,new_a);
    // println!("the b is {:?} new its {:?}",b,new_b);
    // println!("");
    // println!("");
    // println!("");
    // println!("");

    // new_b.vertex[0https://gizmodo.com/picture-of-a-duck-accidentally-sent-to-stripe-workers-being-laid-off-2000552964]
    //
    //
    let mut min_dist=f32::INFINITY;
    let a_things: Vec<Thing> = all_things(new_a);
    let b_things: Vec<Thing> = all_things(new_b);
    let mut best_thing_a = a_things[0];
    let mut best_thing_b = b_things[0];
    for a_thing in a_things {
        for b_thing in &b_things {
            if let Some(dist) = to_touch_thing(&a_thing, &b_thing, &dir/* , false, gizmo */){
                if dist <= min_dist {
                    best_thing_a = a_thing;
                    best_thing_b = *b_thing;
                    min_dist = min_dist.min(dist);
                }
            }

            if let Some(dist) = to_touch_thing(&b_thing, &a_thing, &Dir3::new_unchecked(dir*-1.0)/* , false, gizmo */){
                if core::mem::discriminant(&a_thing) != core::mem::discriminant(&b_thing) {
                    min_dist = min_dist.min(dist);
                }
            }

        }
    }

    // let Thing::Line(a_start,a_end) = best_thing_a else { return min_dist; };
    // let Thing::Line(b_start,b_end) = best_thing_b else { return min_dist; };
    // gizmo.arrow(a_start, b_start, Color::srgb_u8(255, 0, 0));
    // gizmo.arrow(a_end, b_end, Color::srgb_u8(255, 0, 0));
    // to_touch_thing(&best_thing_a, &best_thing_b, &dir, true, gizmo);

    



    
    
    // let bounding_a = Aabb3d {
    //     min: Vec3A::from(axis_a.translation-(axis_a.scale/2.0)),
    //     max: Vec3A::from(axis_a.translation+(axis_a.scale/2.0)),
    // };

    // let new_b = b.with_rotation(b.rotation*a.rotation.inverse());
    //
    // let ray: Ray3d = Ray3d { origin: a.translation-(axis_a.scale/2.0), direction: dir };
    

    //ray.intersect_plane(b.translation+(b.forward()*(b.scale.z/2.0)), InfinitePlane3d {normal: b.forward()});

    return min_dist;
}

pub fn cuboid_vertex(a: &Transform, i: u8) -> Vec3{
    return a.translation+(((a.forward()*neg(i&4)*a.scale.z)+(a.up()*neg(i&2)*a.scale.y)+(a.left()*neg(i&1)*a.scale.x)));
}
pub fn cuboid_edge(a: &Transform, i: u8) -> (Vec3, Vec3){
    match(i){
        0  => (cuboid_vertex(a, 0),cuboid_vertex(a, 1)), //left right
        1  => (cuboid_vertex(a, 2),cuboid_vertex(a, 3)),
        2  => (cuboid_vertex(a, 4),cuboid_vertex(a, 5)),
        3  => (cuboid_vertex(a, 6),cuboid_vertex(a, 7)),

        4  => (cuboid_vertex(a, 0),cuboid_vertex(a, 2)), //up down
        5  => (cuboid_vertex(a, 1),cuboid_vertex(a, 3)),
        6  => (cuboid_vertex(a, 4),cuboid_vertex(a, 6)),
        7  => (cuboid_vertex(a, 5),cuboid_vertex(a, 7)),

        8  => (cuboid_vertex(a, 0),cuboid_vertex(a, 4)), //forward backward
        9  => (cuboid_vertex(a, 1),cuboid_vertex(a, 5)),
        10 => (cuboid_vertex(a, 2),cuboid_vertex(a, 6)),
        11 => (cuboid_vertex(a, 3),cuboid_vertex(a, 7)),
        _ => {panic!("wtf")}
    }
}
pub fn cuboid_face(a: &Transform, i: u8) -> ((Vec3,Vec3,Vec3), Vec3){
    let s = a.scale/2.0;
    let dir = match(i){
        0 => {(*a.forward()*s.z,*a.left()*s.x,*a.up()*s.y)}
        1 => {(*a.back()*s.z,*a.left()*s.x,*a.up()*s.y)}
        2 => {(*a.right()*s.x,*a.forward()*s.z,*a.up()*s.y)}
        3 => {(*a.left()*s.x,*a.forward()*s.z,*a.up()*s.y)}
        4 => {(*a.up()*s.y,*a.forward()*s.z,*a.left()*s.x)}
        5 => {(*a.down()*s.y,*a.forward()*s.z,*a.left()*s.x)}
        _ => {panic!("wtf")}
    };
    //println!("cuboid face of {:?} is {:?}",a,a.translation+(dir*s));
    return (dir, a.translation+(dir.0));
}
fn neg(num: u8) -> f32{
    if num==0 {-0.5}else{0.5}
}


fn pos_in_cuboid(a: &Vec3A, b: &Transform) -> bool {
    let new_a = b.rotation.inverse().mul_vec3a(*a);
    
    return 
        (b.translation.x-b.scale.x <= new_a.x)&&(new_a.x <= b.translation.x+b.scale.x) &&
        (b.translation.y-b.scale.y <= new_a.y)&&(new_a.y <= b.translation.y+b.scale.y) &&
        (b.translation.z-b.scale.z <= new_a.z)&&(new_a.z <= b.translation.z+b.scale.z)
        ;
}

pub fn dir_from_index(i: &u8) -> Vec3 {
    match(i){
        0 => *Dir3::NEG_Z,
        1 => *Dir3::Z,
        2 => *Dir3::X,
        3 => *Dir3::NEG_X,
        4 => *Dir3::Y,
        5 => *Dir3::NEG_Y,
        _ => {panic!("wtf")}
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

pub fn dist_to_int(num: f32) -> f32{
    return f32::min((num.round()-num).abs(),(num-num.round()).abs());
}

fn in_rect(a_center: Vec3, a_axis1: Vec3, a_axis2: Vec3, check: Vec3, gizmo: &mut Gizmos) -> bool{
    let diff = (check-a_center);
    gizmo.arrow(a_center,check,Color::srgb_u8(0, 255, 0));
    gizmo.arrow(a_center,a_center+(diff.dot(a_axis1.normalize())*a_axis1.normalize()),Color::srgb_u8(255, 0, 0));
    gizmo.arrow(a_center,a_center+(diff.dot(a_axis2.normalize())*a_axis2.normalize()),Color::srgb_u8(255, 0, 0));
    return (diff.dot(a_axis1.normalize()).abs() <= a_axis1.length()+0.01)
        && (diff.dot(a_axis2.normalize()).abs() <= a_axis2.length()+0.01)

}

fn plane_vertexes(center: Vec3, axis1: Vec3, axis2: Vec3) -> [Vec3;4] {
    return [
        center+axis1+axis2,
        center+axis1-axis2,
        center+axis2-axis1,
        center-(axis1+axis2)
    ];
}

fn all_on_one_side(points: &[Vec3], center: Vec3, dir: Vec3, gizmo: &mut Gizmos) -> bool {
    let normalized_dir = dir.normalize();
    let center_pos = (center).dot(normalized_dir);

    let boundary = dir.length();

    let num = (points[0].dot(normalized_dir)-center_pos);
    //gizmo.arrow(center, center+(num*normalized_dir), Color::srgb_u8(255, 0, 0));
    if num.abs() <= boundary { return false; }
    let orig_side = num.signum();


    for point in &points[1..] {
        let num = (point.dot(normalized_dir)-center_pos);
        //gizmo.arrow(center, center+(num*normalized_dir), Color::srgb_u8(255, 0, 0));
        if num.abs() < boundary { return false; }
        if num.signum()!=orig_side { return false; }
        
    }

    return true;
}

fn rects_intersect(a_center: Vec3, a_axis1: Vec3, a_axis2: Vec3, b_center: Vec3, b_axis1: Vec3, b_axis2: Vec3, gizmo: &mut Gizmos) -> bool{
    
    let a_points = plane_vertexes(a_center, a_axis1, a_axis2);
    let b_points = plane_vertexes(b_center, b_axis1, b_axis2);

    if all_on_one_side(&a_points, b_center, b_axis1, gizmo) {  return false; }
    if all_on_one_side(&a_points, b_center, b_axis2, gizmo) {  return false; }
    if all_on_one_side(&b_points, a_center, a_axis1, gizmo) {  return false; }
    if all_on_one_side(&b_points, a_center, a_axis2, gizmo) {  return false; }

    return true;
}



pub fn get_nearby<'a>(
    origin: &Transform,
    to_check: &'a Vec<Transform>,
    gizmo: &mut Gizmos
) -> HashMap<u8,Vec<&'a Transform>> {
    let mut nearby: HashMap<u8,Vec<&'a Transform>> = HashMap::new();
    let mut faces :Vec<((Vec3,Vec3,Vec3),Vec3)> = Vec::with_capacity(6);
    for i in 0..6 {
        faces.push(cuboid_face(origin, i as u8));
    }

    for check in to_check {
        if check==origin {
            continue;
        }
        // let rotation_diff =
        //     (Vec3::from(check.rotation.to_euler(EulerRot::XYZ))-
        //     Vec3::from(origin.rotation.to_euler(EulerRot::XYZ)))
        //     / f32::consts::FRAC_PI_2;
        // println!("rotation diff is {:?}",rotation_diff);

        // if 
        //     dist_to_int(rotation_diff.x) > f32::EPSILON*2.0 &&
        //     dist_to_int(rotation_diff.y) > f32::EPSILON*2.0 &&
        //     dist_to_int(rotation_diff.z) > f32::EPSILON*2.0
        // {
        //     continue;
        // }

        let mut touching: Vec<u8> = Vec::new();
        'origin_faces: for i in 0..6 {
            'check_faces: for j in 0..6 {
                let face = cuboid_face(check, j);
                let normal = face.0.0;
                if (faces[i].0.0.normalize()+normal.normalize()).length() > 0.001 { continue; } 

                let dist = ((check.translation-origin.translation).dot(faces[i].0.0.normalize()));
                if (dist-(normal.length()+faces[i].0.0.length())).abs() <= 3.0 {
                    //println!("the dist diff thing was {:?} - {:?}",dist,(normal.length()+faces[i].0.0.length()),);
                    //gizmo.cuboid(*check, Color::srgba_u8(255, 0, 0, 100));
                    //gizmo.arrow(origin.translation, origin.translation+faces[i].0.0, Color::srgb_u8(0, 0, 255));
                    //gizmo.arrow(check.translation, check.translation+normal, Color::srgb_u8(0, 0, 255));
                }
                //if (dist-(normal.length()+faces[i].0.0.length())).abs() <= (1.0*f32::EPSILON) {

                //if (dist-(normal.length()+faces[i].0.0.length())).abs() > (0.01) { //na suckS
                if (dist-(normal.length()+faces[i].0.0.length())) > (0.01) || dist < 0.0 { //na suckS
                    break 'check_faces;
                }
                

                //if rects_intersect(faces[i].1, faces[i].0.1, faces[i].0.2, face.1, face.0.1, face.0.2, gizmo) {
                if rects_intersect(origin.translation, faces[i].0.1, faces[i].0.2, check.translation, face.0.1, face.0.2, gizmo) {
                    touching.push(i as u8);
                }
                break 'check_faces;
            }
        }
        for touched in touching {
            if !nearby.contains_key(&touched) {
                nearby.insert(touched,Vec::new());
            }
            nearby.get_mut(&touched).unwrap().push(check);
        }

        //nearby.push(check);
    }
    return nearby;
}




fn move_selected_relative_dir(
    mut selected: &mut Query<&mut BasePart, With<Selected>>,
    camera_transform: &Query<&Transform, With<Camera3d>>,
    vector: &Vec3,
    multiplier: f32
){
    
    let mut rot = camera_transform.get_single().unwrap().rotation.to_euler(EulerRot::XYZ);

    rot.0 = (rot.0/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.1 = (rot.1/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.2 = (rot.2/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;

    let translation = unity_to_bevy_translation(
        &Quat::from_euler(EulerRot::XYZ, rot.0, rot.1, rot.2).mul_vec3(*vector)
    ) * multiplier;

    for mut base_part in selected {
        base_part.position = base_part.position + translation;
    }
}

fn smart_move_selected_relative_dir(
    mut selected: &mut Query<&mut BasePart, With<Selected>>,
    camera_transform: &Query<&Transform, With<Camera3d>>,
    vector: &Vec3,
    multiplier: f32
){
    let mut rot = camera_transform.get_single().unwrap().rotation.to_euler(EulerRot::XYZ);

    rot.0 = (rot.0/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.1 = (rot.1/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.2 = (rot.2/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;

    let translation = unity_to_bevy_translation(
        &Quat::from_euler(EulerRot::XYZ, rot.0, rot.1, rot.2).mul_vec3(*vector),
    );

    for mut base_part in selected {
        base_part.position = base_part.position + translation;
    }
}

