use stam::*;
use std::collections::BTreeSet;
use std::process::exit;

pub fn to_html(
    store: &AnnotationStore,
    resource_ids: Vec<&str>,
    highlights: Vec<BTreeSet<AnnotationHandle>>,
    with_data: bool,
) {
    for resource_id in resource_ids {
        let mut all_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
        if let Some(resource) = store.resource(resource_id) {
            print!("<div data-resource=\"{}\">\n", resource_id);
            let mut span_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
            let mut begin: usize = 0;
            for i in resource.as_ref().positions(stam::PositionMode::Both) {
                if *i > begin {
                    let text = resource
                        .text_by_offset(&Offset::simple(begin, *i))
                        .expect("offset should be valid");
                    print!(
                        "{}",
                        html_escape::encode_text(text.replace("\n", "<br/>").as_str())
                    );
                    begin = *i;
                }
                if !span_annotations.is_empty() {
                    print!("</span>");
                }
                if let Some(position) = resource.as_ref().position(*i) {
                    for (_, textselection) in position.iter_end2begin() {
                        let textselection = resource
                            .as_ref()
                            .get(*textselection)
                            .unwrap()
                            .as_resultitem(resource.as_ref(), store);
                        let close: Vec<_> =
                            textselection.annotations().map(|a| a.handle()).collect();
                        span_annotations.retain(|a| !close.contains(a));
                    }
                    for (_, textselection) in position.iter_begin2end() {
                        let textselection = resource
                            .as_ref()
                            .get(*textselection)
                            .unwrap()
                            .as_resultitem(resource.as_ref(), store);
                        span_annotations.extend(textselection.annotations().map(|a| a.handle()));
                        if with_data {
                            all_annotations.extend(textselection.annotations().map(|a| a.handle()));
                        }
                    }
                    if !span_annotations.is_empty() {
                        let mut classes = vec!["a".to_string()];
                        for (j, highlights) in highlights.iter().enumerate() {
                            if span_annotations.intersection(highlights).next().is_some() {
                                classes.push(format!("hi{}", j + 1));
                            }
                        }
                        print!(
                            "<span data-annotations=\"{}\" data-pos=\"{}\" class=\"{}\">",
                            span_annotations
                                .iter()
                                .map(|a_handle| {
                                    let annotation = store.get(*a_handle).unwrap();
                                    annotation
                                        .id()
                                        .map(|x| x.to_string())
                                        .unwrap_or_else(|| annotation.temp_id().unwrap())
                                })
                                .collect::<Vec<_>>()
                                .join(" "),
                            i,
                            classes.join(" ")
                        );
                    }
                }
            }
            if resource.textlen() > begin {
                let text = resource
                    .text_by_offset(&Offset::simple(begin, resource.textlen()))
                    .expect("offset should be valid");
                print!(
                    "{}",
                    html_escape::encode_text(text.replace("\n", "<br/>").as_str())
                );
            }
            print!("</div>");
        } else {
            eprintln!("Error: Resource with ID {} does not exist", resource_id);
            exit(1);
        }
        if with_data {
            print!("<script>");
            //TODO: call data_to_json()
            print!("</script>");
        }
    }
}

/*
fn data_to_json(store: &AnnotationStore, annotations: impl Iterator<Item = AnnotationHandle>) -> String {
        print!("annotations = {{");
        for a_handle in all_annotations.iter() {
            let annotation = store.get(*a_handle).unwrap();
            print!("  \"\"
        }
        print!("}}");
}
*/
