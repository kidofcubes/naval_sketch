use std::{fmt::Display};
use enum_collections::{Enumerated};
use serde::{de::Visitor, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use crate::{parts_loader::PartData};
use core::f32;
use std::collections::HashMap;

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct PartRegistry {
    pub parts: HashMap<i32,PartData>,
}

use glam::Vec3;

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
pub struct BasePart {
    pub id: i32,
    pub ignore_physics: bool,
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
    pub color: [u8; 4], /// ARGB
    pub armor: i32,
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
#[cfg_attr(feature = "bevy", require(BasePart))]
pub struct AdjustableHull {
    pub length: f32,
    pub height: f32,
    pub front_width: f32,
    pub back_width: f32,
    pub front_spread: f32,
    pub back_spread: f32,
    pub top_roundness: f32,
    pub bottom_roundness: f32,
    pub height_scale: f32,
    pub height_offset: f32,
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
#[cfg_attr(feature = "bevy", require(BasePart))]
pub struct Turret{
    pub manual_control: bool,
    pub elevator: Option<f32>,
}


impl Default for BasePart {
    fn default() -> BasePart {
        BasePart {
            id: 0,
            ignore_physics: false,
            position: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            color: [255,255,255,255],
            armor: 0,
        }
    }
}


impl Default for AdjustableHull{
    fn default() -> AdjustableHull {
        AdjustableHull {
            length:6.0,
            height:6.0,
            front_width:6.0,
            back_width:6.0,
            front_spread:0.0,
            back_spread:0.0,
            top_roundness:0.0,
            bottom_roundness:1.0,
            height_scale:1.0,
            height_offset:0.0
        }
    }
}

impl Default for Turret{
    fn default() -> Turret{
        Turret {
            manual_control: true,
            elevator: None
        }
    }
}


#[derive(Debug, Copy, Clone)]
pub enum Part {
    Normal(BasePart),
    AdjustableHull(BasePart,AdjustableHull),
    Turret(BasePart,Turret),
}

impl Part{
    pub fn base_part(&self) -> &BasePart {
        match self {
            Part::Normal(part) => part,
            Part::AdjustableHull(part,_) => part,
            Part::Turret(part,_) => part
        }
    }
    pub fn base_part_mut(&mut self) -> &mut BasePart {
        match self {
            Part::Normal(part) => part,
            Part::AdjustableHull(part,_) => part,
            Part::Turret(part,_) => part
        }
    }

    pub fn from_optionals(components: (&BasePart, Option<&AdjustableHull>, Option<&Turret>)) -> Part {
        if let Some(adjustable_hull) = components.1 {
            Part::AdjustableHull(*components.0, *adjustable_hull)
        } else if let Some(turret) = components.2 {
            Part::Turret(*components.0, *turret)
        } else {
            Part::Normal(*components.0)
        }
    }

    pub fn to_optionals(&self) -> (&BasePart, Option<&AdjustableHull>, Option<&Turret>) {
        match self {
            Part::Normal(part) => (part,None,None),
            Part::AdjustableHull(part,adjustable_hull) => (part, Some(adjustable_hull), None),
            Part::Turret(part,turret) => (part, None, Some(turret))
        }
    }
}


#[derive(Enumerated, Debug, Copy, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum Language {
    CN,
    EN,
    UNSPECIFIED
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiLangString {
    pub texts: HashMap<Language,String>,
}
impl Default for MultiLangString {
    fn default() -> Self {
        MultiLangString { texts: HashMap::new() }
    }
}
impl MultiLangString {
    pub fn of(lang: Language, text: String) -> Self {
        MultiLangString::default().with(lang,text)
    }

    pub fn with(mut self, lang: Language, text: String) -> Self {
        self.texts.insert(lang,text);
        return self;
    }
    pub fn get(&self, lang: Language) -> &str {
        return self.texts.get(&lang).map(String::as_str).unwrap_or(self.get_fallback());
    }
    pub fn get_fallback(&self) -> &str {
        for lang in Language::VARIANTS {
            if let Some(text) = self.texts.get(lang).as_ref() {
                return text;
            }
        }
        return "NO TEXT FOUND";
    }
}



#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct WeaponData {
}

pub fn set_adjustable_hull_width(
    hull: &mut AdjustableHull,
    back: &bool,
    bottom: &bool,
    value: &f32
){
    if *back {
        if *bottom {
            hull.back_spread=(hull.back_spread+hull.back_width)-value;
            hull.back_width=*value;
        }else{
            hull.back_spread=value-hull.back_width;
        }
    }else{
        if *bottom {
            hull.front_spread=(hull.front_spread+hull.front_width)-value;
            hull.front_width=*value;
        }else{
            hull.front_spread=value-hull.front_width;
        }
    }
}