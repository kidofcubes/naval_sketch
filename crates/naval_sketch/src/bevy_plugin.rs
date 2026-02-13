use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use bevy::prelude::*;
use crate::parts_loader::{get_all_parts, LocalPaths, PartData};

///todo split bevy specific data into its own feature
///
#[derive(Resource)]
struct ReadyMutexResource(

);


pub struct NavalSketchPlugin{
    pub init_paths: LocalPaths,
    initalizer_lock: Arc<Mutex<Option<Vec<PartData>>>>
}
impl Default for NavalSketchPlugin{
    fn default() -> Self {
        NavalSketchPlugin{
            initalizer_lock: Arc::new(Mutex::new(None))
        }
    }
}

impl Plugin for NavalSketchPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        // let data_paths = app.world().resource::<InitData>().data_paths.clone();
        // structure taken from RenderPlugin
        let lock = Arc::new(std::sync::Mutex::new(None));
        app.insert_resource(ReadyMutexResource(lock.clone()));

        let part_retriever = async move {
            let stuff = get_all_parts(Some(&self.init_paths)).await.unwrap();
            let mut thingy = lock.lock().unwrap();
            *thingy = Some(stuff);
        };



        #[cfg(target_arch = "wasm32")]
        bevy::tasks::IoTaskPool::get()
            .spawn_local(part_retriever)
            .detach();
        // Otherwise, just block for it to complete
        #[cfg(not(target_arch = "wasm32"))]
        futures_lite::future::block_on(part_retriever);
    }

    fn ready(&self, app: &bevy::prelude::App) -> bool {

        app.world()
            .get_resource::<InitDataHolder>()
            .and_then(|frr| frr.0.try_lock().map(|locked| locked.is_some()).ok())
            .unwrap_or(true)
    }
    fn cleanup(&self, app: &mut bevy::app::App) {
        let thing = app.world_mut().remove_resource::<crate::parts_loader::InitDataHolder>().unwrap();
        let result = thing.0.lock().unwrap().take().unwrap();


        // #[cfg(target_arch = "wasm32")]
        // //let mut part_registry = PartRegistry { path_prefix: Some(std::path::PathBuf::new()), parts: HashMap::new() };
        // let mut part_registry = PartRegistry { path_prefix: None, parts: HashMap::new() };
        // #[cfg(not(target_arch = "wasm32"))]
        // let mut part_registry = PartRegistry { path_prefix: Some(app.world().resource::<InitData>().data_paths.as_ref().unwrap().cache_folder.clone()), parts: HashMap::new() };
        let mut part_registry = PartRegistry { parts: HashMap::new() };

        for part in result {
            part_registry.parts.insert(part.id,part);
        }
        app.world_mut().insert_resource(part_registry);

    }
}

pub fn generate_part_registry(paths: &LocalPaths) -> PartRegistry{
    let mut registry = PartRegistry {
        parts: HashMap::new(),
    };


    // get_all_parts(None);
    // info!("DONE DOING THE THING 1");
    // let result = futures::executor::block_on(get_all_parts(init_data.data_paths.as_ref()));

    #[cfg(target_arch = "wasm32")]
    {
        // can't get the value out, or pass my resource in (also doesn't seem to block)
        // wasm_bindgen_futures::spawn_local(async {
        //     let result = get_all_parts(None).await;
        //     info!("DONE DOING THE THING 3 ");
        // });

        // fails with a panic
        // ComputeTaskPool::get().scope(|s| {
        //     s.spawn(async {
        //         let result = get_all_parts(None).await;
        //         info!("DONE DOING THE THING 3 ");
        //     });
        // });

        // hangs forever
        // futures::executor::block_on(async{
        //         let result = get_all_parts(None).await;
        //         info!("DONE DOING THE THING 4 ");
        // });

        // let result = pollster::FutureExt::block_on(get_all_parts(None));
        // info!("DONE DOING THE THING 3 ");


        // bevy::tasks::block_on(async {
        //         let result = get_all_parts(None).await;
        //         info!("DONE DOING THE THING 3 ");
        //         if let Ok(parts) = result {
        //             for part in parts {
        //                 part_registry.parts.insert(part.id,part);
        //             }
        //         }
        //         info!("DONE DOING THE THING 4 ");
        //
        //
        // });
        info!("DONE DOING THE THING 5 ");
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let result = futures_lite::future::block_on(get_all_parts(Some(paths)));
        if let Ok(parts) = result {
            for part in parts {
                registry.parts.insert(part.id,part);
            }
        }

    }
    // info!("DONE DOING THE THING 2 {:?}",result.unwrap().len());


    // let workshop_parts = get_workshop_parts(&workshop_folder, &cache_folder);
    // for workshop_port in workshop_parts {
    //     part_registry.parts.insert(workshop_port.id,workshop_port);
    // }
    //
    // let builtin_parts = get_builtin_parts(&game_folder, &cache_folder);
    // for builtin_part in builtin_parts {
    //     part_registry.parts.insert(builtin_part.id,builtin_part);
    // }

    // println!("all registered parts is {:?}",part_registry.parts.keys());
    return registry;

}
