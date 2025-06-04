use crate::query::textselection_from_queryresult;
use stam::{AnnotationStore, Query, Text};

/// Run a query and outputs the text of the results to standard output. Some extra information (identifiers as headers) will be outputted to standard error output
pub fn to_text<'a, W: std::io::Write>(
    store: &'a AnnotationStore,
    writer: &mut W,
    query: Query<'a>,
    varname: Option<&'a str>,
) -> Result<(), String> {
    let results = store.query(query).map_err(|e| format!("{}", e))?;
    let mut prevresult = None;
    for selectionresult in results {
        match textselection_from_queryresult(&selectionresult, varname) {
            Err(msg) => {
                return Err(format!("{}", msg));
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
                write!(writer, "{}\n", textselection.text()).map_err(|e| format!("{}", e))?;
            }
        }
    }
    Ok(())
}
