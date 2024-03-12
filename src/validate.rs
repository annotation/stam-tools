use stam::{AnnotationStore, Configurable, ToJson};

pub fn validate(store: &AnnotationStore, verbose: bool) -> Result<(), String> {
    let result = store.to_json_string(&store.config().clone().with_use_include(false));
    match result {
        Ok(result) => {
            if verbose {
                println!("{}", result)
            }
        }
        Err(err) => {
            return Err(format!("Error during serialization: {}", err));
        }
    }
    Ok(())
}
