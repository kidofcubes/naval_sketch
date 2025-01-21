mod parsing;
use std::{collections::BTreeMap, env, error::Error, ffi::OsStr, fs::{self, create_dir_all, read_dir, ReadDir}, mem, path::{Path, PathBuf}, time::Duration};

use bevy::utils::HashMap;
use parsing::{get_attribute_string, ParseError};
use quick_xml::{events::{BytesStart}, Reader};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use yaml_rust2::{parser::{EventReceiver, MarkedEventReceiver, Parser, Tag}, scanner::{Marker, TScalarStyle}, yaml::Hash, Event, ScanError, Yaml};

fn main() {
    let args: Vec<String> = env::args().collect();

    let steam_folder = Path::new(&args[1]);
    let out_folder = Path::new(&args[2]);

    let workshop_folder = steam_folder.join("steamapps/workshop/content/842780/");
    let game_folder = steam_folder.join("steamapps/common/NavalArt/");

    println!("workshop folder is {:?}",workshop_folder);
    println!("game folder is {:?}",game_folder);
    // This will POST a body of `foo=bar&baz=quux`
    // let params = [("path", "")];
    // let client = reqwest::blocking::Client::new();
    // let res = client.post("http://127.0.0.1:8001/LoadFile")
    //     //.body("the exact body that is sent")
    //     .form(&params)
    //     .send().unwrap();
    // println!("response is {:?}",res);

    for workshop_item in read_dir(workshop_folder.clone()).unwrap() {
        let Ok(path) = workshop_item else {
            continue;
        };

        if !path.path().is_dir() {
            continue;
        }

        let Some(namod_file) = get_file_with_extension(read_dir(path.path()).unwrap(), "namod") else {
            println!("workshop item {:?} isn't a parts mod",path);
            continue;
        };

        let namod_stem = namod_file.file_stem().unwrap().to_str().unwrap();




        println!("file is {:?}",namod_file);


        //config file
        let config_file = get_file_with_extension(read_dir(path.path()).unwrap(), "xml").unwrap();
        //println!("GOT A MOD WITH {:?}",mod_config);
        let stem_name = config_file.file_stem().unwrap().to_str().unwrap();
        let xml_noconfig_stem=stem_name[0..stem_name.len()-6].to_string();

        if xml_noconfig_stem.to_lowercase() != namod_stem {
            println!("file names {:?} and {:?} didn't match",xml_noconfig_stem.to_lowercase(),namod_stem);
            continue;
        }

        let unity_project_dir = out_folder.join(path.path().file_stem().unwrap()).join("unity_project_extracted");
        let primary_content_dir = out_folder.join(path.path().file_stem().unwrap()).join("primary_content_extracted");

        let mut manifest_file_name = namod_file.file_name().unwrap().to_str().unwrap().to_owned();
        manifest_file_name.push_str(".manifest");
        let manifest_file = path.path().join(manifest_file_name);
        println!("THE MANIFEST FILE IS {:?}",manifest_file);


        let manifest_yaml_vec = yaml_rust2::YamlLoader::load_from_str(&fs::read_to_string(manifest_file).expect("Unable to read file")).unwrap();

        let mut prefab_paths: Vec<PathBuf> = Vec::new();
        for asset in manifest_yaml_vec[0]["Assets"].clone().into_iter() {
            //let asset_path = unity_project_dir.join("ExportedProject").join(asset.as_str().unwrap());
            let mut asset_path = PathBuf::from(asset.as_str().unwrap());
            //skip Assets
            asset_path = PathBuf::from_iter(asset_path.components().skip(1));
            println!("the new assetpath is {:?}",asset_path);

            if asset_path.extension() == Some(OsStr::new("prefab")) {
                prefab_paths.push(asset_path);
            }
        }

        for prefab_path in prefab_paths {
            let lowercased_path =  unity_project_dir.join("ExportedProject").join("Assets").join(PathBuf::from(prefab_path.to_str().unwrap().to_ascii_lowercase()));
            println!("prefab is {:?}",lowercased_path.clone());

            let components = parse_prefab(&lowercased_path);

            let part_mono_behaviour = components.iter().filter(|pair| {
                if pair.0 != "MonoBehaviour" { return false; }
                let Some(m_script) = pair.1.get("m_Script") else { return false; };
                let Some(map) = m_script.as_hash() else { return false; };
                let Some(file_id) = map.get(&Yaml::from_str("fileID")) else { return false; };
                let Some(guid) = map.get(&Yaml::from_str("guid")) else { return false; };
                let Some(type_num) = map.get(&Yaml::from_str("type")) else { return false; };
                println!("checking fileid {:?} guid {:?} type {:?}",file_id,guid,type_num);
                //println!("of {:?}",pair.1);

                return (file_id.as_i64()==Some(11500000))&&(guid.as_str()==Some("3ec293451970d5435a860c7692116d1d"))&&(type_num.as_i64()==Some(3));
            }).next().unwrap().1;

            println!("the MonoBehaviour is {:?}",part_mono_behaviour);
            
        }





        create_dir_all(unity_project_dir.clone());
        create_dir_all(primary_content_dir.clone());

        load_file(&namod_file);

        //extract_unity_project_to(&unity_project_dir);
        //extract_primary_content_to(&primary_content_dir);


        //let mod_config = load_mod_config_file(&config_file);



        

        
    }
    
    // let client = reqwest::Client::new();
    // let res = client.post("http://httpbin.org/post")
    //     .form(&params)
    //     .send()
    //     .await?;
}

//fn get_path_from_case_insensitive(start_path: &Path, relative_path: &Path) -> PathBuf {
//
//}


struct GameObject {
    components: Vec<HashMap<String,Yaml>>,
    children: Vec<Box<GameObject>>
}



fn parse_prefab(path: &Path) -> HashMap<String,HashMap<String,Yaml>>{
    let mut main_map: HashMap<String,HashMap<String,Yaml>> = HashMap::new();

    let file = fs::read_to_string(path).expect("Unable to read file");
    let mut parser = Parser::new(file.chars()).keep_tags(true);
    //let mut loader = YamlLoader::default();

    //println!("the file is {:?}",);
    //let prefab_yaml_vec = yaml_rust2::YamlLoader::load_from_str(&fs::read_to_string(path).expect("Unable to read file")).unwrap();
    //let prefab_yaml_vec = YamlLoaderTagged::load_from_parser(&mut parser).unwrap();
    let mut loader = YamlLoaderTagged::default();
    parser.load(&mut loader, true);

    if let Some(e) = loader.error {
        panic!("{:?}",e)
    }
    println!("the things are {:?} {:?}",&loader.docs,loader.doc_stack);
    let prefab_yaml_vec = loader.docs;


    for component in prefab_yaml_vec.clone().into_iter() {
        let mut base_hashmap = component.into_hash().unwrap();
        let component_type = base_hashmap.keys().next().unwrap().clone();


    }

    for component in prefab_yaml_vec.clone().into_iter() {
        let mut base_hashmap = component.into_hash().unwrap();
        let component_type = base_hashmap.keys().next().unwrap().clone();

        let mut attributes_hashmap = base_hashmap.remove(&component_type).unwrap().into_hash().unwrap();
        let mut attribute_map: HashMap<String,Yaml> = HashMap::new();

        let mut keys: Vec<Yaml> = Vec::new(); 
        attributes_hashmap.keys().for_each(|key| {keys.push(key.clone());});

        for key in keys {
            let key_str = key.clone().into_string().unwrap();
            attribute_map.insert(key_str,attributes_hashmap.remove(&key).unwrap());
            //println!("the component {:?} has attribute {:?}",component_type,attribute);
        }
        main_map.insert(component_type.into_string().unwrap(),attribute_map);
    }

    return main_map;
}


fn get_file_with_extension(dir: ReadDir, extension: &str) -> Option<PathBuf>{
    let file_option =
    dir.filter(|thing| {
        //println!("CHECKING FILE {:?} to see if it matches {:?}",thing,extension);
        let Ok(dir_entry) = thing else {
            return false;
        };
        let path = dir_entry.path();
        let Some(file_extension) = path.extension() else {
            return false;
        };
        return file_extension == extension;
    }).next();

    let Some(file) = file_option else {
        return None;
    };

    return Some(file.unwrap().path());
}

#[derive(Debug)]
struct ModConfig {
    mod_name: String,
    mod_type: i32,
    author: String,
    description: String,
    create_time: i64, //timestamp in seconds
    version: String,
    parts: Vec<ModPart>
}

#[derive(Debug)]
struct ModPart {
    path: String,
    part_class: String,
    part_name: String,
    part_description: String,
    part_nation: String,
    part_weapon_type: String,
}

// #[derive(Debug)]
// pub struct ParseError {
//     desc: String
// }
// impl Display for ParseError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f,"{}",self.desc)
//     }
// }
//
// impl std::error::Error for ParseError {}
//
// fn get_attribute_string<'a>(e: &'a BytesStart<'a>, field_name: &str) -> Result<String, Box<dyn std::error::Error>> {
//     //println!("checking the {:?} which was {:?}", field_name, str::from_utf8(e.try_get_attribute(field_name)?.unwrap().value.as_ref()));
//
//     return Ok(str::from_utf8(e.try_get_attribute(field_name)?
//         .ok_or(ParseError{desc: format!("field missing").to_string()})?
//         .value.as_ref())?.to_string());
// }

fn load_mod_config_file(file_path: &Path) -> Result<ModConfig,Box<dyn Error>> {
    let xml = fs::read_to_string(file_path).expect("Should have been able to read the file");
    let mut reader = Reader::from_str(xml.as_str());
    //reader.config_mut().trim_text(true);


    let mut mod_config: ModConfig = ModConfig {
        mod_name: "".to_string(),
        mod_type: 0,
        author: "".to_string(),
        description: "".to_string(),
        create_time: 0,
        version: "".to_string(),
        parts: Vec::new(),
    };

    // The `Reader` does not implement `Iterator` because it outputs borrowed data (`Cow`s)
    loop {
        // NOTE: this is the generic case when we don't know about the input BufRead.
        // when the input is a &str or a &[u8], we don't actually need to use another
        // buffer, we could directly call `reader.read_event()`

        match reader.read_event() {
            Err(e) => panic!("Error at position {}: {:?}", reader.error_position(), e),
            // exits the loop when reaching end of file
            Ok(quick_xml::events::Event::Eof) => {
                break;
            },

            Ok(quick_xml::events::Event::Start(e)) => {
                match e.name().as_ref() {
                    _ => {
                    }
                }
            }
            Ok(quick_xml::events::Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"config" => {
                        mod_config.mod_name = get_attribute_string(&e,"ModName")?;
                        mod_config.mod_type= get_attribute_string(&e,"ModType")?.parse::<i32>()?;
                        mod_config.author= get_attribute_string(&e,"Author")?;
                        mod_config.description= get_attribute_string(&e,"Description")?;
                        // mod_config.create_time=
                        //     NaiveDateTime::parse_from_str(&get_attribute_string(&e,"CreateTime")?, "%Y_%M_%D %H_%M_%S")?.and_utc().timestamp();
                        mod_config.version= get_attribute_string(&e,"Version")?;
                    }
                    b"Part" => {
                        mod_config.parts.push(
                            ModPart {
                                path: get_attribute_string(&e,"Path")?,
                                part_class: get_attribute_string(&e,"PartClass")?,
                                part_name: get_attribute_string(&e,"PartName")?,
                                part_description: get_attribute_string(&e, "PartDescription")?,
                                part_nation: get_attribute_string(&e, "PartNation")?,
                                part_weapon_type: get_attribute_string(&e, "PartWeaponType")?,
                            }
                        );
                    }
                    _ => {}
                }

                



            }
            Ok(quick_xml::events::Event::End(e)) => {
                match e.name().as_ref() {
                    _ => {}
                }
            }

            // There are several other `Event`s we do not consider here
            _ => (),
        }
        // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        //buf.clear();
    };
    return Ok(mod_config);

}

// parse f64 as Core schema
// See: https://github.com/chyh1990/yaml-rust/issues/51
fn parse_f64(v: &str) -> Option<f64> {
    match v {
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => Some(f64::INFINITY),
        "-.inf" | "-.Inf" | "-.INF" => Some(f64::NEG_INFINITY),
        ".nan" | "NaN" | ".NAN" => Some(f64::NAN),
        _ => v.parse::<f64>().ok(),
    }
}

/// Main structure for quickly parsing YAML.
///
/// See [`YamlLoader::load_from_str`].
#[derive(Default)]
pub struct YamlLoaderTagged {
    /// The different YAML documents that are loaded.
    docs: Vec<Yaml>,
    // states
    // (current node, anchor_id) tuple
    doc_stack: Vec<(Yaml, usize)>,
    key_stack: Vec<Yaml>,
    anchor_map: BTreeMap<usize, Yaml>,
    /// An error, if one was encountered.
    error: Option<ScanError>,
}

impl MarkedEventReceiver for YamlLoaderTagged {
    fn on_event(&mut self, ev: yaml_rust2::parser::Event, mark: Marker) {
        if self.error.is_some() {
            return;
        }
        if let Err(e) = self.on_event_impl(ev, mark) {
            self.error = Some(e);
        }
    }
}

impl YamlLoaderTagged {
    fn on_event_impl(&mut self, ev: Event, mark: Marker) -> Result<(), ScanError> {
        // println!("EV {:?}", ev);
        match ev {
            Event::DocumentStart | Event::Nothing | Event::StreamStart | Event::StreamEnd => {
                // do nothing
            }
            Event::DocumentEnd => {
                //println!("the docstack last is {:?}",self.doc_stack.last());
                match self.doc_stack.len() {
                    // empty document
                    0 => self.docs.push(Yaml::BadValue),
                    1 => self.docs.push(self.doc_stack.pop().unwrap().0),
                    _ => unreachable!(),
                }
            }
            Event::SequenceStart(aid, tag) => {
                //println!("sequence starts with a {:?} and {:?}",aid,tag);
                self.doc_stack.push((Yaml::Array(Vec::new()), aid));
            }
            Event::SequenceEnd => {
                let node = self.doc_stack.pop().unwrap();
                self.insert_new_node(node, mark)?;
            }
            Event::MappingStart(aid, tag) => {
                //println!("mapping start with {:?} and {:?}",aid,tag);
                self.doc_stack.push((Yaml::Hash(Hash::new()), aid));
                self.key_stack.push(Yaml::BadValue);
            }
            Event::MappingEnd => {
                self.key_stack.pop().unwrap();
                let node = self.doc_stack.pop().unwrap();
                self.insert_new_node(node, mark)?;
            }
            Event::Scalar(v, style, aid, tag) => {
                let node = if style != TScalarStyle::Plain {
                    Yaml::String(v)
                } else if let Some(Tag {
                    ref handle,
                    ref suffix,
                }) = tag
                {
                    //println!("TAG THING {:?} {:?}",handle,suffix);
                    if handle == "tag:yaml.org,2002:" {
                        match suffix.as_ref() {
                            "bool" => {
                                // "true" or "false"
                                match v.parse::<bool>() {
                                    Err(_) => Yaml::BadValue,
                                    Ok(v) => Yaml::Boolean(v),
                                }
                            }
                            "int" => match v.parse::<i64>() {
                                Err(_) => Yaml::BadValue,
                                Ok(v) => Yaml::Integer(v),
                            },
                            "float" => match parse_f64(&v) {
                                Some(_) => Yaml::Real(v),
                                None => Yaml::BadValue,
                            },
                            "null" => match v.as_ref() {
                                "~" | "null" => Yaml::Null,
                                _ => Yaml::BadValue,
                            },
                            _ => Yaml::String(v),
                        }
                    } else {
                        Yaml::String(v)
                    }
                } else {
                    // Datatype is not specified, or unrecognized
                    Yaml::from_str(&v)
                };

                self.insert_new_node((node, aid), mark)?;
            }
            Event::Alias(id) => {
                println!("got alias with {:?}",id);
                println!("got alias with {:?}",id);
                println!("got alias with {:?}",id);
                println!("got alias with {:?}",id);
                println!("got alias with {:?}",id);
                let n = match self.anchor_map.get(&id) {
                    Some(v) => v.clone(),
                    None => Yaml::BadValue,
                };
                self.insert_new_node((n, 0), mark)?;
            }
        }
        // println!("DOC {:?}", self.doc_stack);
        Ok(())
    }

    fn insert_new_node(&mut self, node: (Yaml, usize), mark: Marker) -> Result<(), ScanError> {
        // valid anchor id starts from 1
        if node.1 > 0 {
            println!("added thing to anchormap {:?}",node.1);
            self.anchor_map.insert(node.1, node.0.clone());
        }
        if self.doc_stack.is_empty() {
            self.doc_stack.push(node);
        } else {
            let parent = self.doc_stack.last_mut().unwrap();
            match *parent {
                (Yaml::Array(ref mut v), _) => v.push(node.0),
                (Yaml::Hash(ref mut h), _) => {
                    let cur_key = self.key_stack.last_mut().unwrap();
                    // current node is a key
                    if cur_key.is_badvalue() {
                        *cur_key = node.0;
                    // current node is a value
                    } else {
                        let mut newkey = Yaml::BadValue;
                        mem::swap(&mut newkey, cur_key);
                        if h.insert(newkey, node.0).is_some() {
                            let inserted_key = h.back().unwrap().0;
                            return Err(ScanError::new_string(
                                mark,
                                format!("{inserted_key:?}: duplicated key in mapping"),
                            ));
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    /// Load the given string as a set of YAML documents.
    ///
    /// The `source` is interpreted as YAML documents and is parsed. Parsing succeeds if and only
    /// if all documents are parsed successfully. An error in a latter document prevents the former
    /// from being returned.
    /// # Errors
    /// Returns `ScanError` when loading fails.
    pub fn load_from_str(source: &str) -> Result<Vec<Yaml>, ScanError> {
        Self::load_from_iter(source.chars())
    }

    /// Load the contents of the given iterator as a set of YAML documents.
    ///
    /// The `source` is interpreted as YAML documents and is parsed. Parsing succeeds if and only
    /// if all documents are parsed successfully. An error in a latter document prevents the former
    /// from being returned.
    /// # Errors
    /// Returns `ScanError` when loading fails.
    pub fn load_from_iter<I: Iterator<Item = char>>(source: I) -> Result<Vec<Yaml>, ScanError> {
        let mut parser = Parser::new(source);
        Self::load_from_parser(&mut parser)
    }

    /// Load the contents from the specified Parser as a set of YAML documents.
    ///
    /// Parsing succeeds if and only if all documents are parsed successfully.
    /// An error in a latter document prevents the former from being returned.
    /// # Errors
    /// Returns `ScanError` when loading fails.
    pub fn load_from_parser<I: Iterator<Item = char>>(
        parser: &mut Parser<I>,
    ) -> Result<Vec<Yaml>, ScanError> {
        let mut loader = YamlLoaderTagged::default();
        parser.load(&mut loader, true)?;
        if let Some(e) = loader.error {
            Err(e)
        } else {
            Ok(loader.docs)
        }
    }

    /// Return a reference to the parsed Yaml documents.
    #[must_use]
    pub fn documents(&self) -> &[Yaml] {
        &self.docs
    }
}

















fn load_file(path: &Path){
    let params = [("path", path)];
    let client = reqwest::blocking::Client::new();
    let res = client.post("http://127.0.0.1:8001/LoadFile")
        //.body("the exact body that is sent")
        .form(&params)
        .timeout(Duration::from_secs(600))
        .send().unwrap();
}

fn extract_unity_project_to(path: &Path){
    let params = [("path", path)];
    let client = reqwest::blocking::Client::new();
    let res = client.post("http://127.0.0.1:8001/Export/UnityProject")
        //.body("the exact body that is sent")
        .timeout(Duration::from_secs(600))
        .form(&params)
        .send().unwrap();
}

fn extract_primary_content_to(path: &Path){
    let params = [("path", path)];
    let client = reqwest::blocking::Client::new();
    let res = client.post("http://127.0.0.1:8001/Export/PrimaryContent")
        //.body("the exact body that is sent")
        .timeout(Duration::from_secs(600))
        .form(&params)
        .send().unwrap();
}
