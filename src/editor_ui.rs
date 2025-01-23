use core::f32;
use std::{iter::once, ops::DerefMut};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, Luminance}, ecs::{event::EventCursor, query}, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{Dir3, EulerRot, FromRng, Isometry3d, Quat, Vec3}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, Gizmos, GlobalTransform, HierarchyQueryExt, KeyCode, Local, Mesh3d, MeshRayCast, Out, Over, Parent, Pointer, PointerButton, Query, RayCastSettings, Ref, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, text::{TextFont, TextLayout}, ui::{BackgroundColor, Node, PositionType, Val}, utils::default, window::Window};
use bevy_mod_outline::OutlineVolume;
use regex::Regex;

use crate::{editor::{CommandData, Selected}, parsing::{AdjustableHull, BasePart}, parts::{base_part_to_bevy_transform, get_collider, unity_to_bevy_translation, PartRegistry}};


#[derive(Resource)]
pub struct CommandDisplayData {
    pub mult: f32,
    pub font_size: f32,
    pub font_width: f32,
    pub input_text_display: Option<Entity>,
    pub history_text_display: Option<Entity>,
    pub flasher: Option<Entity>,
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
            font_data.input_text_display = Some(
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

            font_data.history_text_display = Some(
                parent.spawn_empty().insert((
                    Node {
                        width: Val::Vw(50.0),
                        height: Val::Auto,
                        left: Val::Vw(50.0),
                        bottom: Val::Px(0.0),
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    Text::new("history"),
                    TextLayout {
                        linebreak: bevy::text::LineBreak::NoWrap,
                        ..default()
                    },
                    TextFont {
                        font: asset_server.load("/usr/share/fonts/TTF/CozetteVector.ttf"),
                        font_size: font_data.font_size*font_data.mult, 
                        ..default()
                    },
                )).id()
            );

        })
    ;
}



#[derive(Component)]
pub struct Hovered{}

pub fn render_gizmos(
    hovered: Query<(&BasePart,Option<&AdjustableHull>),With<Hovered>>,
    part_registry: Res<PartRegistry>,
    mut gizmos: Gizmos
){
    for hovered in &hovered {
        // gizmos.cuboid(
        //     get_collider(hovered.0, hovered.1, part_registry.parts.get(&hovered.0.id).unwrap()),
        //     Color::srgb_u8(0, 255, 0)
        // );
    }
}




pub fn on_click(
    click: Trigger<Pointer<Down>>,
    part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    children_query: Query<&Children>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected: Query<Entity, With<Selected>>,
    key: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
){
    if click.event().button != PointerButton::Primary {
        return;
    }
    if let Some(clicked) = get_base_part_entity(&parent_query, &part_query, click.entity()) {
        
        if !key.pressed(KeyCode::ControlLeft) {
            for thing in &selected {
                commands.entity(thing).remove::<Selected>();
            }
        }
        
        if selected.contains(clicked) {
            commands.entity(clicked).remove::<Selected>();
        }else{
            commands.entity(clicked).insert(Selected{});
        }
    };
}

pub fn on_hover(
    hover: Trigger<Pointer<Over>>,
    part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    children_query: Query<&Children>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
){

    // i'm assuming iter_ancestors loops it in order of nearest parent hopfully

    for base_entity in once(hover.entity()).chain(parent_query.iter_ancestors(hover.entity())) {
        if let Ok(base_part) = part_query.get(base_entity) {
            commands.entity(base_entity).insert(Hovered{});
            for entity in once(base_entity).chain(children_query.iter_descendants(base_entity)) {
                if let Ok(mut material) = material_query.get_mut(entity) {
                    material.0 = materials.add(StandardMaterial::from_color(base_part.color.with_luminance(base_part.color.luminance()*2.0)));
                }
            }
            break;
        }
    };
}

pub fn on_unhover(
    unhover: Trigger<Pointer<Out>>,
    part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    children_query: Query<&Children>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
){
    // i'm assuming iter_ancestors loops it in order of nearest parent hopfully
    for base_entity in once(unhover.entity()).chain(parent_query.iter_ancestors(unhover.entity())) {
        if let Ok(base_part) = part_query.get(base_entity) {
            commands.entity(base_entity).remove::<Hovered>();
            for entity in once(base_entity).chain(children_query.iter_descendants(base_entity)) {
                if let Ok(mut material) = material_query.get_mut(entity) {
                    material.0 = materials.add(StandardMaterial::from_color(base_part.color));
                }
            }
            break;
        }
    };
}

pub fn get_base_part_entity(parent_query: &Query<&Parent>, part_query: &Query<&BasePart>, entity: Entity) -> Option<Entity>{
    // i'm assuming iter_ancestors loops it in order of nearest parent hopfully
    for base_entity in once(entity).chain(parent_query.iter_ancestors(entity)) {
        if part_query.get(base_entity).is_ok() {
            return Some(base_entity);
        }
    };
    return None;
}

pub fn update_selected(
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
                // commands.entity(entity).insert(
                //     OutlineVolume {
                //         visible: true,
                //         width: 5.0,
                //         colour: Color::srgba_u8(0, 255, 0, 255),
                //         ..default()
                //     }
                // );
            }
        }
    }
}

pub fn on_part_changed(
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

pub fn update_command_text(
    command_data: Res<CommandData>,
    command_display_data: Res<CommandDisplayData>,
    mut text_query: Query<&mut Text>,
    mut node_query: Query<&mut Node>,
){
    if command_data.is_changed()  {
        text_query.get_mut(command_display_data.input_text_display.unwrap()).unwrap().0 =
            String::from_utf8(command_data.current_command.clone()).unwrap();
        // println!("text_display is {:?}",command_display_data.text_display);
        // println!("text_query is {:?}",text_query.get_mut(command_display_data.text_display.unwrap()));
        // println!("flasher is {:?}",command_display_data.flasher);
        // println!("node_query is {:?}",node_query.get_mut(command_display_data.flasher.unwrap()));

        node_query.get_mut(command_display_data.flasher.unwrap()).unwrap().left =
            Val::Px(command_display_data.font_width * command_display_data.mult * (command_data.current_byte_index as f32));

        let mut history_text = String::new();


        let mut count = 0;
        for command in &command_data.command_history {
            history_text.push_str(command);
            history_text.push_str(" ");

            count+=1;
            if count >= 100 {
                break;
            }
        }

        

        text_query.get_mut(command_display_data.history_text_display.unwrap()).unwrap().0 =history_text;
    }
}
