use stam::*;
use std::collections::BTreeSet;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SplitMode {
    Retain,
    Delete,
}

pub fn split<'a>(store: &'a mut AnnotationStore, queries: Vec<Query<'a>>, mode: SplitMode) {
    let mut split_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
    let mut split_resources: BTreeSet<TextResourceHandle> = BTreeSet::new();
    let mut split_datasets: BTreeSet<AnnotationDataSetHandle> = BTreeSet::new();
    for query in queries {
        let iter = store.query(query).expect("query failed");
        let names = iter.names();
        for resultrow in iter {
            if let Ok(result) = resultrow.get_by_name_or_last(&names, Some("split")) {
                match result {
                    QueryResultItem::None => {}
                    QueryResultItem::Annotation(annotation) => {
                        split_annotations.insert(annotation.handle());
                    }
                    QueryResultItem::TextResource(resource) => {
                        split_resources.insert(resource.handle());
                    }
                    QueryResultItem::AnnotationDataSet(dataset) => {
                        split_datasets.insert(dataset.handle());
                    }
                    _ => {
                        eprintln!("Error: Obtained result type is invalid for split, only ANNOTATION, RESOURCE and DATASET work.");
                    }
                }
            }
        }
    }

    match mode {
        SplitMode::Delete => {
            //split_* sets contains resources to delete
            if !split_resources.is_empty() {
                for resource in split_resources {
                    let _ = store.remove_resource(resource); //we can just ignore handle error at this point
                }
            }
            if !split_datasets.is_empty() {
                for dataset in split_datasets {
                    let _ = store.remove_dataset(dataset); //we can just ignore handle error at this point
                }
            }
            if !split_annotations.is_empty() {
                for annotation in split_annotations {
                    let _ = store.remove_annotation(annotation); //we can just ignore handle error at this point
                }
            }
        }
        SplitMode::Retain => {
            //split_* sets contains resources to retain
            if !split_resources.is_empty() {
                for resource in 0..store.resources_len() {
                    let resource = TextResourceHandle::new(resource);
                    if store.has(resource) && !split_resources.contains(&resource) {
                        let _ = store.remove_resource(resource); //we can just ignore handle error at this point
                    }
                }
            }
            if !split_datasets.is_empty() {
                for dataset in 0..store.datasets_len() {
                    let dataset = AnnotationDataSetHandle::new(dataset);
                    if store.has(dataset) && !split_datasets.contains(&dataset) {
                        let _ = store.remove_dataset(dataset); //we can just ignore handle error at this point
                    }
                }
            }
            if !split_annotations.is_empty() {
                for annotation in 0..store.annotations_len() {
                    let annotation = AnnotationHandle::new(annotation);
                    if store.has(annotation) && !split_annotations.contains(&annotation) {
                        let _ = store.remove_annotation(annotation); //we can just ignore handle error at this point
                    }
                }
            }
        }
    }
}
