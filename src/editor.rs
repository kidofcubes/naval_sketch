use core::f32;
use std::{collections::VecDeque, iter::once, ops::DerefMut};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, Luminance}, ecs::{event::EventCursor, query}, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{Dir3, EulerRot, Isometry3d, Quat, Vec3}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, Gizmos, GlobalTransform, HierarchyQueryExt, KeyCode, Local, Mesh3d, MeshRayCast, Out, Over, Parent, Pointer, PointerButton, Query, RayCastSettings, Ref, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, text::TextFont, ui::{BackgroundColor, Node, PositionType, Val}, utils::{default, HashMap}, window::Window};
use bevy_mod_outline::OutlineVolume;
use regex::Regex;
use smol_str::SmolStr;

use crate::{editor_ui::{on_click, on_hover, on_part_changed, on_unhover, render_gizmos, spawn_ui, update_command_text, update_selected, CommandDisplayData}, parsing::BasePart, parts::{base_part_to_bevy_transform, unity_to_bevy_translation, BasePartMesh}};

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.insert_resource(
            EditorData {
                action_history: Vec::new(),
                queued_commands: Vec::new(),
                floating: false
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
    editor_data: Res<EditorData>,
    camera_query: Single<(&Camera, &GlobalTransform)>,
    windows: Single<&Window>,
    mut ray_cast: MeshRayCast,
    mut gizmos: Gizmos,
    mut selected_query: Query<&mut Transform, With<Selected>>,
    base_part_mesh_query: Query<&BasePartMesh>,
) {
    let (camera, camera_transform) = *camera_query;

    let Some(cursor_position) = windows.cursor_position() else {
        return;
    };

    // Calculate a ray pointing from the camera into the world based on the cursor's position.
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };

    let Some((_, hit)) = ray_cast.cast_ray(ray, &RayCastSettings {
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
        let mut average_pos = Vec3::ZERO;

        for transform in &selected_query {
            average_pos+=transform.translation;
        }
        average_pos/=selected_query.iter().len() as f32;

        let translation = hit.point-average_pos;
        println!("TRANSOFMRMED EVERYTHING BY {:?}",translation);
        for mut transform in &mut selected_query {
            transform.translation.x+=translation.x;
            transform.translation.y+=translation.y;
            transform.translation.z+=translation.z;
        }
    }





    // Draw a circle just above the ground plane at that position.
    // gizmos.circle(
    //     Isometry3d::new(
    //         hit.point,
    //         Quat::from_rotation_arc(Vec3 {x:1.0,y:0.0,z:0.0},hit.normal.normalize()),
    //     ),
    //     0.2,
    //     Color::srgb_u8(255, 0, 0),
    // );
    // gizmos.circle(
    //     Isometry3d::new(
    //         hit.point,
    //         Quat::from_rotation_arc(Vec3 {x:0.0,y:1.0,z:0.0},hit.normal.normalize()),
    //     ),
    //     0.2,
    //     Color::srgb_u8(0, 255, 0),
    // );
    // gizmos.circle(
    //     Isometry3d::new(
    //         hit.point,
    //         Quat::from_rotation_arc(Vec3 {x:0.0,y:0.0,z:1.0},hit.normal.normalize()),
    //     ),
    //     0.2,
    //     Color::srgb_u8(0, 0, 255),
    // );
    // gizmos.arrow(
    //     hit.point,
    //     hit.point+hit.normal.normalize(),
    //     Color::WHITE,
    // );
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

