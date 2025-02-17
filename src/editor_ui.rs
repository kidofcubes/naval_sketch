use core::f32;
use std::{fmt::Display, iter::once, path::Path};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetPath, AssetServer, Assets, RenderAssetUsages}, color::{Color, Luminance, Srgba}, ecs::{event::EventReader, query::Or, schedule::IntoSystemConfigs, world::{OnAdd, OnRemove}}, gltf::GltfAssetLabel, hierarchy::ChildBuilder, input::{mouse::{MouseScrollUnit, MouseWheel}, ButtonInput}, math::{bounding::BoundingVolume, primitives::Cuboid, Isometry3d, Quat, UVec2, Vec2, Vec3}, pbr::{DirectionalLight, MeshMaterial3d, StandardMaterial}, picking::{focus::HoverMap, PickingBehavior}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Gizmos, HierarchyQueryExt, KeyCode, Mesh3d, Out, Over, Parent, Pointer, PointerButton, Query, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, render::{camera::{ClearColorConfig, OrthographicProjection, Projection, Viewport}, mesh::Mesh, view::RenderLayers}, scene::{SceneInstance, SceneRoot}, text::{TextColor, TextFont, TextLayout}, ui::{widget::ImageNode, BackgroundColor, FlexDirection, FlexWrap, Node, Overflow, PositionType, ScrollPosition, TargetCamera, UiRect, Val}, utils::{default, HashMap}};
use bevy_egui::{egui::{self, Color32, Context, FontData, FontDefinitions, RichText, TextEdit, Vec2b}, EguiContexts};
use enum_collections::{EnumMap, Enumerated};
use rand::{rngs::SmallRng, Rng, SeedableRng};

use crate::{cam_movement::{spawn_player, EditorCamera}, editor::{CommandData, CommandMode, EditorData, Selected}, editor_actions::EditorActionEvent, editor_utils::{cuboid_face, get_nearby, simple_closest_dist, with_corner_adjacent_adjustable_hulls, AdjHullSide}, parsing::{AdjustableHull, BasePart, Turret}, parts::{base_part_to_bevy_transform, colored_part_material, generate_adjustable_hull_mesh, get_collider, register_all_parts, BasePartMeshes, PartAttributes, PartRegistry}};

pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(
            PropertiesDisplayData {
                properties_text_buffers: EnumMap::new(|| {"".to_string()}),
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
                mut display_properties: ResMut<PropertiesDisplayData>
            | {
                let selected_parts: Vec<(&BasePart, Option<&AdjustableHull>, Option<&Turret>)> = parts.iter().collect();
                //update_display_text(&selected_parts, &mut text_query, &display_properties);
                update_display_text(&selected_parts, &mut display_properties);
            }
        );
        app.add_observer(
            |
                trigger: Trigger<OnRemove, Selected>,
                parts: Query<(&BasePart, Option<&AdjustableHull>, Option<&Turret>, Entity), With<Selected>>,
                mut text_query: Query<&mut Text>,
                mut display_properties: ResMut<PropertiesDisplayData>
            | {
                let selected_parts: Vec<(&BasePart, Option<&AdjustableHull>, Option<&Turret>)> = parts.iter().filter_map(|part| {
                    if part.3 == trigger.entity() { None } else { Some((part.0,part.1,part.2)) }
                }).collect();
                //update_display_text(&selected_parts, &mut text_query, &display_properties);
                update_display_text(&selected_parts, &mut display_properties);
            }
        );
        app.add_systems(Startup, spawn_ui.after(register_all_parts).after(spawn_player));
        app.add_systems(Update, (update_scroll_position));
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
        app.add_systems(Startup, setup_ui);
        app.add_systems(Update, egui_update);
        app.insert_resource(TestData { text: "".to_owned() } );
    }
}


#[derive(Resource)]
pub struct TestData {
    pub text: String,
}
fn setup_ui(
    mut contexts: EguiContexts,
){
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("my_font".to_owned(),
       std::sync::Arc::new(
           // .ttf and .otf supported
           FontData::from_static(include_bytes!("/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc"))
       )
    );
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
    .insert(0, "my_font".to_owned());

    contexts.ctx_mut().set_fonts(fonts);


    //Context::set_fonts(, font_definitions);

}


fn egui_update(
    mut contexts: EguiContexts,
    mut editor_data: ResMut<EditorData>,
    mut test_data: ResMut<TestData>,
    mut all_parts: Query<(&mut BasePart, Option<&mut AdjustableHull>, Option<&mut Turret>, Entity)>,
    selected: Query<Entity, With<Selected>>,
    mut display_properties: ResMut<PropertiesDisplayData>
) {
    contexts.ctx_mut().memory(|mem|{
        match mem.focused() {
            Some(focused) => {
            },
            None => {},
        }

    });

    

    egui::Window::new("Part Properties|设置")
        .resizable(Vec2b::new(false,false))
        // .max_width(f32::MAX)
        // .max_height(f32::MAX)
        .show(contexts.ctx_mut(), |ui| {
            

            for attr in PartAttributes::VARIANTS {
                //properties_display_data.displays[*attr]=Some(attribute_editor(parent, *attr, font.clone()));
                ui.horizontal(|ui| {
                    let mut label = RichText::new(attr.to_string());
                    if display_properties.selected == *attr {
                        label = label
                            .background_color(Color32::from_rgb(96, 96, 96))
                            .color(Color32::from_rgb(255, 255, 255));
                    }
                    ui.label(label);
                    let text_box = ui.add(TextEdit::singleline(&mut display_properties.properties_text_buffers[*attr]));
                    if(text_box.changed()){
                        // if editor_data.edit_near {
                        //     display_properties.selected.smart_set_field(&mut all_parts, &selected_parts, &part_registry, &value.to_string());
                        // }else{
                        //     for selected_entity in &selected_parts {
                        //         let mut selected_part = all_parts.get_mut(selected_entity).unwrap();
                        //         display_properties.selected.set_field(Some(selected_part.0.as_mut()), selected_part.1.as_deref_mut(), selected_part.2.as_deref_mut(), &value.to_string());
                        //     }
                        // }

                        if display_properties.properties_text_buffers[*attr] != "" {
                            editor_data.queued_actions.push_front(
                                EditorActionEvent::SetAttribute {
                                    attribute: Some(*attr), value: display_properties.properties_text_buffers[*attr].clone()
                                }
                            );
                        }
                    }
                });
            }
            
        });
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
    pub properties_text_buffers: EnumMap<PartAttributes,String,{PartAttributes::SIZE}>, 
    pub selected: PartAttributes,
}

/// Spawn a bit of UI text to explain how to move the player.
pub fn spawn_ui(
    asset_server: Res<AssetServer>,
    mut font_data: ResMut<CommandDisplayData>,
    mut properties_display_data: ResMut<PropertiesDisplayData>,

    editor_camera: Single<Entity, With<EditorCamera>>,

    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    part_registry: Res<PartRegistry>,

    mut commands: Commands
) {
    font_data.mult = 2.0;
    font_data.font_size = 13.0;
    font_data.font_width = 6.0;

    //bottom command bar
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Px(12.0),
                ..default()
            },
            TargetCamera(*editor_camera),
        ))
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
        //font: asset_server.load("/usr/share/fonts/TTF/CozetteVector.ttf"),
        //font: asset_server.load("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
        font: asset_server.load("/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc"),
        font_size: 20.0, 
        ..default()
    };


    commands.spawn((
        RenderLayers::layer(1),
        Mesh3d(meshes.add(Cuboid::new(10.0, 10.0, 10.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(0.0,0.0,0.0)),
    )); //???
    
    let part_offset = Vec3::new(0.0,0.0,0.0);
    // for part in &part_registry.parts {
    //     //println!("PARTS ARE {:?}",part_registry.parts.len());
    //     let mut display_part = commands.spawn((
    //         RenderLayers::layer(1),
    //     ));
    //     let base_part = BasePart {
    //         id: *part.0,
    //         ignore_physics: false,
    //         position: part_offset.clone(),
    //         rotation: Vec3::ZERO,
    //         scale: Vec3 {x:1.0,y:1.0,z:1.0},
    //         color: Color::WHITE,
    //         armor: 0,
    //     };
    //
    //     let part: Part = 
    //         if let Some(weapon) = &part.1.weapon {
    //             Part::Turret(base_part, 
    //                 Turret {
    //                     manual_control: false,
    //                     elevator: None,
    //                 }
    //             )
    //
    //         }else{
    //             Part::Normal(base_part)
    //         };
    //     
    //     place_part(
    //         &mut meshes,
    //         &mut materials,
    //         &asset_server,
    //         &part_registry,
    //         &mut display_part,
    //         &part
    //     );
    //     part_offset+=Vec3::new(0.0,0.0,10.0);
    // }
    



    let ui_camera = commands.spawn((
        Camera3d {
            ..default()
        },
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::render::camera::ScalingMode::Fixed { width: 512.0, height: 512.0 },
            scale: 0.1,
            ..OrthographicProjection::default_3d()
        }),
        Camera {
            // renders after / on top of the main camera
            order: 1,
            clear_color: ClearColorConfig::None,
            viewport: Some(Viewport {
                physical_position: UVec2::new(0, 0),
                physical_size: UVec2::new(512, 512),
                ..default()
            }),
            

            
            ..default()
        },
        RenderLayers::layer(1),
        Transform::from_xyz(0.0,100.0,100.0).looking_at(Vec3::ZERO, Vec3::Y),
    )).id();

    let mut light_transform = Transform::from_xyz(500.0, 500.0, 500.0);
    light_transform = light_transform.looking_at(Vec3 {x:0.0, y:0.0, z:0.0 }, Vec3::Y);
    // light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        light_transform,
        RenderLayers::layer(1),
    ));




    println!("the mode is {:?}",asset_server.mode());

    // parts browser
    commands
        .spawn((Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                height: Val::Px(512.0),
                width: Val::Px(512.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                flex_basis: Val::Px(512.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(64, 64, 64, 128)),
            RenderLayers::layer(1),
            TargetCamera(ui_camera),
        )).with_children(|parent| {
            // parent.spawn((
            //     RenderLayers::layer(1),
            //     Mesh3d(meshes.add(Cuboid::new(10.0, 10.0, 10.0))),
            //     MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
            //     Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(0.0,0.0,0.0)),
            // ));

            for part in &part_registry.parts {
                let thumbnail_path = part.1.thumbnail.clone().unwrap_or(Path::new("/home/kidofcubes/Downloads/shiggysharp.png").to_owned());
                parent.spawn((
                    Node {
                        width: Val::Px(152.0),
                        height: Val::Px(152.0),
                        border: UiRect::all(Val::Px(12.0)),
                        ..default()
                    },
                    PickingBehavior {
                        should_block_lower: false,
                        ..default()
                    },
                    // BorderColor(RED.into()),
                    // BorderRadius::MAX,
                    // Outline {
                    //     width: Val::Px(3.0),
                    //     offset: Val::Px(3.0),
                    //     color: Color::WHITE,
                    // },
                )).with_child((
                    Node {
                        width: Val::Px(128.0),
                        height: Val::Px(128.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        ..default()
                    },
                    PickingBehavior {
                        should_block_lower: false,
                        ..default()
                    },
                    ImageNode::new(asset_server.load(AssetPath::from_path(&thumbnail_path))).with_mode(bevy::ui::widget::NodeImageMode::Auto),
                    // Transform::from_scale(Vec3::new(0.5,0.5,0.5)),
                    
                )).with_child((
                    Node {
                        bottom: Val::Px(0.0),
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba_u8(0, 0, 0, 128)),
                    Text::new(format!("{}",part.1.part_name)),
                    TextFont {
                        font: asset_server.load("/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc"),
                        font_size: 15.0, 
                        ..default()
                    },
                    PickingBehavior {
                        should_block_lower: false,
                        ..default()
                    },
                ));
            }

        })
    ;
    
}

/// Updates the scroll position of scrollable nodes in response to mouse input
pub fn update_scroll_position(
    mut mouse_wheel_events: EventReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    mut scrolled_node_query: Query<&mut ScrollPosition>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    for mouse_wheel_event in mouse_wheel_events.read() {
        let (mut dx, mut dy) = match mouse_wheel_event.unit {
            MouseScrollUnit::Line => (
                mouse_wheel_event.x * 32.0,
                mouse_wheel_event.y * 32.0,
            ),
            MouseScrollUnit::Pixel => (mouse_wheel_event.x, mouse_wheel_event.y),
        };

        if keyboard_input.pressed(KeyCode::ControlLeft)
            || keyboard_input.pressed(KeyCode::ControlRight)
        {
            std::mem::swap(&mut dx, &mut dy);
        }

        for (_pointer, pointer_map) in hover_map.iter() {
            for (entity, _hit) in pointer_map.iter() {
                if let Ok(mut scroll_position) = scrolled_node_query.get_mut(*entity) {
                    scroll_position.offset_x -= dx;
                    scroll_position.offset_y -= dy;
                }
            }
        }
    }
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
    command_data: Res<CommandData>,
    editor_data: Res<EditorData>,
    display_properties: Res<PropertiesDisplayData>,
    selected: Query<Entity, With<Selected>>,
    //all_parts: Query<(&mut BasePart,Option<&mut AdjustableHull>)>,
    all_parts: Query<(&BasePart, Option<&AdjustableHull>, Option<&Turret>, Entity)>,
    // hovered: Query<(&BasePart,Option<&AdjustableHull>),With<Hovered>>,
    // selected: Query<(&BasePart,Option<&AdjustableHull>),With<Selected>>,
    // all_parts: Query<(&BasePart,Option<&AdjustableHull>)>,
    part_registry: Res<PartRegistry>,
    mut gizmo: Gizmos
){
    // for hovered in &hovered {
    //     // gizmos.cuboid(
    //     //     get_collider(hovered.0, hovered.1, part_registry.parts.get(&hovered.0.id).unwrap()),
    //     //     Color::srgb_u8(0, 255, 0)
    //     // );
    // }

    
    for selected_entity in &selected {
        let selected = all_parts.get(selected_entity).unwrap();
        let selected_bounding_box = get_collider(selected.0, selected.1, part_registry.parts.get(&selected.0.id).unwrap());
        gizmo.cuboid(
            selected_bounding_box,
            Color::srgb_u8(0, 255, 0)
        );
    }


    if command_data.mode==CommandMode::Attributes {
        if editor_data.edit_near && display_properties.selected.is_adjustable_hull() {

            let mut all_colliders: Vec<(Transform,AdjustableHull)> = Vec::new();

            for part in &all_parts {
                let Some(adjustable_hull) = part.1.as_deref() else {continue;};
                all_colliders.push((get_collider(part.0, Some(adjustable_hull), part_registry.parts.get(&part.0.id).unwrap()),(adjustable_hull.clone())));
            }
            for selected_entity in &selected {
                let selected_part = all_parts.get(selected_entity).unwrap();
                if selected_part.1.is_some() {
                    let collider = get_collider(selected_part.0, selected_part.1.as_deref(), part_registry.parts.get(&selected_part.0.id).unwrap());
                    let adjacents = with_corner_adjacent_adjustable_hulls((&collider,selected_part.1.unwrap()), &all_colliders);

                    for origin_side in AdjHullSide::VARIANTS{
                        let Some(adjacent) = adjacents[*origin_side] else {continue;};
                        gizmo.cuboid(all_colliders[adjacent.0].0, Color::srgb_u8(255, 0, 255));
                    }

                    // let adjacents2 = adjacent_adjustable_hulls((&collider,selected_part.1.unwrap()), &all_colliders);
                    // 
                    // for adjacent in adjacents2{
                    //     gizmo.cuboid(all_colliders[adjacent.1.0].0, Color::srgb_u8(255, 0, 255));
                    // }
                }
            }
        }
    }





    if command_data.mode==CommandMode::Translation {
        let mut other_parts = Vec::new();
        for part in &all_parts {
            other_parts.push(get_collider(part.0, part.1, part_registry.parts.get(&part.0.id).unwrap()))
        }

        for selected_entity in &selected {

            let selected = all_parts.get(selected_entity).unwrap();

            let selected_bounding_box = get_collider(selected.0, selected.1, part_registry.parts.get(&selected.0.id).unwrap());

            let dir_nearbys = get_nearby(&selected_bounding_box, &other_parts,false,false /* ,&mut gizmos */);

            let possible_positions: HashMap<u8,Vec<f32>> = HashMap::new();

            for i in 0..6 as u8 {
                let selected_shared_face = cuboid_face(&selected_bounding_box,i);


                for nearby in dir_nearbys.get(&i).unwrap_or(&Vec::new()) {

                    let nearby_transform = &other_parts[nearby.0];

                    if simple_closest_dist(&selected_bounding_box, nearby_transform) > (1.0) {
                        continue;
                    }
                    //gizmo.cuboid(*nearby.0,Color::srgb_u8(0, 255, 255));

                    let face = cuboid_face(nearby_transform, nearby.1);
                    let mut dotted_dist = face.1-selected_shared_face.1;
                    dotted_dist = dotted_dist - (dotted_dist.dot(selected_shared_face.0.0.normalize())*selected_shared_face.0.0.normalize());
                    
                        
                        // cuboid_face_normal(&selected_bounding_box, &i)*
                        // ((nearby.0.translation-selected_bounding_box.translation).dot(cuboid_face_normal(&selected_bounding_box, &i)));

                    for j in 0..1 {
                        let face = cuboid_face(nearby_transform, (nearby.1+(j*3))%6);
                        //let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                        let mut thing = Isometry3d::from_translation(face.1-dotted_dist);
                        thing.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, face.0.0.normalize());

                        let color = match (nearby.1+(j*3))%6 {
                            0|3 => Color::srgb_u8(255, 0, 0),
                            1|4 => Color::srgb_u8(0, 255, 0),
                            2|5 => Color::srgb_u8(0, 0, 255),
                            _ => panic!()
                        };

                        gizmo.rect(thing, Vec2::ONE*2.0, color);

                        thing.translation = (nearby_transform.translation-dotted_dist).into();
                        gizmo.rect(thing, Vec2::ONE*2.0, color);
                    }
                }
            }
        }
    }
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
    commands: Commands,
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
    mut base_part_meshes: Query<&mut BasePartMeshes>,
    selected: Query<Entity, With<Selected>>,
    mut display_properties: ResMut<PropertiesDisplayData>,

    mut meshes_query: Query<(&mut Mesh3d, &mut MeshMaterial3d<StandardMaterial>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    children_query: Query<&Children>,
    part_registry: Res<PartRegistry>,
    mut commands: Commands,
){
    let mut has_changed = false;
    for mut pair in &mut changed_base_part {
        //println!("THE THING CHANGED OH MAI GAH {:?}",pair);
        let new_transform =
            base_part_to_bevy_transform(&parts.get(pair.1).unwrap().0);
        pair.0.translation = new_transform.translation;
        pair.0.rotation = new_transform.rotation;
        pair.0.scale = new_transform.scale;
        has_changed = true;

        if let Some(adjustable_hull) = parts.get(pair.1).unwrap().1 {
            let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList,RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
            
            generate_adjustable_hull_mesh(
                &mut mesh,
                adjustable_hull
            );

            meshes_query.get_mut(pair.1).unwrap().0.0 = meshes.add(mesh);
        } else {
            // TODO THIS IS STUPID
            for child in children_query.iter_descendants(pair.1) {
                commands.entity(child).despawn();
            }
            //commands.entity(pair.1).clear_children();


            if let Some(new_part_data) = part_registry.parts.get(&parts.get(pair.1).unwrap().0.id) {
                let asset_path = AssetPath::from(new_part_data.model.clone());
                let mut handle = asset_server.get_handle(&asset_path);
                if handle.is_none() {
                    handle = Some(asset_server.load(
                         GltfAssetLabel::Scene(0).from_asset(
                             asset_path
                         )
                     ));
                }

                if let Ok(mut part_meshes) = base_part_meshes.get_mut(pair.1) {
                    part_meshes.meshes.clear();
                }
                commands.entity(pair.1)
                    .remove::<(SceneInstance,Children)>()
                    .insert(SceneRoot(handle.unwrap()))
                ;
            }
            
        }

        if let Ok(part_meshes) = base_part_meshes.get(pair.1) {
            for mesh_entity in &part_meshes.meshes {
                meshes_query.get_mut(*mesh_entity).unwrap().1.0 = materials.add(colored_part_material(parts.get(pair.1).unwrap().0.color));
            }
        }



    }
    if !has_changed {return;}

    let mut selected_parts = Vec::with_capacity(selected.iter().len());
    for selected_part in &selected {
        selected_parts.push(parts.get(selected_part).unwrap());
    }

    update_display_text(&selected_parts, &mut display_properties);
}


pub fn update_display_text(
    parts: &[(&BasePart, Option<&AdjustableHull>, Option<&Turret>)],
    //text_query: &mut Query<&mut Text>,
    display_properties: &mut ResMut<PropertiesDisplayData>
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
        display_properties.properties_text_buffers[*attr] = recompute_display_text(&selected_properties[*attr]);
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

pub fn on_click(
    click: Trigger<Pointer<Down>>,
    part_query: Query<&BasePart>,
    parent_query: Query<&Parent>,
    children_query: Query<&Children>,
    material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    materials: ResMut<Assets<StandardMaterial>>,
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
                    material.0 = materials.add(colored_part_material(base_part.color.with_luminance(base_part.color.luminance()*2.0)));
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
                    material.0 = materials.add(colored_part_material(base_part.color));
                }
            }
            break;
        }
    };
}
