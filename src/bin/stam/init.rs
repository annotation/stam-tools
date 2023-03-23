use stam::{AnnotationStore, Config, Configurable};

use crate::annotate::*;

pub fn init(
    resourcefiles: &[&str],
    setfiles: &[&str],
    storefiles: &[&str],
    annotationfiles: &[&str],
    id: Option<&str>,
    config: Config,
) -> AnnotationStore {
    let mut store = AnnotationStore::new().with_config(config);
    store = annotate(store, resourcefiles, setfiles, storefiles, annotationfiles);
    if let Some(id) = id {
        store = store.with_id(id.to_string());
    }
    store
}
