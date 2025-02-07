use core::f32;
use std::{fmt::Display, iter::once, ops::{Deref, DerefMut}};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, ColorToComponents, Luminance, Srgba}, ecs::{event::EventCursor, query::{self, Or}, world::{OnAdd, OnRemove}}, hierarchy::ChildBuilder, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{bounding::BoundingVolume, Dir3, EulerRot, FromRng, Isometry3d, Quat, Vec2, Vec3}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, GizmoPrimitive3d, Gizmos, GlobalTransform, HierarchyQueryExt, KeyCode, Local, Mesh3d, MeshRayCast, Out, Over, Parent, Plane3d, Pointer, PointerButton, Query, RayCastSettings, Ref, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, text::{TextColor, TextFont, TextLayout}, ui::{BackgroundColor, FlexDirection, Node, PositionType, Val}, utils::{default, HashMap}, window::Window};
use enum_collections::{EnumMap, Enumerated};
use rand::{rngs::{mock::StepRng, SmallRng, StdRng}, Rng, SeedableRng};
use regex::Regex;

use crate::{editor::{CommandData, Selected}, editor_utils::{aabb_from_transform, cuboid_face, cuboid_face_normal, get_nearby, get_relative_nearbys, simple_closest_dist, transform_from_aabb}, parsing::{AdjustableHull, BasePart, HasBasePart, Part, Turret}, parts::{base_part_to_bevy_transform, get_collider, unity_to_bevy_translation, PartRegistry}};

pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(
            PropertiesDisplayData {
                displays: EnumMap::new_option(),
                selected: PartAttributes::PositionX
            }
        );

        app.add_observer(on_hover);
        app.add_observer(on_unhover);
        app.add_observer(on_click);
        app.add_observer(
            |
                trigger: Trigger<OnAdd, Selected>,
                parts: Query<(&BasePart, Option<&AdjustableHull>, Option<&Turret>), With<Selected>>,
                mut text_query: Query<&mut Text>,
                display_properties: Res<PropertiesDisplayData>
            | {
                let selected_parts: Vec<(&BasePart, Option<&AdjustableHull>, Option<&Turret>)> = parts.iter().collect();
                update_display_text(&selected_parts, &mut text_query, &display_properties);
            }
        );
        app.add_observer(
            |
                trigger: Trigger<OnRemove, Selected>,
                parts: Query<(&BasePart, Option<&AdjustableHull>, Option<&Turret>, Entity), With<Selected>>,
                mut text_query: Query<&mut Text>,
                display_properties: Res<PropertiesDisplayData>
            | {
                let selected_parts: Vec<(&BasePart, Option<&AdjustableHull>, Option<&Turret>)> = parts.iter().filter_map(|part| {
                    if part.3 == trigger.entity() { None } else { Some((part.0,part.1,part.2)) }
                }).collect();
                update_display_text(&selected_parts, &mut text_query, &display_properties);
            }
        );
        app.add_systems(Startup, (spawn_ui));
        app.add_systems(Update, (on_part_display_changed));
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
    }
}


#[derive(Resource)]
pub struct CommandDisplayData {
    pub mult: f32,
    pub font_size: f32,
    pub font_width: f32,
    pub input_text_display: Option<Entity>,
    pub history_text_display: Option<Entity>,
    pub flasher: Option<Entity>,
}

#[derive(Resource)]
pub struct PropertiesDisplayData {
    pub displays: EnumMap<PartAttributes,Option<Entity>,{PartAttributes::SIZE}>, 
    pub selected: PartAttributes,
}

/// Spawn a bit of UI text to explain how to move the player.
pub fn spawn_ui(
    asset_server: Res<AssetServer>,
    mut font_data: ResMut<CommandDisplayData>,
    mut properties_display_data: ResMut<PropertiesDisplayData>,
    mut commands: Commands
) {
    font_data.mult = 2.0;
    font_data.font_size = 13.0;
    font_data.font_width = 6.0;

    //bottom command bar
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

    //right properties panel
    let font = TextFont {
        font: asset_server.load("/usr/share/fonts/TTF/CozetteVector.ttf"),
        font_size: font_data.font_size*font_data.mult, 
        ..default()
    };


    commands
        .spawn((Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(12.0),
                height: Val::Auto,
                width: Val::Px(40.0*(font_data.font_width*font_data.mult)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba_u8(64, 64, 64, 128))
        ))
        .with_children(|parent| {
            for attr in PartAttributes::VARIANTS {
                properties_display_data.displays[*attr]=Some(attribute_editor(parent, *attr, font.clone()));
            }
        })
    ;
}

fn on_part_display_changed(
    mut text_color_query: Query<&mut TextColor>,
    display_properties: Res<PropertiesDisplayData>
){
    if display_properties.is_changed() {
        for attribute in PartAttributes::VARIANTS {
            text_color_query.get_mut(display_properties.displays[*attribute].unwrap()).unwrap().0 = Color::srgb_u8(255, 255, 255);
        }
        text_color_query.get_mut(display_properties.displays[display_properties.selected].unwrap()).unwrap().0 = Color::srgb_u8(128, 128, 255);
    }
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
    pub fn set_field(&self, base_part: &mut BasePart, adjustable_hull: Option<&mut AdjustableHull>, turret: Option<&mut Turret>, text: &str) -> Result<(),Box<dyn std::error::Error>>{
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
}


fn attribute_editor(parent: &mut ChildBuilder, attribute: PartAttributes, font: TextFont) -> Entity{
    return parent.spawn((
            Node {
                left: Val::Px(12.0),
                position_type: PositionType::Relative,
                ..default()
            },
            Text::new(format!("{:?}: ???",attribute)),
            TextColor::WHITE,
            font,
    )).id();
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
    mut changed_base_part: Query<(&mut Transform, Entity), Or<(Changed<BasePart>,Changed<AdjustableHull>,Changed<Turret>)>>,
    //parts: Query<(Ref<BasePart>, Option<Ref<AdjustableHull>>, Option<Ref<Turret>>)>,
    parts: Query<(&BasePart, Option<&AdjustableHull>, Option<&Turret>)>,
    selected: Query<Entity, With<Selected>>,
    mut text_query: Query<&mut Text>,
    display_properties: Res<PropertiesDisplayData>
){
    let mut has_changed = false;
    for mut pair in &mut changed_base_part {
        println!("THE THING CHANGED OH MAI GAH {:?}",pair);
        let new_transform =
            base_part_to_bevy_transform(&parts.get(pair.1).unwrap().0);
        pair.0.translation = new_transform.translation;
        pair.0.rotation = new_transform.rotation;
        pair.0.scale = new_transform.scale;
        has_changed = true;
    }
    if !has_changed {return;}

    let mut selected_parts = Vec::with_capacity(selected.iter().len());
    for selected_part in &selected {
        selected_parts.push(parts.get(selected_part).unwrap());
    }

    update_display_text(&selected_parts, &mut text_query, &display_properties);
}


pub fn update_display_text(
    parts: &[(&BasePart, Option<&AdjustableHull>, Option<&Turret>)],
    text_query: &mut Query<&mut Text>,
    display_properties: &Res<PropertiesDisplayData>
){

    let mut selected_properties: EnumMap<PartAttributes,Vec<String>,{PartAttributes::SIZE}> = EnumMap::new_default();
    for attr in PartAttributes::VARIANTS {
        selected_properties[*attr] = Vec::new();
    }

    for part in parts {
        for attr in PartAttributes::VARIANTS {
            if let Some(string) = attr.get_field(part.0, part.1, part.2) {
                selected_properties[*attr].push(string);
            }
        }
    }

    for attr in PartAttributes::VARIANTS {
        //if !selected_properties[*attr].is_empty() {
        text_query.get_mut(display_properties.displays[*attr].unwrap()).unwrap().0 = format!("{:?}: {}",attr,recompute_display_text(&selected_properties[*attr]));
        //}
    }


}



fn recompute_display_text(values: &Vec<String>) -> String {
    if values.is_empty() { return "???".to_owned(); }
    let orig = values.first().unwrap();
    for check in values {
        if orig!=check {
            return "XXX".to_owned();
        }
    }
    return orig.to_owned();
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

