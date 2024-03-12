use stam::{AnnotationStore, Regex, RegexSet, Text};

pub fn grep<'a>(
    store: &AnnotationStore,
    expressions: Vec<&'a str>,
    allow_overlap: bool,
) -> Result<(), String> {
    let mut error: bool = false;
    let error = &mut error;
    let expressions: Vec<_> = expressions
        .into_iter()
        .filter_map(|exp| {
            Regex::new(exp)
                .map_err(|e| {
                    eprintln!("[warning] Error in expression {}: {}", exp, e);
                    *error = true;
                })
                .ok()
        })
        .collect();
    if *error || expressions.is_empty() {
        return Err(format!("There were errors in the regular expressions"));
    }
    let precompiledset = RegexSet::new(expressions.iter().map(|x| x.as_str()))
        .map_err(|e| format!("[warning] Error in compiling regexset: {}", e))?;
    //search the text and build annotations
    for textmatch in store.find_text_regex(&expressions, &Some(precompiledset), allow_overlap) {
        for (i, textselection) in textmatch.textselections().iter().enumerate() {
            println!(
                "{}\t{}:{}\t{}\t{}/{}",
                textselection.resource().id().unwrap_or("(no id)"),
                textselection.begin(),
                textselection.end(),
                textselection.text(),
                i + 1,
                textmatch.textselections().len(),
            );
        }
    }
    Ok(())
}
