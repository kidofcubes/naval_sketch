use core::f32;
use std::{collections::VecDeque, sync::Arc};

use bevy_egui::EguiContexts;
use enum_collections::{EnumMap, Enumerated};
use regex::Regex;

use crate::{asset_extractor::get_all_parts, cam_movement::EditorCamera, editor_actions::{EditorActionEvent, EditorSettingChange}, editor_ui::{render_gizmos, update_command_text, update_display_text, update_selected, EditorUiPlugin, Language, PropertiesDisplayData}, editor_utils::to_touch, parsing::{AdjustableHull, BasePart, Part, Turret}, parts::{base_part_to_bevy_transform, bevy_quat_to_unity, bevy_to_unity_translation, colored_part_material, generate_adjustable_hull_mesh, get_collider, BasePartMesh, BasePartMeshes, PartData, PartRegistry}, transform_gizmo::{config::TransformPivotPoint, GizmoOrientation}, transform_gizmo_bevy::{GizmoOptions, GizmoTarget}};
use bevy::{app::{DynEq, Plugin, Startup, Update}, asset::{AssetPath, AssetServer, Assets, Handle, RenderAssetUsages}, color::{Color, Luminance, Srgba}, ecs::{event::{EventCursor, EventReader, Events}, query::Or, schedule::IntoSystemConfigs, system::{Local, SystemState}, world::{OnAdd, OnRemove, World}}, gltf::GltfAssetLabel, hierarchy::ChildBuilder, image::Image, input::{keyboard::{Key, KeyboardInput}, mouse::{MouseScrollUnit, MouseWheel}, ButtonInput}, log::info, math::{bounding::BoundingVolume, primitives::Cuboid, Dir3, Isometry3d, Quat, UVec2, Vec2, Vec3}, pbr::{DirectionalLight, MeshMaterial3d, StandardMaterial}, picking::{focus::HoverMap, mesh_picking::ray_cast::{MeshRayCast, RayCastSettings}, pointer::{PointerInteraction, PointerPress}, PickingBehavior}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Gizmos, HierarchyQueryExt, KeyCode, Mesh3d, Out, Over, Parent, Pointer, PointerButton, Query, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, render::{camera::{ClearColorConfig, OrthographicProjection, Projection, Viewport}, mesh::Mesh, view::RenderLayers, RenderPlugin}, scene::{SceneInstance, SceneRoot}, tasks::{futures_lite::future, Task}, text::{TextColor, TextFont, TextLayout}, transform::components::GlobalTransform, ui::{widget::ImageNode, BackgroundColor, FlexDirection, FlexWrap, Node, Overflow, PositionType, ScrollPosition, TargetCamera, UiRect, Val}, utils::{default, HashMap}, window::Window};
use rand::{rngs::SmallRng, Rng, SeedableRng};



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
                queued_actions: VecDeque::new(),
                clipboard: Vec::new(),
                latest_selected: None,
                language: Language::CN,
            }
        );
        app.insert_resource(
            EditorOptions {
                floating: false,
                edit_near: true,
                group_edit_attributes: false,
                gizmos_activated: true,
                group_gizmos: true,
                local_gizmo: true,
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

        command_tree.add_command(b"y");
        command_tree.add_command(b"p");

        command_tree.add_command(b"f");
        command_tree.add_command(b"F");

        command_tree.add_command(b"L");

        command_trees[CommandMode::Translation]=command_tree;


        let mut command_tree = CommandTree::default();

        command_tree.add_command(b"q");
        command_tree.add_command(b"e");
        command_tree.add_command(b"w");
        command_tree.add_command(b"a");
        command_tree.add_command(b"s");
        command_tree.add_command(b"d");

        command_tree.add_command(b" ");

        command_tree.add_command(b"L");

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
                (on_gizmo_update,on_part_changed).chain(),
                command_typing,
                update_command_text,
                execute_queued_commands,
                render_gizmos,
                debug_gizmos
        ));
        app.add_observer(on_click);
        // app.insert_resource(TestResource(bevy::tasks::IoTaskPool::get().spawn(async {
        //     return get_all_parts(None).await.unwrap();
        // })));
        let the_thing = Arc::new(std::sync::Mutex::new(None));
        app.insert_resource(TestResource(the_thing.clone()));

        #[cfg(target_arch = "wasm32")]
        bevy::tasks::IoTaskPool::get()
            .spawn_local(async move {
                let stuff = get_all_parts(None).await.unwrap();
                let mut thingy = the_thing.lock().unwrap();
                *thingy = Some(stuff);
            })
            .detach();
        // Otherwise, just block for it to complete
        // #[cfg(not(target_arch = "wasm32"))]
        // futures_lite::future::block_on(async_renderer);

        // RenderPlugin
    }

    fn ready(&self, app: &bevy::prelude::App) -> bool {

        app.world()
            .get_resource::<TestResource>()
            .and_then(|frr| frr.0.try_lock().map(|locked| locked.is_some()).ok())
            .unwrap_or(true)
        //return app.world().resource::<TestResource>().0.is_finished();
        // bevy::tasks::block_on(future::poll_once())
        //return app.world().resource::<TestResource>().0.is_finished();
    }
    fn cleanup(&self, app: &mut bevy::app::App) {
        let thing = app.world_mut().remove_resource::<TestResource>().unwrap();
        // let result = futures::executor::block_on(thing.unwrap().0);
        let result = thing.0.lock().unwrap().take().unwrap();
        let mut part_registry = app.world_mut().resource_mut::<PartRegistry>();
        info!("THING WORKSINASKJDA");

        for part in result {
            info!("ADDED PART {:?}",part);
            part_registry.parts.insert(part.id,part);
        }

    }
}

#[derive(Resource)]
//pub struct TestResource(bevy::tasks::Task<Vec<PartData>>);
// pub struct TestResource(bevy::tasks::Task<Vec<PartData>>);
struct TestResource(
    Arc<
        std::sync::Mutex<
            Option<Vec<PartData>>,
        >,
    >,
);

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
    pub queued_actions: VecDeque<EditorActionEvent>, //use deque?
    pub clipboard: Vec<Part>,
    pub language: Language,
    pub latest_selected: Option<Entity>
}

#[derive(Resource)]
pub struct EditorOptions {
    pub floating: bool,
    pub edit_near: bool,
    pub group_edit_attributes: bool,
    pub gizmos_activated: bool,
    pub group_gizmos: bool,
    pub local_gizmo: bool,
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


    // let mut flip_floating = false;
    //
    // if flip_floating {
    //     editor_data.floating=!editor_data.floating;
    // }


    //for command in editor_data.queued_actions.into_iter() {

    let mut queued_actions = std::mem::take(&mut editor_data.queued_actions);

    while(queued_actions.len()>0){
        world.trigger(queued_actions.pop_front().unwrap());
    }
}

pub fn translate_floatings(
    editor_options: Res<EditorOptions>,
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
            if editor_options.floating {
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

    if editor_options.floating {
        if selected_query.iter().len() != 1 { return; }

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
    mut editor_options: ResMut<EditorOptions>,
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
                        match command_data.mode {
                            CommandMode::Translation => match command_match.as_str() {
                                "W" => {editor_data.queued_actions.push_front(EditorActionEvent::MoveRelativeDir { vector: Vec3::NEG_Z, mult: mult });},
                                "A" => {editor_data.queued_actions.push_front(EditorActionEvent::MoveRelativeDir { vector: Vec3::NEG_X, mult: mult });},
                                "S" => {editor_data.queued_actions.push_front(EditorActionEvent::MoveRelativeDir { vector: Vec3::Z, mult: mult });},
                                "D" => {editor_data.queued_actions.push_front(EditorActionEvent::MoveRelativeDir { vector: Vec3::X, mult: mult });},
                                "Q" => {editor_data.queued_actions.push_front(EditorActionEvent::MoveRelativeDir { vector: Vec3::NEG_Y, mult: mult });},
                                "E" => {editor_data.queued_actions.push_front(EditorActionEvent::MoveRelativeDir { vector: Vec3::Y, mult: mult });},
                                //"W" => {editor_command_writer.send(EditorCommandEvent::MoveRelativeDir { vector: *Dir3::NEG_Z, mult: mult });},



                                "w" => {editor_data.queued_actions.push_front(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::NEG_Z, mult: mult });},
                                "a" => {editor_data.queued_actions.push_front(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::NEG_X, mult: mult });},
                                "s" => {editor_data.queued_actions.push_front(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::Z, mult: mult });},
                                "d" => {editor_data.queued_actions.push_front(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::X, mult: mult });},
                                "q" => {editor_data.queued_actions.push_front(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::NEG_Y, mult: mult });},
                                "e" => {editor_data.queued_actions.push_front(EditorActionEvent::SmartMoveRelativeDir { dir: Dir3::Y, mult: mult });},

                                "y" => {editor_data.queued_actions.push_front(EditorActionEvent::Copy {});},
                                "p" => {editor_data.queued_actions.push_front(EditorActionEvent::Paste { selected: true });},

                                "f" => {command_data.mode = CommandMode::Attributes}

                                //"F" => {editor_data.queued_actions.push_front(EditorActionEvent::SetEditorSetting { change: EditorSettingChange { floating: Some(), ..default()} });}
                                "F" => {editor_options.floating = !editor_options.floating;}
                                _ => {}
                            },
                            CommandMode::Attributes => match command_match.as_str() {
                                "w" => {editor_data.queued_actions.push_front(EditorActionEvent::SwitchSelectedAttribute{offset:-1,do_loop:false});},
                                "s" => {editor_data.queued_actions.push_front(EditorActionEvent::SwitchSelectedAttribute{offset:1 ,do_loop:false});},
                                "a" => {editor_data.queued_actions.push_front(EditorActionEvent::SwitchSelectedAttribute{offset:-5,do_loop:false});},
                                "d" => {editor_data.queued_actions.push_front(EditorActionEvent::SwitchSelectedAttribute{offset:5 ,do_loop:false});},

                                " " => {editor_data.queued_actions.push_front(EditorActionEvent::SetAttribute {attribute: None, value: mult.to_string()});},

                                "L" => {
                                    let orig_lang = editor_data.language.clone();
                                    editor_data.queued_actions.push_front(EditorActionEvent::SetEditorSetting {
                                        change: EditorSettingChange {language: Some(
                                                    Language::VARIANTS[((Language::VARIANTS.iter().position(|x| *x==orig_lang).unwrap()+1)%Language::SIZE)]
                                                ), ..default() }
                                    });
                                },
                                _ => {}
                            },
                            CommandMode::Rotation => todo!(),
                            CommandMode::Disabled => todo!(),
                        }

        

                        let mut history: String= String::new();
                        if mult!=1.0 {
                            history.push_str(&mult.to_string());
                        }
                        history.push_str(&command_match.as_str());
                        command_data.command_history.push_front(history);
                        command_data.command_history.truncate(100);
                        
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

pub fn on_gizmo_update(
    // mut changed_gizmo_parts: Query<(&Transform, &mut BasePart, Entity), Changed<GizmoTarget>>,
    // gizmo_targets: Query<&GizmoTarget>,

    mut gizmo_parts: Query<(&Transform, &mut BasePart, &GizmoTarget)>,
    editor_data: Res<EditorData>,
    editor_options: Res<EditorOptions>,
    mut gizmo_options: ResMut<GizmoOptions>,
){
    for mut gizmo_part in &mut gizmo_parts {
        if gizmo_part.2.is_active() {

            gizmo_part.1.position = bevy_to_unity_translation(&gizmo_part.0.translation);
            gizmo_part.1.rotation = bevy_quat_to_unity(&gizmo_part.0.rotation);
            // println!("changed scale from {:?} to {:?}",gizmo_part.1.scale,gizmo_part.0.scale.abs());
            gizmo_part.1.scale = gizmo_part.0.scale.abs();
        }
    }
    
    gizmo_options.group_targets = editor_options.group_gizmos;
    gizmo_options.gizmo_orientation = if editor_options.local_gizmo { GizmoOrientation::Local } else { GizmoOrientation::Global };
    if gizmo_options.group_targets {
        if let Some(entity) = editor_data.latest_selected {
            if let Ok(thing) = gizmo_parts.get(entity) {
                gizmo_options.pivot_point = TransformPivotPoint::Point(thing.0.translation.into());
            }else{
                gizmo_options.pivot_point = TransformPivotPoint::MedianPoint;
            }
        }
    }else{
        gizmo_options.pivot_point = TransformPivotPoint::MedianPoint;
    }
    // for mut changed_gizmo_part in &mut changed_gizmo_parts {
    //     if let Some(latest_result) = gizmo_targets.get(changed_gizmo_part.2).unwrap().latest_result(){
    //         //println!("the latest result is {:?}",latest_result);
    //         changed_gizmo_part.1.position = bevy_to_unity_translation(&changed_gizmo_part.0.translation);
    //         changed_gizmo_part.1.rotation = bevy_quat_to_unity(&changed_gizmo_part.0.rotation);
    //         println!("changed scale from {:?} to {:?}",changed_gizmo_part.1.scale,changed_gizmo_part.0.scale.abs());
    //         //changed_gizmo_part.1.scale = changed_gizmo_part.0.scale.abs();
    //     }
    // }
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
    editor_options: Res<EditorOptions>,
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
            // // TODO THIS IS STUPID
            // for child in children_query.iter_descendants(pair.1) {
            //     commands.entity(child).despawn();
            // }
            // //commands.entity(pair.1).clear_children();
            //
            //
            // if let Some(new_part_data) = part_registry.parts.get(&parts.get(pair.1).unwrap().0.id) {
            //     let asset_path = AssetPath::from(new_part_data.model.clone());
            //     let mut handle = asset_server.get_handle(&asset_path);
            //     if handle.is_none() {
            //         handle = Some(asset_server.load(
            //              GltfAssetLabel::Scene(0).from_asset(
            //                  asset_path
            //              )
            //          ));
            //     }
            //
            //     if let Ok(mut part_meshes) = base_part_meshes.get_mut(pair.1) {
            //         part_meshes.meshes.clear();
            //     }
            //     commands.entity(pair.1)
            //         .remove::<(SceneInstance,Children)>()
            //         .insert(SceneRoot(handle.unwrap()))
            //     ;
            // }
            
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

    update_display_text(&selected_parts, editor_options.group_edit_attributes, &mut display_properties);
}


pub fn on_click(
    click: Trigger<Pointer<Down>>,
    base_part_query: Query<&BasePartMesh>,
    selected: Query<Entity, With<Selected>>,
    parent_query: Query<&Parent>,
    key: Res<ButtonInput<KeyCode>>,
    gizmo_targets: Query<&GizmoTarget>,
    world: &World,
    mut commands: Commands,
){
    if click.event().button != PointerButton::Primary {
        return;
    }
    if !gizmo_targets.iter().all(|target| !target.is_focused() && !target.is_active()) {
        return;
    }
    
        println!("first parent is {:#?}", world.inspect_entity(click.entity())
                         .map(|info| info.name())
                         .collect::<Vec<_>>());

    for check_entity in std::iter::once(click.entity()).chain(parent_query.iter_ancestors(click.entity())) {
        

    }
    if let Ok(base_part_mesh) = base_part_query.get(click.entity()) {
        println!("CLICKED ON A THING");
        let clicked = base_part_mesh.base_part;

        
        if (!key.pressed(KeyCode::ControlLeft)) && (!selected.contains(clicked)) {
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


