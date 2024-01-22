use crate::query::textselection_from_queryresult;
use stam::{AnnotationStore, Query, Text};
use std::process::exit;

pub fn to_text<'a>(store: &'a AnnotationStore, query: Query<'a>, varname: Option<&'a str>) {
    let results = store.query(query);
    let names = results.names();
    let mut prevresult = None;
    for selectionresult in results {
        match textselection_from_queryresult(&selectionresult, varname, &names) {
            Err(msg) => {
                eprintln!("Error: {}", msg);
                exit(1);
            }
            Ok((textselection, _, id)) => {
                if prevresult == Some(textselection.clone()) {
                    //prevent duplicates (especially relevant when --use is set)
                    continue;
                }
                prevresult = Some(textselection.clone());
                if let Some(id) = id {
                    eprintln!(
                        "--------------------------- {} ---------------------------",
                        id
                    );
                } else {
                    eprintln!(
                        "--------------------------- {}#{}-{} ---------------------------",
                        textselection.resource().id().unwrap_or("undefined"),
                        textselection.begin(),
                        textselection.end(),
                    );
                }
                println!("{}", textselection.text());
            }
        }
    }
}
