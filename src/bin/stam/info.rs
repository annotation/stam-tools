use std::process::exit;

use stam::{AnnotationStore, AnyId, Configurable, Handle, Storable};

pub fn info(store: &AnnotationStore, verbose: bool) {
    if !verbose {
        eprintln!("(Tip: add --verbose for more detailed info output)");
    }
    if let Some(id) = store.id() {
        println!("ID: {}", id);
    }
    println!("Configuration: {:?}", store.config());
    println!(
        "Filename: {:?}",
        store
            .filename()
            .map(|x| x.to_str().unwrap())
            .unwrap_or("(none)")
    );
    let count = store.index_totalcount();
    println!("Indices:");
    println!("    - dataset_data_annotation_map:      {}", count.0);
    println!("    - textrelationmap:                  {}", count.1);
    println!("    - resource_annotation_map:          {}", count.2);
    println!("    - dataset_annotation_map:           {}", count.3);
    println!("    - annotation_annotation_map:        {}", count.4);
    println!("Resources:              {}", store.resources_len());
    for resource in store.resources() {
        println!(
            "    - [{}] Resource ID: {:?}; textlength: {}",
            resource.handle().unwrap().unwrap(),
            resource.id().unwrap_or("(none)"),
            resource.textlen()
        );
        if verbose {
            for textselection in resource.textselections() {
                let annotations = store.annotations_by_textselection_handle(
                    resource.handle().unwrap(),
                    textselection.handle().unwrap(),
                );
                println!(
                    "        - [{}] TextSelection; begin: {}; end: {}, text: {:?}, #annotations: {}",
                    textselection.handle().unwrap().unwrap(),
                    textselection.begin(),
                    textselection.end(),
                    //text:
                    {
                        if let Some(resource) = store.resource(&AnyId::Handle(resource.handle().unwrap())) {
                            let text = resource.text_by_textselection(textselection).unwrap_or_else(|err| {
                                eprintln!("Failed get text: {}", err);
                                exit(1);
                            });
                            if text.len() > 1024 {
                                "(too long)"
                            } else {
                                text
                            }
                        } else {
                            "(none)"
                        }
                    },
                    //nrannotations:
                    if let Some(annotations) = annotations {
                        annotations.len()
                    } else {
                        0
                    }
                );
            }
        }
    }
    println!("Annotation datasets:    {}", store.annotationsets_len());
    for annotationset in store.annotationsets() {
        println!(
            "    - [{}] Set ID: {:?}; #keys: {}; #data: {}",
            annotationset.handle().unwrap().unwrap(),
            annotationset.id().unwrap_or("(none)"),
            annotationset.keys_len(),
            annotationset.data_len(),
        );
        if verbose {
            for key in annotationset.keys() {
                println!(
                    "        - [{}] Key ID: {:?}; #data: {}",
                    key.handle().unwrap().unwrap(),
                    key.id().unwrap_or("(none)"),
                    annotationset
                        .data_by_key(key.handle().unwrap())
                        .unwrap_or(&vec!())
                        .len()
                );
            }
            for data in annotationset.data() {
                let key = annotationset
                    .key(&AnyId::from(data.key()))
                    .expect("Key not found");
                let annotations = store
                    .annotations_by_data(annotationset.handle().unwrap(), data.handle().unwrap());
                println!(
                    "        - [{}] Data ID: {:?}; Key: {:?}; Value: {:?}; #annotations: {}",
                    data.handle().unwrap().unwrap(),
                    data.id().unwrap_or("(none)"),
                    key.id().unwrap_or("(none)"),
                    data.value(),
                    if let Some(annotations) = annotations {
                        annotations.len()
                    } else {
                        0
                    }
                );
            }
        }
    }
    println!("Annotations:            {}", store.annotations_len());
    if verbose {
        for annotation in store.annotations() {
            println!(
                "    - [{}] Annotation ID: {:?}; target: {:?}; text: {:?}, #data: {}",
                annotation.handle().unwrap().unwrap(),
                annotation.id().unwrap_or("(none)"),
                annotation.target(),
                //text:
                {
                    if let Some(annotation) =
                        store.annotation(&AnyId::Handle(annotation.handle().unwrap()))
                    {
                        let text: Vec<&str> = store.text_by_annotation(annotation).collect();
                        text
                    } else {
                        vec!["(no text)"]
                    }
                },
                annotation.len(),
            );
            for (key, data, annotationset) in store.data_by_annotation(annotation) {
                println!(
                    "        - [{}] Data ID: {:?}; Set ID: {:?}; Key: {:?}; Value: {:?}",
                    data.handle().unwrap().unwrap(),
                    data.id().unwrap_or("(none)"),
                    annotationset.id().unwrap_or("(none)"),
                    key.id().unwrap_or("(none)"),
                    data.value(),
                );
            }
        }
    }
}
