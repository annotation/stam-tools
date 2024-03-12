use stam::{AnnotationDataSet, AnnotationStore, Configurable, StoreFor, TextResource};

pub fn annotate(
    store: &mut AnnotationStore,
    resourcefiles: &[&str],
    setfiles: &[&str],
    annotationfiles: &[&str],
) -> Result<(), String> {
    for filename in setfiles {
        let annotationset = AnnotationDataSet::from_file(filename, store.config().clone())
            .map_err(|err| format!("Error loading AnnotationDataSet {}: {}", filename, err))?;
        store
            .insert(annotationset)
            .map_err(|err| format!("Error adding AnnotationDataSet {}: {}", filename, err))?;
    }
    for filename in resourcefiles {
        let resource = TextResource::from_file(filename, store.config().clone())
            .map_err(|err| format!("Error loading TextResource {}: {}", filename, err))?;
        store
            .insert(resource)
            .map_err(|err| format!("Error adding TextResource {}: {}", filename, err))?;
    }
    for filename in annotationfiles {
        store
            .annotate_from_file(filename)
            .map_err(|err| format!("Error parsing annotations from {}: {}", filename, err))?;
    }
    Ok(())
}
