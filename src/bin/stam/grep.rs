use stam::{AnnotationStore, Regex, RegexSet, Text};
use std::process::exit;

pub fn grep<'a>(store: &AnnotationStore, expressions: Vec<&'a str>, allow_overlap: bool) {
    let expressions: Vec<_> = expressions
        .into_iter()
        .map(|exp| {
            Regex::new(exp).unwrap_or_else(|e| {
                eprintln!("Error in expression {}: {}", exp, e);
                exit(1)
            })
        })
        .collect();
    let precompiledset =
        RegexSet::new(expressions.iter().map(|x| x.as_str())).unwrap_or_else(|e| {
            eprintln!("Error in compiling regexset: {}", e);
            exit(1);
        });
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
}
