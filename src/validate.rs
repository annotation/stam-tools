use stam::{AnnotationStore, Configurable, ToJson};
use std::process::exit;

pub fn validate(store: &AnnotationStore, verbose: bool) {
    let result = store.to_json_string(&store.config().clone().with_use_include(false));
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
}
