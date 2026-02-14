use core::str;
use std::{error::Error, fmt::Display, fs, path::Path};

use quick_xml::{events::{BytesStart, Event}, Reader};
use regex::Regex;
use anyhow::{anyhow, Result};
use crate::parts::{AdjustableHull, BasePart, Part, Turret};

pub fn get_attribute_string<'a>(e: &'a BytesStart<'a>, field_name: &str) -> Result<String> {
    //println!("checking the {:?} which was {:?}", field_name, str::from_utf8(e.try_get_attribute(field_name)?.unwrap().value.as_ref()));

    return Ok(str::from_utf8(e.try_get_attribute(field_name)?
        .ok_or(anyhow!("field {:?} missing from {:?}",field_name,e))?
        .value.as_ref())?.to_string());
}

pub fn load_save(file_path: &Path) -> Result<Vec<Part>> {
    //let xml = fs::read_to_string(&file_path).expect("Should have been able to read the file");
    let xml = fs::read_to_string(&file_path)?;

    //println!("thing is {xml}");

    let mut reader = Reader::from_str(xml.as_str());
    //reader.config_mut().trim_text(true);

    let mut parts: Vec<Part> = Vec::new();

    let mut current_part: Part = Part::Normal(BasePart::default());
    let re = Regex::new(r"([^A-Fa-f0-9])").unwrap();

    // The `Reader` does not implement `Iterator` because it outputs borrowed data (`Cow`s)
    loop {
        // NOTE: this is the generic case when we don't know about the input BufRead.
        // when the input is a &str or a &[u8], we don't actually need to use another
        // buffer, we could directly call `reader.read_event()`

        match reader.read_event() {
            Err(e) => panic!("Error at position {}: {:?}", reader.error_position(), e),
            // exits the loop when reaching end of file
            Ok(Event::Eof) => {
                break;
            },

            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"part" => {
                        for attribute_result in e.attributes() {
                            let attribute = attribute_result?;
                            match attribute.key.local_name().as_ref() {
                                b"id" => {
                                    current_part.base_part_mut().id = str::from_utf8(attribute.value.as_ref())?
                                        .parse::<i32>()?;
                                }
                                b"ignorePhysics" => {
                                    current_part.base_part_mut().ignore_physics = str::from_utf8(attribute.value.as_ref())?.to_lowercase().parse::<bool>()?
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"data" => {
                        if let Part::Normal(base_part) = current_part {
                            current_part = Part::AdjustableHull(base_part, AdjustableHull::default());
                        }
                        if let Part::AdjustableHull(_,adjustable_hull) = &mut current_part {
                            adjustable_hull.length= get_attribute_string(&e, "length")?.parse::<f32>()?;
                            adjustable_hull.height= get_attribute_string(&e, "height")?.parse::<f32>()?;
                            adjustable_hull.front_width= get_attribute_string(&e, "frontWidth")?.parse::<f32>()?;
                            adjustable_hull.back_width= get_attribute_string(&e, "backWidth")?.parse::<f32>()?;
                            adjustable_hull.front_spread= get_attribute_string(&e, "frontSpread")?.parse::<f32>()?;
                            adjustable_hull.back_spread= get_attribute_string(&e, "backSpread")?.parse::<f32>()?;
                            adjustable_hull.top_roundness= get_attribute_string(&e, "upCurve")?.parse::<f32>()?;
                            adjustable_hull.bottom_roundness= get_attribute_string(&e, "downCurve")?.parse::<f32>()?;
                            adjustable_hull.height_scale= get_attribute_string(&e, "heightScale")?.parse::<f32>()?;
                            adjustable_hull.height_offset= get_attribute_string(&e, "heightOffset")?.parse::<f32>()?;
                        }
                    }

                    b"turret" => {
                        if let Part::Normal(base_part) = current_part {
                            current_part = Part::Turret(base_part, Turret::default());
                        }
                        if let Part::Turret(_,turret) = &mut current_part {
                            turret.manual_control = get_attribute_string(&e, "manualControl")?.to_lowercase().parse::<bool>()?;
                            if let Ok(elevator_string) = get_attribute_string(&e, "evevator"){
                                turret.elevator = Some(elevator_string.parse::<f32>()?);
                            }
                        }
                    }
                    b"position" => {
                        current_part.base_part_mut().position.x = get_attribute_string(&e, "x")?.parse::<f32>()?;
                        current_part.base_part_mut().position.y = get_attribute_string(&e, "y")?.parse::<f32>()?;
                        current_part.base_part_mut().position.z = get_attribute_string(&e, "z")?.parse::<f32>()?;
                    }
                    b"rotation" => {
                        current_part.base_part_mut().rotation.x = get_attribute_string(&e, "x")?.parse::<f32>()?;
                        current_part.base_part_mut().rotation.y = get_attribute_string(&e, "y")?.parse::<f32>()?;
                        current_part.base_part_mut().rotation.z = get_attribute_string(&e, "z")?.parse::<f32>()?;
                    }
                    b"scale" => {
                        current_part.base_part_mut().scale.x = get_attribute_string(&e, "x")?.parse::<f32>()?;
                        current_part.base_part_mut().scale.y = get_attribute_string(&e, "y")?.parse::<f32>()?;
                        current_part.base_part_mut().scale.z = get_attribute_string(&e, "z")?.parse::<f32>()?;
                    }
                    b"color" => {
                        let color: u32 = u32::from_str_radix(re.replace_all(&get_attribute_string(&e, "hex")?,"").as_ref(), 16)?;
                        current_part.base_part_mut().color = [255,(color >> 16) as u8,(color >> 8) as u8,(color >> 0) as u8];
                    }
                    _ => {}
                }

                



            }
            Ok(Event::End(e)) => {
                //println!("the end is {:?}",e.name().as_ref());
                match e.name().as_ref() {
                    b"part" => {
                        parts.push(current_part);
                        current_part = Part::Normal(BasePart::default());
                    }
                    _ => {}
                }
            }

            // There are several other `Event`s we do not consider here
            _ => (),
        }
        // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        //buf.clear();
    }

    return Ok(parts);
}
