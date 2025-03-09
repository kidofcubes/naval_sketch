use std::{error::Error, ffi::OsStr, fs::{self, create_dir_all, read_dir, File, ReadDir}, path::{Path, PathBuf}, time::Duration};

use bevy::{math::Vec3, reflect::List, utils::HashMap};
use csv::StringRecord;
use quick_xml::Reader;

use regex::Regex;
use yaml_rust2::Yaml;

use crate::{editor_ui::Language, parsing::get_attribute_string, parts::{MultiLangString, PartData, WeaponData}};


pub fn get_builtin_parts(game_folder: &Path, cache_folder: &Path) -> Vec<PartData> {
    let mut parts: Vec<PartData> = Vec::new();
    let unity_project_dir = cache_folder.join("naval_art").join("unity_project_extracted");
    let primary_content_dir = cache_folder.join("naval_art").join("primary_content_extracted");

    if (!Path::exists(&unity_project_dir)) || (!Path::exists(&primary_content_dir)) {
        load_folder(game_folder);
    }
    if !Path::exists(&unity_project_dir) {
        create_dir_all(unity_project_dir.clone()).unwrap();
        extract_unity_project_to(&unity_project_dir);
    }
    if !Path::exists(&primary_content_dir) {
        create_dir_all(primary_content_dir.clone()).unwrap();
        extract_primary_content_to(&primary_content_dir);
    }

    let part_names_path = game_folder.join("NavalArt_Data").join("Localization").join("parts.csv");
    let part_descriptions_path = game_folder.join("NavalArt_Data").join("Localization").join("descriptions.csv");

    let mut part_names_csv = csv::Reader::from_reader(File::open(part_names_path).unwrap());
    let mut part_names: HashMap<i32,StringRecord> = HashMap::new();
    for result in part_names_csv.records() {
        let record = result.unwrap();
        //println!("THING IS {:?}", record);
        let part_id = (record.get(0).unwrap()[1..]).parse::<i32>().unwrap();
        part_names.insert(part_id,record);
    }


    let mut part_descriptions: HashMap<i32,StringRecord> = HashMap::new();
    let mut part_descriptions_csv = csv::Reader::from_reader(File::open(part_descriptions_path).unwrap());

    for result in part_descriptions_csv.records() {
        let record = result.unwrap();
        //println!("THING IS {:?}", record);
        let part_id = (record.get(0).unwrap()[1..]).parse::<i32>().unwrap();
        part_descriptions.insert(part_id,record);
    }



    let parts_dir = unity_project_dir.join("ExportedProject").join("Assets").join("Resources").join("parts");
    let models_dir = primary_content_dir.join("Assets").join("PrefabHierarchyObject");
    let thumbnails_dir = unity_project_dir.join("ExportedProject").join("Assets").join("Resources").join("images");

    let part_prefab_regex = Regex::new(r"^(\d+)\.prefab$").unwrap();
    for part_file in read_dir(parts_dir.clone()).unwrap() {
        let Ok(prefab_path) = part_file else {continue;};
        if !prefab_path.path().is_file() || prefab_path.path().is_dir() {continue;}
        if !part_prefab_regex.is_match(prefab_path.path().file_name().unwrap().to_str().unwrap()) {continue;}


        
        let model_path = models_dir.join(prefab_path.path().file_stem().unwrap().to_str().unwrap().to_owned()+".glb");
        if !model_path.exists() {
            println!("model path {:?} doesn't exist",model_path);
            continue;
        }

        let thumbnail_path = thumbnails_dir.join(prefab_path.path().file_stem().unwrap().to_str().unwrap().to_owned()+".png");
        let mut thumbnail_path_option: Option<PathBuf> = None;
        if thumbnail_path.exists() {
            thumbnail_path_option = Some(thumbnail_path);
        }

        let prefab = parse_prefab(&prefab_path.path());
        let mut part_data = load_prefab(&prefab, model_path, thumbnail_path_option, false);
        if let Some(part_namez) = part_names.get(&part_data.id) {
            if let Some(name) = part_namez.get(1) { part_data.part_name = part_data.part_name.with(Language::CN, name.to_owned()); }
            if let Some(name) = part_namez.get(2) { part_data.part_name = part_data.part_name.with(Language::EN, name.to_owned()); }
        }

        if let Some(part_descriptionz) = part_descriptions.get(&part_data.id) {
            if let Some(name) = part_descriptionz.get(1) { part_data.part_description = part_data.part_description.with(Language::CN, name.to_owned()); }
            if let Some(name) = part_descriptionz.get(2) { part_data.part_description = part_data.part_description.with(Language::EN, name.to_owned()); }
        }

        parts.push(part_data);
    }

    return parts;
}



fn vec3_from_yaml(yaml: &Yaml) -> Vec3{
    return Vec3 {
        x: get_as_f32(yaml.as_hash().unwrap().get(&Yaml::from_str("x")).unwrap()),
        y: get_as_f32(yaml.as_hash().unwrap().get(&Yaml::from_str("y")).unwrap()),
        z: get_as_f32(yaml.as_hash().unwrap().get(&Yaml::from_str("z")).unwrap()),
    };
}
fn get_as_f32(value: &Yaml) -> f32{
    if let Yaml::Integer(num) = value {
        return num.clone() as f32;
    }else{
        return value.as_f64().unwrap() as f32;

    }
}
fn get_option_as_str(value: Option<&Yaml>) -> String {
    match value {
        Some(yaml) => return get_as_str(yaml),
        None => "".to_owned(),
    }
}
fn get_as_str(value: &Yaml) -> String {
    match value {
        Yaml::Real(num) => format!("{}",num),
        Yaml::Integer(num) => format!("{}",num),
        Yaml::String(str) => str.to_owned(),
        Yaml::Boolean(bool) => (if *bool {"True"}else{"False"}).to_owned(),
        Yaml::Null => "".to_owned(),
        _ => "".to_owned()
    }
}

const BASE_PART_COMPONENT_GUID: &str = "025b32fee4a7141deb25f3720956b98b";
const MOD_PART_COMPONENT_GUID: &str = "3ec293451970d5435a860c7692116d1d";
const WEAPON_COMPONENT_GUID: &str = "ba97f7bd460ed4e540710e28d77a5180";



fn get_monobehaviour<'a>(game_object: &'a GameObject, search_guid: &str) -> Option<&'a HashMap<String,Yaml>>{
    let thing = game_object.components.iter().filter(|pair| {
                if pair.0 != "MonoBehaviour" { return false; }
                let Some(m_script) = pair.1.get("m_Script") else { return false; };
                let Some(map) = m_script.as_hash() else { return false; };
                let Some(file_id) = map.get(&Yaml::from_str("fileID")) else { return false; };
                let Some(guid) = map.get(&Yaml::from_str("guid")) else { return false; };
                let Some(type_num) = map.get(&Yaml::from_str("type")) else { return false; };
                //println!("checking fileid {:?} guid {:?} type {:?}",file_id,guid,type_num);
                //println!("of {:?}",pair.1);

                return (file_id.as_i64()==Some(11500000))&&(guid.as_str()==Some(search_guid))&&(type_num.as_i64()==Some(3));
            }).next();
    if let Some(thing) = thing {
        return Some(&thing.1);
    }else{
        return None;
    }
}

//loads on a best effort basis
fn load_prefab(prefab: &GameObject, model_path: PathBuf, thumbnail_path: Option<PathBuf>, modded: bool) -> PartData {
    let part_mono_behaviour = get_monobehaviour(&prefab, if modded {MOD_PART_COMPONENT_GUID}else{BASE_PART_COMPONENT_GUID}).unwrap();
    let weapon_mono_behaviour = get_monobehaviour(&prefab, if modded {WEAPON_COMPONENT_GUID}else{WEAPON_COMPONENT_GUID});

    let box_collider = &prefab.components.iter().filter(|pair| {
        return pair.0 == "BoxCollider"
    }).next().unwrap().1;


    // println!("THING IS {:?}",part_mono_behaviour);
    // println!("doing {:?}",(part_mono_behaviour.get("partName")));
    // println!("description is {:?}",part_mono_behaviour.get("partDescription"));
    // println!("weapontype is {:?}",part_mono_behaviour.get("weaponType"));
    
    let mut part_name = MultiLangString::default();
    if let Some(yaml) = part_mono_behaviour.get("partName") { part_name = part_name.with(Language::UNSPECIFIED,get_as_str(yaml)); }

    let mut part_description = MultiLangString::default();
    if let Some(yaml) = part_mono_behaviour.get("partDescription") { part_description = part_description.with(Language::UNSPECIFIED,get_as_str(yaml)); }


    let mut part_data = PartData {
        id: part_mono_behaviour.get("id").unwrap().as_i64().unwrap() as i32,
        part_name,
        part_description,
        armor: part_mono_behaviour.get("armor").unwrap().as_i64().unwrap() as i32,
        density: get_as_f32(part_mono_behaviour.get("density").unwrap()),
        builder_class: part_mono_behaviour.get("builderClass").unwrap_or(&Yaml::Integer(-1)).as_i64().unwrap() as i32,
        weapon_type: part_mono_behaviour.get("weaponType").unwrap_or(&Yaml::Integer(-1)).as_i64().unwrap() as i32,
        nation: part_mono_behaviour.get("nation").unwrap().as_i64().unwrap() as u32,
        volume: get_as_f32(part_mono_behaviour.get("volume").unwrap()),
        price: part_mono_behaviour.get("price").unwrap().as_i64().unwrap() as i32,
        model: model_path,
        thumbnail: thumbnail_path,
        collider: vec3_from_yaml(box_collider.get("m_Size").unwrap()),
        center: vec3_from_yaml(box_collider.get("m_Center").unwrap()),
        weapon: None
    };
    if let Some(weapon_mono_behaviour) = weapon_mono_behaviour {
        part_data.weapon = Some(WeaponData {

        });
    }

    return part_data;
}



pub fn get_workshop_parts(workshop_folder: &Path, cache_folder: &Path) -> Vec<PartData> {
    println!("workshop folder is {:?}",workshop_folder);
    let mut parts: Vec<PartData> = Vec::new();

    for workshop_item in read_dir(workshop_folder).unwrap() {
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




        let unity_project_dir = cache_folder.join(path.path().file_stem().unwrap()).join("unity_project_extracted");
        let primary_content_dir = cache_folder.join(path.path().file_stem().unwrap()).join("primary_content_extracted");


        if (!Path::exists(&unity_project_dir)) || (!Path::exists(&primary_content_dir)) {
            load_file(&namod_file);
        }
        if !Path::exists(&unity_project_dir) {
            create_dir_all(unity_project_dir.clone()).unwrap();
            extract_unity_project_to(&unity_project_dir);
        }
        if !Path::exists(&primary_content_dir) {
            create_dir_all(primary_content_dir.clone()).unwrap();
            extract_primary_content_to(&primary_content_dir);
        }





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
            //println!("the new assetpath is {:?}",asset_path);

            if asset_path.extension() == Some(OsStr::new("prefab")) {
                prefab_paths.push(asset_path);
            }
        }



        let models_dir = primary_content_dir.join("Assets").join("PrefabHierarchyObject");
        let thumbnails_dir = unity_project_dir.join("ExportedProject").join("Assets").join("resources");


        for prefab_path in prefab_paths {
            let lowercased_path =  unity_project_dir.join("ExportedProject").join("Assets").join(PathBuf::from(prefab_path.to_str().unwrap().to_ascii_lowercase()));
            //println!("prefab is {:?}",lowercased_path.clone());

            let prefab = parse_prefab(&lowercased_path);

            let model_path = models_dir.join(prefab_path.file_stem().unwrap().to_str().unwrap().to_owned()+".glb");
            let thumbnail_path = thumbnails_dir.join(format!("p{}.png",prefab_path.file_stem().unwrap().to_str().unwrap().to_owned()));
            let mut thumbnail_path_option: Option<PathBuf> = None;
            if thumbnail_path.exists() {
                thumbnail_path_option = Some(thumbnail_path);
            }

            if model_path.exists() {
                parts.push(load_prefab(&prefab, model_path, thumbnail_path_option, true));
            }
        }
    }
    return parts;
}

//fn get_path_from_case_insensitive(start_path: &Path, relative_path: &Path) -> PathBuf {
//
//}


#[derive(Debug)]
struct GameObject {
    id: usize,
    components: Vec<(String,HashMap<String,Yaml>)>,
    children: Vec<Box<GameObject>>
}
impl GameObject {
    fn get_transform(&self) -> &HashMap<String,Yaml> {
        return &self.components.iter().filter(|thing2| {thing2.0=="Transform"}).next().unwrap().1;
    }

    fn take_children_recursive(&mut self, gameobjects: &mut Vec<GameObject>){
        let to_take: Vec<usize> = gameobjects.iter().enumerate().filter_map(|thing| {
            if thing.1.get_transform().get("m_Father").unwrap().as_hash().unwrap().get(&Yaml::from_str("fileID")).unwrap().as_i64().unwrap() as usize==self.id {
                return Some(thing.0);
            }else{
                return None;
            }
        }).collect();

        for item in to_take {
            self.children.push(Box::new(gameobjects.remove(item)));
        }
        for thing in self.children.iter_mut() {
            thing.take_children_recursive(gameobjects);
        }
    }
}



fn parse_prefab(path: &Path) -> GameObject{
    //println!("parsing prefab {:?}",path);
    let file = fs::read_to_string(path).expect("Unable to read file");
    let re = Regex::new(r"--- !u!\d+ &(\d+)").unwrap();
    let mut captures: Vec<((usize,usize),usize)> = Vec::new();
    for capture in re.captures_iter(&file){
        captures.push(((capture.get(0).unwrap().start(),capture.get(0).unwrap().end()),capture.get(1).unwrap().as_str().parse::<usize>().unwrap()));
    }

    let mut components: Vec<(usize,Yaml)> = Vec::new();
    for i in 0..(captures.len() - 1) {
        let slice = &file[(captures[i].0.1)..(captures[i+1].0.0)];
        components.push((captures[i].1,yaml_rust2::YamlLoader::load_from_str(slice).unwrap().pop().unwrap()));
    }
    let slice = &file[(captures.last().unwrap().0.1)..];
    components.push((captures.last().unwrap().1,yaml_rust2::YamlLoader::load_from_str(slice).unwrap().pop().unwrap()));

    let mut gameobjects: HashMap<usize,GameObject> = HashMap::new();


    for component in components {
        let mut base_hashmap = component.1.into_hash().unwrap();
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

        

        if component_type.as_str().unwrap() == "GameObject" {
            gameobjects.try_insert(component.0, GameObject { id: component.0, components: Vec::new(), children: Vec::new() });
            gameobjects.get_mut(&component.0).unwrap().components.push((component_type.as_str().unwrap().to_string(),attribute_map));
        }else {
            let gameobject_id = attribute_map.get("m_GameObject").unwrap().as_hash().unwrap().get(&Yaml::from_str("fileID")).unwrap().as_i64().unwrap() as usize;
            gameobjects.try_insert(gameobject_id, GameObject { id: gameobject_id, components: Vec::new(), children: Vec::new() });
            gameobjects.get_mut(&gameobject_id).unwrap().components.push((component_type.as_str().unwrap().to_string(),attribute_map));
        }
    }

    let root_key = gameobjects.iter().filter_map(|thing| {

        if thing.1.get_transform().get("m_Father").unwrap().as_hash().unwrap().get(&Yaml::from_str("fileID")).unwrap().as_i64().unwrap()==0 {
            return Some(thing.0);
        }else{
            return None;
        }
    }).next().unwrap().clone();
    let mut root = gameobjects.remove(&root_key).unwrap();
    let mut gameobjects: Vec<GameObject> = gameobjects.into_values().collect();
    root.take_children_recursive(&mut gameobjects);

    return root;
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










fn load_folder(path: &Path){
    let params = [("path", path)];
    generate_request("/LoadFolder")
        .form(&params)
        .send().unwrap();
}

fn load_file(path: &Path){
    let params = [("path", path)];
    generate_request("/LoadFile")
        .form(&params)
        .send().unwrap();
}

fn extract_unity_project_to(path: &Path){
    let params = [("path", path)];
    generate_request("/Export/UnityProject")
        .form(&params)
        .send().unwrap();
}

fn extract_primary_content_to(path: &Path){
    let params = [("path", path)];
    generate_request("/Export/PrimaryContent")
        .form(&params)
        .send().unwrap();
}

fn generate_request(path: &str) -> reqwest::blocking::RequestBuilder{
    let client = reqwest::blocking::Client::new();
    let mut url = "http://127.0.0.1:8001".to_owned();
    url.push_str(path);
    return client.post(&url)
        //.body("the exact body that is sent")
        .timeout(Duration::from_secs(60*60))
        // .form(params)
        // .send();

}
