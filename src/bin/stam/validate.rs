use stam::{AnnotationStore, Configurable, SerializeMode, ToJson};
use std::process::exit;

pub fn validate(store: &AnnotationStore, verbose: bool) {
    if !store.config().use_include() {
        store.set_serialize_mode(SerializeMode::NoInclude);
    }
    let result = store.to_json_string(store.config());
    match result {
        Ok(result) => {
            if verbose {
                println!("{}", result)
            }
        }
        Err(err) => {
            eprintln!("Error during serialization: {}", err);
            exit(1);
        }
    }
    if !store.config().use_include() || !verbose {
        //reset
        store.set_serialize_mode(SerializeMode::AllowInclude);
    }
}
