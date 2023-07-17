use stam::{AnnotationStore, AssociatedFile, Configurable, Handle, Storable, Text};

const BYTES: &str = "bytes";
const GIB: &str = "GiB";
const MIB: &str = "MiB";
const KIB: &str = "KiB";

pub fn humanmem(bytes: usize) -> (f64, &'static str) {
    if bytes >= 1024 * 1024 * 1024 {
        let bytes: f64 = bytes as f64;
        let gb = bytes / 1024.0 / 1024.0 / 1024.0;
        (gb, GIB)
    } else if bytes >= 1024 * 1024 {
        let bytes: f64 = bytes as f64;
        let mb = bytes / 1024.0 / 1024.0;
        (mb, MIB)
    } else if bytes >= 1024 {
        let bytes: f64 = bytes as f64;
        let kb = bytes / 1024.0;
        (kb, KIB)
    } else {
        let bytes: f64 = bytes as f64;
        (bytes, BYTES)
    }
}

pub fn info(store: &AnnotationStore, verbose: bool) {
    if !verbose {
        eprintln!("(Tip: add --verbose for more detailed info output)");
    }
    if let Some(id) = store.id() {
        println!("ID: {}", id);
    }
    println!("Configuration: {:?}", store.config());
    println!("Filename: {:?}", store.filename().unwrap_or("(none)"));
    let len = store.index_len();
    let partial = store.index_partialcount();
    let total = store.index_totalcount();
    let bytes = store.index_meminfo();
    println!("Indices:");
    let mem = humanmem(bytes.0);
    println!(
        "    - dataset_data_annotation_map:      {} -> {} -> {} (> {:.2} {})",
        len.0, partial.0, total.0, mem.0, mem.1
    );
    let mem = humanmem(bytes.1);
    println!(
        "    - textrelationmap:                  {} -> {} -> {} (> {:.2} {})",
        len.1, partial.1, total.1, mem.0, mem.1
    );
    let mem = humanmem(bytes.2);
    println!(
        "    - resource_annotation_map:          {} -> {} (> {:.2} {})",
        len.2, total.2, mem.0, mem.1
    );
    let mem = humanmem(bytes.3);
    println!(
        "    - dataset_annotation_map:           {} -> {} (> {:.2} {})",
        len.3, total.3, mem.0, mem.1
    );
    let mem = humanmem(bytes.4);
    println!(
        "    - annotation_annotation_map:        {} -> {} (> {:.2} {})",
        len.4, total.4, mem.0, mem.1
    );
    let mem = humanmem(bytes.5);
    println!(
        "    - resource_idmap:        {} (> {:.2} {})",
        len.5, mem.0, mem.1
    );
    let mem = humanmem(bytes.6);
    println!(
        "    - dataset_idmap:        {} (> {:.2} {})",
        len.6, mem.0, mem.1
    );
    let mem = humanmem(bytes.7);
    println!(
        "    - annotation_idmap:        {} (> {:.2} {})",
        len.7, mem.0, mem.1
    );
    println!("Resources:              {}", store.resources_len());
    for resource in store.resources() {
        let textsize = humanmem(resource.text().len());
        let mem = humanmem(resource.as_ref().meminfo());
        println!(
            "    - [{}] Resource ID: {:?}; textlength: {}, textsize: {:.2} {}, #positions: {}, #textselections: {}, memory estimate: {:.2} {}",
            resource.handle().unwrap(),
            resource.id().unwrap_or("(none)"),
            resource.textlen(),
            textsize.0,
            textsize.1,
            resource.positionindex_len(),
            resource.textselections_len(),
            mem.0,
            mem.1,
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
        let mem = humanmem(annotationset.as_ref().meminfo());
        println!(
            "    - [{}] Set ID: {:?}; #keys: {}; #data: {}, memory estimate: {:.2} {}",
            annotationset.handle().unwrap(),
            annotationset.id().unwrap_or("(none)"),
            annotationset.as_ref().keys_len(),
            annotationset.as_ref().data_len(),
            mem.0,
            mem.1,
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
    let mem = humanmem(store.annotations_meminfo());
    println!(
        "Annotations:            {} ({:.2} {})",
        store.annotations_len(),
        mem.0,
        mem.1
    );
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
