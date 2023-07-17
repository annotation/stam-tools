use stam::{AnnotationStore, AssociatedFile, Configurable, Handle, Storable, Text};

pub fn info(store: &AnnotationStore, verbose: bool) {
    if !verbose {
        eprintln!("(Tip: add --verbose for more detailed info output)");
    }
    if let Some(id) = store.id() {
        println!("ID: {}", id);
    }
    println!("Configuration: {:?}", store.config());
    println!("Filename: {:?}", store.filename().unwrap_or("(none)"));
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
            "    - [{}] Resource ID: {:?}; textlength: {}, #positions: {}",
            resource.handle().unwrap(),
            resource.id().unwrap_or("(none)"),
            resource.textlen(),
            resource.positionindex_len(),
        );
        if verbose {
            for textselection in resource.textselections() {
                println!(
                    "        - [{}] TextSelection; begin: {}; end: {}, text: {:?}, #annotations: {}",
                    textselection.handle().unwrap(),
                    textselection.begin(),
                    textselection.end(),
                    //text:
                    {
                        let text = textselection.text();
                        if text.len() > 1024 {
                            "(too long)"
                        } else {
                            text
                        }
                    },
                    //nrannotations:
                    textselection.annotations_len(store)
                );
            }
        }
    }
    println!("Annotation datasets:    {}", store.annotationsets_len());
    for annotationset in store.annotationsets() {
        println!(
            "    - [{}] Set ID: {:?}; #keys: {}; #data: {}",
            annotationset.handle().unwrap(),
            annotationset.id().unwrap_or("(none)"),
            annotationset.as_ref().keys_len(),
            annotationset.as_ref().data_len(),
        );
        if verbose {
            for key in annotationset.as_ref().keys() {
                println!(
                    "        - [{}] Key ID: {:?}; #data: {}",
                    key.handle().unwrap(),
                    key.id().unwrap_or("(none)"),
                    annotationset
                        .as_ref()
                        .data_by_key(key)
                        .unwrap_or(&vec!())
                        .len()
                );
            }
            for data in annotationset.as_ref().data() {
                let annotations = store.annotations_by_data(annotationset.handle(), data.handle());
                println!(
                    "        - [{}] Data ID: {:?}; Key: {:?}; Value: {:?}; #annotations: {}",
                    data.handle().unwrap(),
                    data.id().unwrap_or("(none)"),
                    data.key().id().unwrap_or("(none)"),
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
                annotation.handle().unwrap(),
                annotation.id().unwrap_or("(none)"),
                annotation.as_ref().target(),
                //text:
                {
                    let text: Vec<&str> = annotation.text().collect();
                    text
                },
                annotation.as_ref().len(),
            );
            for data in annotation.data() {
                println!(
                    "        - [{}] Data ID: {:?}; Set ID: {:?}; Key: {:?}; Value: {:?}",
                    data.handle().unwrap(),
                    data.id().unwrap_or("(none)"),
                    data.set().id().unwrap_or("(none)"),
                    data.key().id().unwrap_or("(none)"),
                    data.value(),
                );
            }
        }
    }
}
