use core::f32;
use std::{iter::once, ops::DerefMut};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, Luminance}, ecs::{event::EventCursor, query}, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{bounding::BoundingVolume, Dir3, EulerRot, FromRng, Isometry3d, Quat, Vec2, Vec3}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, GizmoPrimitive3d, Gizmos, GlobalTransform, HierarchyQueryExt, KeyCode, Local, Mesh3d, MeshRayCast, Out, Over, Parent, Plane3d, Pointer, PointerButton, Query, RayCastSettings, Ref, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, text::{TextFont, TextLayout}, ui::{BackgroundColor, Node, PositionType, Val}, utils::{default, HashMap}, window::Window};
use rand::{rngs::{mock::StepRng, SmallRng, StdRng}, Rng, SeedableRng};
use regex::Regex;

use crate::{editor::{CommandData, Selected}, editor_utils::{aabb_from_transform, cuboid_face, cuboid_face_normal, get_nearby, get_relative_nearbys, simple_closest_dist, transform_from_aabb}, parsing::{AdjustableHull, BasePart}, parts::{base_part_to_bevy_transform, get_collider, unity_to_bevy_translation, PartRegistry}};


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

// fn random_color() -> Color {
//     Color::srgb_u8(rand::random(), rand::random(), rand::random())
// }

fn random_color(seed: u64) -> Color {
    let mut randomnums = SmallRng::seed_from_u64(seed);
    Color::srgb_u8(randomnums.random(), randomnums.random(), randomnums.random())
}


#[derive(Component)]
pub struct Hovered{}

pub fn render_gizmos(
    mut selected: Query<Entity, With<Selected>>,
    all_parts: Query<(&mut BasePart,Option<&mut AdjustableHull>)>,
    // hovered: Query<(&BasePart,Option<&AdjustableHull>),With<Hovered>>,
    // selected: Query<(&BasePart,Option<&AdjustableHull>),With<Selected>>,
    // all_parts: Query<(&BasePart,Option<&AdjustableHull>)>,
    camera_query: Single<(&Camera, &GlobalTransform)>,
    part_registry: Res<PartRegistry>,
    mut gizmo: Gizmos
){
    // for hovered in &hovered {
    //     // gizmos.cuboid(
    //     //     get_collider(hovered.0, hovered.1, part_registry.parts.get(&hovered.0.id).unwrap()),
    //     //     Color::srgb_u8(0, 255, 0)
    //     // );
    // }

    let mut other_parts = Vec::new();
    for part in &all_parts {
        other_parts.push(get_collider(part.0, part.1, part_registry.parts.get(&part.0.id).unwrap()))
    }

    for selected_entity in &selected {

        let selected = all_parts.get(selected_entity).unwrap();

        let selected_bounding_box = get_collider(selected.0, selected.1, part_registry.parts.get(&selected.0.id).unwrap());

        let dir_nearbys = get_nearby(&selected_bounding_box, &other_parts,false,false /* ,&mut gizmos */);

        let mut possible_positions: HashMap<u8,Vec<f32>> = HashMap::new();

        gizmo.cuboid(
            selected_bounding_box,
            Color::srgb_u8(0, 255, 0)
        );
        for i in 0..6 as u8 {
            let selected_shared_face = cuboid_face(&selected_bounding_box,i);


            for nearby in dir_nearbys.get(&i).unwrap_or(&Vec::new()) {

                if simple_closest_dist(&selected_bounding_box, nearby.0) > (1.0) {
                    continue;
                }
                //gizmo.cuboid(*nearby.0,Color::srgb_u8(0, 255, 255));

                let face = cuboid_face(nearby.0, nearby.1);
                let mut dotted_dist = (face.1-selected_shared_face.1);
                dotted_dist = dotted_dist - (dotted_dist.dot(selected_shared_face.0.0.normalize())*selected_shared_face.0.0.normalize());
                
                    
                    // cuboid_face_normal(&selected_bounding_box, &i)*
                    // ((nearby.0.translation-selected_bounding_box.translation).dot(cuboid_face_normal(&selected_bounding_box, &i)));

                for j in 0..1 {
                    let face = cuboid_face(nearby.0, (nearby.1+(j*3))%6);
                    //let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                    let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                    thing.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, face.0.0.normalize());

                    let color = match((nearby.1+(j*3))%6) {
                        0|3 => Color::srgb_u8(255, 0, 0),
                        1|4 => Color::srgb_u8(0, 255, 0),
                        2|5 => Color::srgb_u8(0, 0, 255),
                        _ => panic!()
                    };

                    gizmo.rect(thing, Vec2::ONE*2.0, color);

                    thing.translation = (nearby.0.translation-dotted_dist).into();
                    gizmo.rect(thing, Vec2::ONE*2.0, color);
                }

            }
        }
    }



    // for selected in &selected{
    //     let bounding_box = get_collider(selected.0, selected.1, part_registry.parts.get(&selected.0.id).unwrap());
    //     gizmo.cuboid(
    //         bounding_box,
    //         Color::srgb_u8(0, 255, 0)
    //     );
    //     let dir_nearbys = get_relative_nearbys(&bounding_box, &other_parts, &camera_query.1.forward()/* ,&mut gizmos */);
    //     
    //     for nearby in dir_nearbys.0 {
    //         //gizmo.cuboid(transform_from_aabb(&aabb_from_transform(&bounding_box)),Color::srgb_u8(255, 0, 255));
    //         if simple_closest_dist(&bounding_box, nearby.0) > (1.0) {
    //             continue;
    //         }
    //         // gizmo.cuboid(
    //         //     *nearby.0,
    //         //     Color::srgb_u8(0, 255, 255)
    //         // );
    //         // let facing_face_side1 = cuboid_face(&bounding_box, (dir_nearbys.1+1)%6);
    //         // let facing_face_side2 = cuboid_face(&bounding_box, (dir_nearbys.1+2)%6);
    //         
    //         let dotted_dist = 
    //             cuboid_face_normal(&bounding_box, &dir_nearbys.1)*
    //             ((nearby.0.translation-bounding_box.translation).dot(cuboid_face_normal(&bounding_box, &dir_nearbys.1)));
    //         
    //         let face = cuboid_face(nearby.0,nearby.1);
    //         let mut thing = Isometry3d::from_translation(face.1);
    //         thing.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, face.0.0.normalize());
    //         //gizmo.rect(thing,Vec2::ONE*2.0,Color::srgb_u8(0, 255, 255));
    //         
    //
    //         // for i in 1..3 {
    //         //     let facing_face_side = cuboid_face(&bounding_box, (dir_nearbys.1+i)%6);
    //         //     for j in 0..2 {
    //         //         let face = cuboid_face(nearby.0, (nearby.1+i+(j*3))%6);
    //         //         let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
    //         //         //thing.rotation = Quat::from_rotation_arc(*Dir3::NEG_Z, face.0.0.normalize());
    //         //         //thing.rotation = nearby.0.rotation *;
    //         //         //gizmos.rect(thing, Vec2::new(face.0.1.length(),face.0.2.length())*2.0, Color::srgb_u8(255, 255, 0));
    //         //         
    //         //         //gizmos.rect(thing, Vec2::new(facing_face_side1.0.1.length(),facing_face_side1.0.2.length())*2.0, Color::srgb_u8(255, 255, 0));
    //         //         //gizmos.rect(thing, Vec2::new(facing_face_side.0.1.length(),facing_face_side.0.2.length())*2.0, Color::srgb_u8(255, 255, 0));
    //         //         //gizmos.rect(thing, Vec2::new(face.0.1.length(),face.0.2.length())*2.0, Color::srgb_u8(255, 255, 0));
    //         //         //gizmos.rect(thing, Vec2::new(facing_face_side.0.1.length(),facing_face_side.0.2.length())*2.0, Color::srgb_u8(255, 255, 0));
    //         //
    //         //         
    //         //         let color = random_color(((nearby.1+i+(j*3))%6).into());
    //         //
    //         //         //gizmos.arrow(bounding_box.translation, bounding_box.translation+cuboid_face_normal(&bounding_box, &((dir_nearbys.1+i+(j*3))%6)), color);
    //         //         //gizmos.arrow(bounding_box.translation+Vec3::Y, bounding_box.translation+face.0.0.normalize(), color);
    //         //         
    //         //     }
    //         // }
    //
    //         
    //         
    //     }
    //
    //     
    //     // for pair in nearbys.iter() {
    //     //     let actual_side = Dir3::new_unchecked(
    //     //         bounding_box.rotation.mul_vec3(dir_from_index(pair.0))
    //     //     );
    //     //     let color = match(pair.0){
    //     //         0 => Color::srgb_u8(255, 255, 0),
    //     //         1 => Color::srgb_u8(255, 0, 255),
    //     //         2 => Color::srgb_u8(0, 255, 255),
    //     //         3 => Color::srgb_u8(255, 0, 0),
    //     //         4 => Color::srgb_u8(0, 255, 0),
    //     //         5 => Color::srgb_u8(0, 0, 255),
    //     //         _ => {panic!("wtfrick")}
    //     //     };
    //     //
    //     //     for nearby in pair.1 {
    //     //         gizmos.cuboid(
    //     //             **nearby,
    //     //             color
    //     //         );
    //     //     }
    //     //
    //     //     // gizmos.cuboid(
    //     //     //     *nearby,
    //     //     //     Color::srgb_u8(255, 0, 255)
    //     //     // );
    //     // }
    //     // let forward_face = cuboid_face(&bounding_box, 0);
    //     // gizmos.rect(
    //     //     Isometry3d::new(forward_face.1, bounding_box.rotation),
    //     //     Vec2::new(forward_face.0.1.length()*2.0,forward_face.0.2.length()*2.0),
    //     //     Color::srgb_u8(255, 0, 0)
    //     // );
    // }
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
                //commands.entity(entity).remove::<OutlineVolume>();
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
