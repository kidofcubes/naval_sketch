use core::f32;
use std::{iter::once, ops::DerefMut};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, Luminance}, ecs::{event::EventCursor, query}, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{Dir3, EulerRot, Quat, Vec3}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, HierarchyQueryExt, KeyCode, Local, Mesh3d, Out, Over, Parent, Pointer, PointerButton, Query, Ref, RemovedComponents, Res, ResMut, Resource, Text, Transform, Trigger, With}, reflect::List, text::TextFont, ui::{BackgroundColor, Node, PositionType, Val}, utils::default};
use bevy_mod_outline::OutlineVolume;
use regex::Regex;
use smol_str::SmolStr;

use crate::{parsing::BasePart, parts::{base_part_to_bevy_transform, unity_to_bevy_translation}};

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.insert_resource(
            EditorData {
                action_history: Vec::new(),
            }
        );
        app.insert_resource(
            CommandData {
                command_history: Vec::new(),
                current_byte_index: 0,
                current_command: Vec::new(),
            }
        );
        app.insert_resource(
            CommandDisplayData {
                mult: -1.0,
                font_size: -1.0,
                font_width: -1.0,
                text_display: None,
                flasher: None,
            }
        );
        app.add_observer(on_hover);
        app.add_observer(on_unhover);
        app.add_observer(on_click);
        app.add_systems(Startup, (spawn_ui));
        app.add_systems(Update, (update_selected, on_part_changed, command_typing, update_command_text));
    }
}

#[derive(Resource)]
pub struct EditorData {
    action_history: Vec<Action>,
}

#[derive(Resource)]
pub struct CommandData {
    command_history: Vec<String>,
    current_byte_index: usize,
    current_command: Vec<u8>,
}

#[derive(Resource)]
pub struct CommandDisplayData {
    mult: f32,
    font_size: f32,
    font_width: f32,
    text_display: Option<Entity>,
    flasher: Option<Entity>,
}

#[derive(Component)]
pub struct Selected {}

pub struct Action {
    affected_entities: Vec<u64>,
    change: Change,
}


pub enum Change {
    SetTranslation(Vec3),
}

fn on_click(
    click: Trigger<Pointer<Down>>,
    part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    children_query: Query<&Children>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected: Query<Entity, With<Selected>>,
    mut commands: Commands,
){
    if click.event().button != PointerButton::Primary {
        return;
    }
    if let Some(clicked) = get_base_part_entity(&parent_query, &part_query, click.entity()) {
        for thing in &selected {
            commands.entity(thing).remove::<Selected>();
        }
        
        commands.entity(clicked).insert(Selected{});
    };
}

fn on_hover(
    hover: Trigger<Pointer<Over>>,
    part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    children_query: Query<&Children>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
){

    // i'm assuming iter_ancestors loops it in order of nearest parent hopfully
    for base_entity in once(hover.entity()).chain(parent_query.iter_ancestors(hover.entity())) {
        if let Ok(base_part) = part_query.get(base_entity) {
            for entity in once(base_entity).chain(children_query.iter_descendants(base_entity)) {
                if let Ok(mut material) = material_query.get_mut(entity) {
                    material.0 = materials.add(StandardMaterial::from_color(base_part.color.with_luminance(base_part.color.luminance()*2.0)));
                }
            }
            break;
        }
    };
}

fn on_unhover(
    unhover: Trigger<Pointer<Out>>,
    part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    children_query: Query<&Children>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,

){
    // i'm assuming iter_ancestors loops it in order of nearest parent hopfully
    for base_entity in once(unhover.entity()).chain(parent_query.iter_ancestors(unhover.entity())) {
        if let Ok(base_part) = part_query.get(base_entity) {
            for entity in once(base_entity).chain(children_query.iter_descendants(base_entity)) {
                if let Ok(mut material) = material_query.get_mut(entity) {
                    material.0 = materials.add(StandardMaterial::from_color(base_part.color));
                }
            }
            break;
        }
    };
}

fn get_base_part_entity(parent_query: &Query<&Parent>, part_query: &Query<&BasePart>, entity: Entity) -> Option<Entity>{
    // i'm assuming iter_ancestors loops it in order of nearest parent hopfully
    for base_entity in once(entity).chain(parent_query.iter_ancestors(entity)) {
        if part_query.get(base_entity).is_ok() {
            return Some(base_entity);
        }
    };
    return None;
}

fn update_selected(
    children_query: Query<&Children>,
    material_query: Query<Entity, With<Mesh3d>>,
    added: Query<Entity, (With<BasePart>, Added<Selected>)>,
    mut removed: RemovedComponents<Selected>,
    mut commands: Commands,
){
    removed.read().for_each(|base_entity| {
        for entity in once(base_entity).chain(children_query.iter_descendants(base_entity)) {
            if material_query.contains(entity) {
                commands.entity(entity).remove::<OutlineVolume>();
            }
        }
    });

    for base_entity in &added {
        for entity in once(base_entity).chain(children_query.iter_descendants(base_entity)) {
            if material_query.contains(entity) {
                commands.entity(entity).insert(
                    OutlineVolume {
                        visible: true,
                        width: 5.0,
                        colour: Color::srgba_u8(0, 255, 0, 255),
                        ..default()
                    }
                );
            }
        }
    }
}

fn on_update(
    key: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut BasePart, With<Selected>>
){
}

fn on_part_changed(
    mut changed_base_part: Query<(&mut Transform, Ref<BasePart>), Changed<BasePart>>,
){
    for mut pair in &mut changed_base_part {
        println!("THE THING CHANGED OH MAI GAH {:?}",pair);
        let new_transform =
            base_part_to_bevy_transform(&pair.1);
        pair.0.translation = new_transform.translation;
        pair.0.rotation = new_transform.rotation;
        pair.0.scale = new_transform.scale;
    }
}

/// Spawn a bit of UI text to explain how to move the player.
pub fn spawn_ui(
    asset_server: Res<AssetServer>,
    mut font_data: ResMut<CommandDisplayData>,
    mut commands: Commands
) {
    font_data.mult = 4.0;
    font_data.font_size = 13.0;
    font_data.font_width = 6.0;
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        })
        .with_children(|parent| {
            font_data.text_display = Some(
                parent.spawn_empty().insert((
                    Text::new("vimming times"),
                    TextFont {
                        font: asset_server.load("/usr/share/fonts/TTF/CozetteVector.ttf"),
                        font_size: font_data.font_size*font_data.mult, 
                        ..default()
                    },
                )).id()
            );


            font_data.flasher = Some(
                parent.spawn_empty().insert((
                    Node {
                        // Take the size of the parent node.
                        width: Val::Px(1.0*font_data.mult),
                        height: Val::Px(font_data.font_size*font_data.mult),
                        position_type: PositionType::Absolute,
                        left: Val::Px(font_data.font_width*font_data.mult*5.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba_u8(255,255,255,255))
                )).id()
            );

        })
    ;
}


fn command_typing(
    mut command_data: ResMut<CommandData>,
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

                let regex: Regex = Regex::new(r"^(\d+(\.\d+)?)?([a-zA-Z]+)$").unwrap();
                if regex.is_match(&string) {
                    let captures = regex.captures(&string).unwrap();
                    let num = captures.get(1);
                    let command = captures.get(3);
                    println!("WE GOT A COMMAND {:?}*{:?}",num,command);
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

fn update_command_text(
    command_data: Res<CommandData>,
    command_display_data: Res<CommandDisplayData>,
    mut text_query: Query<&mut Text>,
    mut node_query: Query<&mut Node>,
){
    if command_data.is_changed()  {
        text_query.get_mut(command_display_data.text_display.unwrap()).unwrap().0 =
            String::from_utf8(command_data.current_command.clone()).unwrap();
        // println!("text_display is {:?}",command_display_data.text_display);
        // println!("text_query is {:?}",text_query.get_mut(command_display_data.text_display.unwrap()));
        // println!("flasher is {:?}",command_display_data.flasher);
        // println!("node_query is {:?}",node_query.get_mut(command_display_data.flasher.unwrap()));

        node_query.get_mut(command_display_data.flasher.unwrap()).unwrap().left =
            Val::Px(command_display_data.font_width * command_display_data.mult * (command_data.current_byte_index as f32))
    }
}


fn move_selected_forwards(
    mut selected: Query<&mut BasePart, With<Selected>>,
    camera_transform: Query<&Transform, With<Camera3d>>,
    multiplier: f32
){
    let mut rot = camera_transform.get_single().unwrap().rotation.to_euler(EulerRot::XYZ);

    rot.0 = (rot.0/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.1 = (rot.1/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;
    rot.2 = (rot.2/f32::consts::FRAC_PI_2).round()*f32::consts::FRAC_PI_2;

    let translation = unity_to_bevy_translation(
        &Quat::from_euler(EulerRot::XYZ, rot.0, rot.1, rot.2).mul_vec3(Dir3::Z.as_vec3())
    ) * multiplier;

    for mut base_part in &mut selected {
        base_part.position = base_part.position + translation;
    }
}

