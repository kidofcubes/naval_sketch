use core::f32;
use std::collections::VecDeque;

use bevy::{app::{Plugin, Update}, color::Color, ecs::{event::EventCursor, system::SystemState, world::World}, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{Dir3, Vec3}, prelude::{Camera, Component, Entity, Events, Gizmos, GlobalTransform, KeyCode, Local, MeshRayCast, Query, RayCastSettings, Res, ResMut, Resource, Single, Transform, With}, utils::{default, HashMap}, window::Window};
use bevy_egui::EguiContexts;
use enum_collections::{EnumMap, Enumerated};
use regex::Regex;

use crate::{cam_movement::EditorCamera, editor_actions::EditorActionEvent, editor_ui::{on_part_changed, render_gizmos, update_command_text, update_selected, EditorUiPlugin}, editor_utils::to_touch, parsing::{AdjustableHull, BasePart}, parts::{bevy_to_unity_translation, get_collider, BasePartMesh, PartRegistry}};

#[derive(Resource)]
pub struct DebugGizmo{
    pub to_display: Vec<GizmoDisplay>,
}

pub enum GizmoDisplay {
    Cuboid(Transform,Color),
    Arrow(Vec3,Vec3,Color),
    Sphere(Vec3,f32,Color),
}

impl GizmoDisplay {
    pub fn display(&self, gizmo: &mut Gizmos){
        match self {
            GizmoDisplay::Cuboid(transform, color) => {gizmo.cuboid(*transform,*color);},
            GizmoDisplay::Arrow(pos1,pos2, color) => {gizmo.arrow(*pos1,*pos2,*color);},
            GizmoDisplay::Sphere(pos,radius, color) => {gizmo.sphere(*pos,*radius,*color);},
        }
    }
}

fn debug_gizmos(
    debug_data: Res<DebugGizmo>,
    mut gizmo: Gizmos,
){
    for debug_thing in &debug_data.to_display {
        debug_thing.display(&mut gizmo);
    }
}




pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins(EditorUiPlugin);
        app.insert_resource(
            EditorData {
                action_history: Vec::new(),
                queued_commands: Vec::new(),
                floating: false,
                edit_near: true,
            }
        );
        app.insert_resource(
            DebugGizmo {
                to_display:Vec::new()
            }
        );
        
        let mut command_trees = EnumMap::new_default();
        
        let mut command_tree = CommandTree::default();
        command_tree.add_command(b"w");
        command_tree.add_command(b"a");
        command_tree.add_command(b"s");
        command_tree.add_command(b"d");
        command_tree.add_command(b"q");
        command_tree.add_command(b"e");

        command_tree.add_command(b"W");
        command_tree.add_command(b"A");
        command_tree.add_command(b"S");
        command_tree.add_command(b"D");
        command_tree.add_command(b"Q");
        command_tree.add_command(b"E");

        command_tree.add_command(b"f");
        command_tree.add_command(b"F");

        command_trees[CommandMode::Translation]=command_tree;


        let mut command_tree = CommandTree::default();

        command_tree.add_command(b"q");
        command_tree.add_command(b"e");
        command_tree.add_command(b"w");
        command_tree.add_command(b"a");
        command_tree.add_command(b"s");
        command_tree.add_command(b"d");

        command_tree.add_command(b" ");

        command_trees[CommandMode::Attributes]=command_tree;

        app.insert_resource(
            CommandData {
                command_history: VecDeque::new(),
                current_byte_index: 0,
                current_command: Vec::new(),
                commands: command_trees,
                mode: CommandMode::Translation
            }
        );
        crate::editor_actions::add_actions(app);
        
        app.add_systems(Update, (
                translate_floatings,
                update_selected,
                on_part_changed,
                command_typing,
                update_command_text,
                execute_queued_commands,
                render_gizmos,
                debug_gizmos
        ));


    }
}

#[derive(Enumerated, Copy, Clone, Debug, PartialEq)]
pub enum CommandMode {
    Translation,
    Attributes,
    Rotation,
    Disabled
}


#[derive(Resource)]
pub struct EditorData {
    pub action_history: Vec<Action>,
    pub queued_commands: Vec<QueuedCommand>, //use deque?
    pub floating: bool,
    pub edit_near: bool,
}

#[derive(Resource)]
pub struct CommandData {
    pub command_history: VecDeque<String>,
    pub current_byte_index: usize,
    pub current_command: Vec<u8>,
    pub commands: EnumMap<CommandMode,CommandTree,{CommandMode::SIZE}>,
    pub mode: CommandMode
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



fn execute_queued_commands(
    // mut editor_data: ResMut<EditorData>,
    // mut command_data: ResMut<CommandData>,
    // camera_query: Single<(&Camera, &GlobalTransform)>,
    // mut selected: Query<Entity, With<Selected>>,
    // mut all_parts: Query<(&mut BasePart,Option<&mut AdjustableHull>)>,
    // part_registry: Res<PartRegistry>,
    // mut display_properties: ResMut<PropertiesDisplayData>,
    // mut editor_command_writer: EventWriter<EditorCommandEvent>,
    // mut gizmo: Gizmos,
    world: &mut World,
    //key: Res<ButtonInput<KeyCode>>,
){
    let mut system_state: SystemState<(
        ResMut<EditorData>,
        ResMut<CommandData>,
    )> = SystemState::new(world);

    let (mut editor_data,mut command_data) = system_state.get_mut(world);


    let mut flip_floating = false;
    let mut editor_commands: Vec<EditorActionEvent> = Vec::new();
    for queued_command in &editor_data.queued_commands {
        match command_data.mode {
            CommandMode::Translation => match queued_command.command.as_str() {
                "W" => {editor_commands.push(EditorActionEvent::MoveRelativeDir { vector: Vec3::NEG_Z, mult: queued_command.multiplier });},
                "A" => {editor_commands.push(EditorActionEvent::MoveRelativeDir { vector: Vec3::NEG_X, mult: queued_command.multiplier });},
                "S" => {editor_commands.push(EditorActionEvent::MoveRelativeDir { vector: Vec3::Z, mult: queued_command.multiplier });},
                "D" => {editor_commands.push(EditorActionEvent::MoveRelativeDir { vector: Vec3::X, mult: queued_command.multiplier });},
                "Q" => {editor_commands.push(EditorActionEvent::MoveRelativeDir { vector: Vec3::NEG_Y, mult: queued_command.multiplier });},
                "E" => {editor_commands.push(EditorActionEvent::MoveRelativeDir { vector: Vec3::Y, mult: queued_command.multiplier });},
                //"W" => {editor_command_writer.send(EditorCommandEvent::MoveRelativeDir { vector: *Dir3::NEG_Z, mult: queued_command.multiplier });},



                "w" => {editor_commands.push(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::NEG_Z, mult: queued_command.multiplier });},
                "a" => {editor_commands.push(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::NEG_X, mult: queued_command.multiplier });},
                "s" => {editor_commands.push(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::Z, mult: queued_command.multiplier });},
                "d" => {editor_commands.push(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::X, mult: queued_command.multiplier });},
                "q" => {editor_commands.push(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::NEG_Y, mult: queued_command.multiplier });},
                "e" => {editor_commands.push(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::Y, mult: queued_command.multiplier });},

                "f" => {command_data.mode = CommandMode::Attributes}
                "F" => {flip_floating=true;}
                _ => {}
            },
            CommandMode::Attributes => match queued_command.command.as_str() {
                "w" => {editor_commands.push(EditorActionEvent::SwitchSelectedAttribute{offset:-1,do_loop:false});},
                "s" => {editor_commands.push(EditorActionEvent::SwitchSelectedAttribute{offset:1 ,do_loop:false});},
                "a" => {editor_commands.push(EditorActionEvent::SwitchSelectedAttribute{offset:-5,do_loop:false});},
                "d" => {editor_commands.push(EditorActionEvent::SwitchSelectedAttribute{offset:5 ,do_loop:false});},

                " " => {editor_commands.push(EditorActionEvent::SetSelectedAttribute {value: queued_command.multiplier});},
                _ => {}
            },
            CommandMode::Rotation => todo!(),
            CommandMode::Disabled => todo!(),
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

    for command in editor_commands {
        world.trigger(command);
    }
}

pub fn translate_floatings(
    editor_data: ResMut<EditorData>,
    camera_query: Single<(&Camera, &GlobalTransform, &EditorCamera)>,
    windows: Single<&Window>,
    mut ray_cast: MeshRayCast,
    mut gizmo: Gizmos,
    mut selected_query: Query<Entity, With<Selected>>,
    mut part_query: Query<(&mut BasePart,Option<&AdjustableHull>,&mut Transform)>,
    base_part_mesh_query: Query<&BasePartMesh>,
    part_registry: Res<PartRegistry>,
    key: Res<ButtonInput<KeyCode>>,
) {
    let (camera, camera_transform, _) = *camera_query;

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

        // if key.pressed(KeyCode::Space) {
        //     editor_data.test_data = camera_transform.translation();
        // }

        //let camera_translation = camera_transform.translation() + (camera_transform.forward()*10.0);
        let camera_translation = camera_transform.translation();
        //let camera_translation = editor_data.test_data;
        let dir = Dir3::new_unchecked((hit.point-camera_translation).normalize());
        let mut dist=f32::INFINITY;

        let main_selected = part_query.get(selected_query.get_single().unwrap()).unwrap();
        let mut a = get_collider(main_selected.0, main_selected.1, part_registry.parts.get(&main_selected.0.id).unwrap());
        a.translation=camera_translation+part_registry.parts.get(&main_selected.0.id).unwrap().center;
        gizmo.cuboid(a, Color::srgb_u8(0,0,255));

        let hit_base_entity_result = part_query.get(base_part_mesh_query.get(*hit_entity).unwrap().base_part).unwrap();

        //println!("collider main is {:?}",a);

        let b = get_collider(hit_base_entity_result.0, hit_base_entity_result.1, part_registry.parts.get(&hit_base_entity_result.0.id).unwrap());
        gizmo.cuboid(b, Color::srgb_u8(0,0,255));
        //gizmos.cuboid(b.with_scale(b.scale*3.0), Color::srgb_u8(0,0,255));
            //println!("collider secondary is {:?}",b);
        dist=dist.min(to_touch(&a, &b, dir/* , &mut gizmo */));
        if dist==f32::INFINITY { return; }
        //println!("DIST IS {:?}",dist);
        //println!("THE CENTERS ARE {:?} and {:?}",part_registry.parts.get(&main_selected.0.id).unwrap().center,part_registry.parts.get(&hit_base_entity_result.0.id).unwrap().center);
        let translation = (hit.point-camera_translation).normalize()*dist;
        

        //println!("TRANSOFMRMED EVERYTHING BY {:?}",translation);
        for selected in &mut selected_query {
            //transform.0.translation=camera_translation+translation;
            //part_query.get_mut(selected).unwrap().2.translation=((camera_translation+translation));
            part_query.get_mut(selected).unwrap().0.position=bevy_to_unity_translation(&(camera_translation+translation));
            //println!("TRANSOFMRMED EVERYTHING TO {:?}",part_query.get(selected).unwrap().0.position);
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
    mut contexts: EguiContexts,
){

    let mut focused = false;
    contexts.ctx_mut().memory(|mem|{
        focused = mem.focused().is_some();
    });
    if focused {return;}

    for input in input_reader.clone().read(&input_events) {
        if !input.state.is_pressed() {
            continue;
        };
        let char: Option<u8> = match &input.logical_key {
            Key::Character(smol_str) => {
                if smol_str.len() > 1 {
                    None
                }else{
                    Some(smol_str.as_bytes()[0])
                }

                
            },
            Key::Space => {
                Some(" ".as_bytes()[0])
            },
            //Key::ArrowDown => todo!(),
            Key::ArrowLeft => {
                if command_data.current_byte_index != 0 {
                    command_data.current_byte_index-=1;
                }
                None
            },
            Key::ArrowRight => {
                if command_data.current_byte_index != command_data.current_command.len() {
                    command_data.current_byte_index+=1;
                }
                None
            },
            //Key::ArrowUp => todo!(),
            Key::Escape => {
                if command_data.current_command.is_empty() {
                    command_data.mode = CommandMode::Translation;
                }else{
                    command_data.current_byte_index=0;
                    command_data.current_command.clear();
                }
                None
            }
            Key::Backspace => {
                let index = command_data.current_byte_index;
                if index > 0 {
                    command_data.current_command.remove(index-1);
                    command_data.current_byte_index-=1;
                }
                None
            }
            Key::Delete => {
                let index = command_data.current_byte_index;
                if index < command_data.current_command.len() {
                    command_data.current_command.remove(index);
                }
                None
            },
            _ => None,
        };

        let Some(char) = char else{continue;};

        let index = command_data.current_byte_index;
        command_data.current_command.insert(index, char); 
        command_data.current_byte_index+=1;

        let string = String::from_utf8(command_data.current_command.clone()).unwrap();

        let regex: Regex = Regex::new(r"^(\d*(\.\d*)?)?([a-zA-Z ]+)?$").unwrap();
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

                let is_command = command_data.commands[command_data.mode].has_command(command_match.as_str().as_bytes());
                
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
    }
}

