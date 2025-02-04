use core::f32;
use std::{collections::VecDeque, iter::once, ops::DerefMut};

use bevy::{app::{Plugin, Startup, Update}, asset::{AssetServer, Assets}, color::{Color, Luminance}, ecs::{event::EventCursor, query}, gizmos::{self, aabb, primitives::dim3::Plane3dBuilder}, input::{keyboard::{Key, KeyboardInput}, ButtonInput}, math::{bounding::{Aabb3d, AabbCast3d, Bounded3d, BoundedExtrusion, BoundingVolume}, Dir3, Direction3d, EulerRot, Isometry3d, Quat, Ray3d, Vec2, Vec3, Vec3A}, pbr::{MeshMaterial3d, StandardMaterial}, prelude::{Added, BuildChildren, Camera, Camera3d, Changed, ChildBuild, Children, Commands, Component, DetectChanges, Down, Entity, Events, GizmoConfig, GizmoPrimitive3d, Gizmos, GlobalTransform, HierarchyQueryExt, InfinitePlane3d, KeyCode, Local, Mesh3d, MeshRayCast, Out, Over, Parent, Plane3d, Pointer, PointerButton, Primitive3d, Query, RayCastSettings, Ref, RemovedComponents, Res, ResMut, Resource, Single, Text, Transform, Trigger, With}, reflect::List, text::TextFont, ui::{BackgroundColor, Node, PositionType, Val}, utils::{default, HashMap}, window::Window};
use regex::Regex;
use smol_str::SmolStr;

use crate::{editor_ui::{on_click, on_hover, on_part_changed, on_unhover, render_gizmos, spawn_ui, update_command_text, update_selected, CommandDisplayData, Hovered}, parsing::{AdjustableHull, BasePart}, parts::{get_collider, unity_to_bevy_translation, BasePartMesh, PartRegistry}};



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

fn all_on_one_side(points: &[Vec3], center: Vec3, dir: Vec3/* , gizmo: &mut Gizmos */) -> bool {
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

fn rects_intersect(a_center: Vec3, a_axis1: Vec3, a_axis2: Vec3, b_center: Vec3, b_axis1: Vec3, b_axis2: Vec3/* , gizmo: &mut Gizmos */) -> bool{

    
    let a_points = plane_vertexes(a_center, a_axis1, a_axis2);
    let b_points = plane_vertexes(b_center, b_axis1, b_axis2);

    // gizmo.arrow(a_center,a_center+a_axis1, Color::srgb_u8(255, 255, 255));
    // gizmo.arrow(a_center,a_center+a_axis2, Color::srgb_u8(255, 255, 255));
    //
    // gizmo.sphere(Isometry3d::from_translation(a_points[0]), 0.1, Color::srgb_u8(255, 255, 255));
    // gizmo.sphere(Isometry3d::from_translation(a_points[1]), 0.1, Color::srgb_u8(255, 255, 255));
    // gizmo.sphere(Isometry3d::from_translation(a_points[2]), 0.1, Color::srgb_u8(255, 255, 255));
    // gizmo.sphere(Isometry3d::from_translation(a_points[3]), 0.1, Color::srgb_u8(255, 255, 255));


    // gizmo.arrow(b_center,b_center+b_axis1, Color::srgb_u8(255, 0, 255));
    // gizmo.arrow(b_center,b_center+b_axis2, Color::srgb_u8(255, 0, 255));
    // 
    // gizmo.sphere(Isometry3d::from_translation(b_points[0]), 0.1, Color::srgb_u8(255, 0, 255));
    // gizmo.sphere(Isometry3d::from_translation(b_points[1]), 0.1, Color::srgb_u8(255, 0, 255));
    // gizmo.sphere(Isometry3d::from_translation(b_points[2]), 0.1, Color::srgb_u8(255, 0, 255));
    // gizmo.sphere(Isometry3d::from_translation(b_points[3]), 0.1, Color::srgb_u8(255, 0, 255));

    if all_on_one_side(&a_points, b_center, b_axis1) {  return false; }
    if all_on_one_side(&a_points, b_center, b_axis2) {  return false; }
    if all_on_one_side(&b_points, a_center, a_axis1) {  return false; }
    if all_on_one_side(&b_points, a_center, a_axis2) {  return false; }

    return true;
}

fn extend_vec(vec: Vec3, num: f32) -> Vec3{
    vec.normalize()*(vec.length()+num)
}

pub fn arrow(gizmo: &mut Gizmos, start: Vec3, difference: Vec3, color: Color) {
    gizmo.arrow(start,start+difference,color);
}


pub fn get_nearby<'a>(
    origin: &Transform,
    to_check: &'a Vec<Transform>,
    check_dist: bool,
    check_offset: bool,
    /* gizmo: &mut Gizmos */
) -> HashMap<u8,Vec<(&'a Transform,u8)>> {
    let mut nearby: HashMap<u8,Vec<(&'a Transform,u8)>> = HashMap::new();
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

        let mut touching: Vec<(u8,u8)> = Vec::new();
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

                if check_dist && (dist-(normal.length()+faces[i].0.0.length())).abs() > (0.01) { //na suckS
                //if (dist-(normal.length()+faces[i].0.0.length())) > (0.01) || dist-f32::max(normal.length(),faces[i].0.0.length()) < 0.0 { //na suckS allows clipping in
                    break 'check_faces;
                }
                

                //if rects_intersect(faces[i].1, faces[i].0.1, faces[i].0.2, face.1, face.0.1, face.0.2, gizmo) {
                
                if check_offset && !rects_intersect(
                    origin.translation, extend_vec(faces[i].0.1,0.1), extend_vec(faces[i].0.2,0.1),
                    check.translation, extend_vec(face.0.1,0.1), extend_vec(face.0.2,0.1),
                    /* gizmo */
                ) {
                    break 'check_faces;
                }
                touching.push((i as u8,j as u8));
            }
        }
        for touched in touching {
            if !nearby.contains_key(&touched.0) {
                nearby.insert(touched.0,Vec::new());
            }
            nearby.get_mut(&touched.0).unwrap().push((check,touched.1));
        }

        //nearby.push(check);
    }
    return nearby;
}


pub fn get_relative_nearbys<'a>(origin: &Transform, to_check: &'a Vec<Transform>, dir: &Dir3/* , gizmo: &mut Gizmos */) -> (Vec<(&'a Transform,u8)>, u8) {
    let mut nearbys = get_nearby(origin, to_check,false,false/* , gizmo */);
    let mut best_side: u8 = 0;
    let mut best_dist: f32 = f32::MAX;
    
    for i in 0..6 {
        let num = Quat::from_rotation_arc(origin.rotation.mul_vec3(dir_from_index(&i)), **dir).to_axis_angle().1.abs();
        if num < best_dist {
            best_dist = num;
            best_side = i;
        }
    }
    // for pair in nearbys.iter() {
    //     gizmo.arrow(origin.translation,origin.translation+cuboid_face(origin, *pair.0).0.0, Color::srgb_u8(255, 0, 0));
    // }
    // gizmo.arrow(origin.translation,origin.translation+cuboid_face(origin, best_side).0.0.normalize(), Color::srgb_u8(255, 255, 255));

    
    return (
        nearbys.remove(&best_side).unwrap_or(Vec::new()),
        best_side
    );
}

pub fn round_to_axis(a: &Transform, dir: &Dir3) -> u8{
    let mut best_side: u8 = 0;
    let mut best_dist: f32 = f32::MAX;
    
    for i in 0..6 {
        let num = Quat::from_rotation_arc(a.rotation.mul_vec3(dir_from_index(&i)), **dir).to_axis_angle().1.abs();
        if num < best_dist {
            best_dist = num;
            best_side = i;
        }
    }
    return best_side;

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
        0 => {(*a.right()*s.x,*a.back()*s.z,*a.up()*s.y)}
        1 => {(*a.up()*s.y,*a.right()*s.x,*a.back()*s.z)}
        2 => {(*a.back()*s.z,*a.up()*s.y,*a.right()*s.x)}

        3 => {(*a.left()*s.x,*a.forward()*s.z,*a.down()*s.y)}
        4 => {(*a.down()*s.y,*a.left()*s.x,*a.forward()*s.z)}
        5 => {(*a.forward()*s.z,*a.down()*s.y,*a.left()*s.x)}
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


pub fn cuboid_face_normal(a: &Transform, i: &u8) -> Vec3 {
    match(i){
        0 => *a.right(),
        1 => *a.up(),
        2 => *a.back(),

        3 => *a.left(),
        4 => *a.down(),
        5 => *a.forward(),
        _ => {panic!("wtf")}
    }
}

pub fn cuboid_scale(a: &Transform, i: &u8) -> f32{
    match(i){
        0|3 => a.scale.x,
        1|4 => a.scale.y,
        2|5 => a.scale.z,
        _ => {panic!("wtf")}
    }
}



pub fn dir_from_index(i: &u8) -> Vec3 {
    match(i){
        0 => *Dir3::X,
        1 => *Dir3::Y,
        2 => *Dir3::Z,

        3 => *Dir3::NEG_X,
        4 => *Dir3::NEG_Y,
        5 => *Dir3::NEG_Z,
        _ => {panic!("wtf")}
    }
}

pub fn aabb_from_transform(a: &Transform) -> Aabb3d{
    let mut aabb = Aabb3d::new(a.translation, Vec3A::ZERO);
    for i in 0..8 {
        let vertex = cuboid_vertex(a, i).into();
        aabb.min = Vec3A::min(aabb.min, vertex);
        aabb.max = Vec3A::max(aabb.max, vertex);
    }
    return aabb;
}

pub fn transform_from_aabb(a: &Aabb3d) -> Transform{
    return Transform::from_translation(((a.min+a.max)/2.0).into()).with_scale(((a.max-a.min)).into());

}


pub fn simple_closest_dist(a: &Transform, b: &Transform) -> f32{
    let a_aabb = aabb_from_transform(a);
    let b_aabb = aabb_from_transform(b);
    let point = a_aabb.closest_point(b.translation);
    return (b_aabb.closest_point(point)-point).length();
}


