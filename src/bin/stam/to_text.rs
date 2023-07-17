use stam::{AnnotationStore, Text};
use std::process::exit;

pub fn to_text(store: &AnnotationStore, resource_ids: Vec<&str>) {
    for resource_id in resource_ids {
        if let Some(resource) = store.resource(resource_id) {
            eprintln!(
                "--------------------------- {} ---------------------------",
                resource_id
            );
            println!("{}", resource.text());
        } else {
            eprintln!("Error: Resource with ID {} does not exist", resource_id);
            exit(1);
        }
    }
}
