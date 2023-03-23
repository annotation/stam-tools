use stam::{AnnotationStore, AnyId, Storable, TextResourceHandle, TextSelection};

pub fn to_tsv(store: &AnnotationStore, verbose: bool) {
    for annotation in store.annotations() {
        let id = annotation.id().unwrap_or("(none)");
        for (key, data, dataset) in store.data_by_annotation(annotation) {
            // get the text to which this annotation refers (if any)
            let text: Vec<&str> = store.text_by_annotation(annotation).collect();
            if verbose {
                let textselections: Vec<(TextResourceHandle, TextSelection)> =
                    store.textselections_by_annotation(annotation).collect();
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    id,
                    dataset.id().unwrap(),
                    key.id().unwrap(),
                    data.value(),
                    text.join("|").replace("\n", " "),
                    textselections
                        .iter()
                        .map(|(reshandle, t)| {
                            let resource = store
                                .resource(&AnyId::Handle(*reshandle))
                                .expect("resource must exist");
                            format!("{}#{}-{}", resource.id().unwrap_or(""), t.begin(), t.end())
                        })
                        .collect::<Vec<String>>()
                        .join("|")
                );
            } else {
                println!(
                    "{}\t{}\t{}\t{}",
                    id,
                    key.id().unwrap(),
                    data.value(),
                    text.join("|").replace("\n", " ")
                );
            }
        }
    }
}
