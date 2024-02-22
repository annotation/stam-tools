use stam::{AnnotationDataSet, AnnotationStore, Configurable, StoreFor, TextResource};
use std::process::exit;

pub fn annotate(
    mut store: AnnotationStore,
    resourcefiles: &[&str],
    setfiles: &[&str],
    storefiles: &[&str],
    annotationfiles: &[&str],
) -> AnnotationStore {
    for filename in storefiles {
        store = store.with_file(filename).unwrap_or_else(|err| {
            eprintln!("Error merging annotation store {}: {}", filename, err);
            exit(1);
        });
    }
    for filename in setfiles {
        let annotationset = AnnotationDataSet::from_file(filename, store.config().clone())
            .unwrap_or_else(|err| {
                eprintln!("Error loading AnnotationDataSet {}: {}", filename, err);
                exit(1);
            });
        store.insert(annotationset).unwrap_or_else(|err| {
            eprintln!("Error adding AnnotationDataSet {}: {}", filename, err);
            exit(1);
        });
    }
    for filename in resourcefiles {
        let resource =
            TextResource::from_file(filename, store.config().clone()).unwrap_or_else(|err| {
                eprintln!("Error loading TextResource {}: {}", filename, err);
                exit(1);
            });
        store.insert(resource).unwrap_or_else(|err| {
            eprintln!("Error adding TextResource {}: {}", filename, err);
            exit(1);
        });
    }
    for filename in annotationfiles {
        store.annotate_from_file(filename).unwrap_or_else(|err| {
            eprintln!("Error parsing annotations from {}: {}", filename, err);
            exit(1);
        });
    }
    store
}
