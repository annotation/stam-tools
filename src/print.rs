use crate::to_text::to_text;
use stam::{AnnotationStore, Cursor, FindText, Offset, Text};

pub fn print<'a, W: std::io::Write>(
    store: &'a AnnotationStore,
    writer: &mut W,
    resource_id: Option<&str>,
    offset: Offset,
) -> Result<(), String> {
    if let Some(resource_id) = resource_id {
        if let Some(resource) = store.resource(resource_id) {
            let textsel = resource
                .textselection(&offset)
                .map_err(|e| format!("{}", e))?;
            write!(writer, "{}", textsel.text()).map_err(|e| format!("{}", e))?;
        }
    } else if (offset.begin == Cursor::BeginAligned(0)) && (offset.end == Cursor::EndAligned(0)) {
        //no resource and no offset provided, print all resources
        let (query, _) = stam::Query::parse("SELECT RESOURCE ?res")
            .map_err(|err| format!("Query syntax error: {}", err))?;
        to_text(&store, writer, query, None)?;
    } else {
        //no resource provided, but offset is provided: use first resource we can find
        if let Some(resource) = store.resources().next() {
            let textsel = resource
                .textselection(&offset)
                .map_err(|e| format!("{}", e))?;
            write!(writer, "{}", textsel.text()).map_err(|e| format!("{}", e))?;
        }
    }
    Ok(())
}
