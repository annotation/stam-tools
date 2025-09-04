use stam::{
    AnnotationBuilder, AnnotationDataBuilder, AnnotationStore, BuildItem, Offset, Regex, RegexSet,
    SelectorBuilder, Text,
};
use std::fs::File;
use std::io::{BufRead, BufReader};

struct Rule<'a> {
    expression: Regex,
    databuilder: AnnotationDataBuilder<'a>,
    //does the value reference capture groups like $1 $2 $3?
    variable_value: bool,
}

fn load_tag_rules<'a>(filename: &'a str) -> Result<Vec<Rule<'a>>, String> {
    let mut rules: Vec<Rule> = Vec::new();
    let f = File::open(filename).map_err(|e| format!("Error opening rules {}: {}", filename, e))?;
    let reader = BufReader::new(f);
    for (i, line) in reader.lines().enumerate() {
        if let Ok(line) = line {
            if !line.is_empty() && !line.starts_with("#") {
                let fields: Vec<&str> = line.split("\t").collect();
                if fields.len() != 4 {
                    return Err(format!(
                        "Error parsing rules {} line {}: Expected 4 columns, got {}",
                        filename,
                        i + 1,
                        fields.len()
                    ));
                }
                let expression = Regex::new(fields[0])
                    .map_err(|e| format!("Error in rules {} line {}: {}", filename, i + 1, e))?;
                let variable_value = if fields[3].find("$").is_some() {
                    true
                } else {
                    false
                };
                rules.push(Rule {
                    expression,
                    databuilder: AnnotationDataBuilder::new()
                        .with_dataset(BuildItem::Id(fields[1].to_string()))
                        .with_key(BuildItem::Id(fields[2].to_string()))
                        .with_value(fields[3].into()),
                    variable_value,
                });
            }
        }
    }
    Ok(rules)
}

/// Tag according to ths specified rule file. This adds new annotations to the store.
pub fn tag<'a>(
    store: &mut AnnotationStore,
    rulefile: &'a str,
    allow_overlap: bool,
) -> Result<(), String> {
    let rules = load_tag_rules(rulefile)?;
    let expressions: Vec<_> = rules.iter().map(|rule| rule.expression.clone()).collect();
    eprintln!("Loaded {} expressions from {}", rules.len(), rulefile);
    let precompiledset = RegexSet::new(expressions.iter().map(|x| x.as_str()))
        .map_err(|e| format!("Error in compiling regexset: {}", e))?;
    //search the text and build annotations
    let annotations: Vec<AnnotationBuilder<'a>> = store
        .find_text_regex(&expressions, &Some(precompiledset), allow_overlap)
        .map(|textmatch| {
            //get the matching rule
            let rule = rules
                .get(textmatch.expression_index())
                .expect("rule must exist");

            //we must clone the data builder because a rule can apply multiple times and a builder is consumed
            let mut databuilder = rule.databuilder.clone();
            //..also, if there are variables in the value, we resolve them:
            if rule.variable_value {
                let mut value = databuilder.value().to_string();
                for (capnum, textselection) in textmatch
                    .capturegroups()
                    .iter()
                    .zip(textmatch.textselections().iter())
                {
                    let pattern = format!("${}", capnum); //this will fail if there are more than 9 capture groups but that seems excessive to me anyway
                    value = value.replace(pattern.as_str(), textselection.text());
                }
                databuilder = databuilder.with_value(value.into());
            }
            if !textmatch.multi() {
                //build an annotation with a TextSelector
                AnnotationBuilder::new()
                    .with_target(SelectorBuilder::textselector(
                        textmatch.resource().handle(),
                        Offset::from(textmatch.textselections().first().unwrap()),
                    ))
                    .with_data_builder(databuilder)
            } else {
                //result references multiple groups, build an annotation with a CompositeSelector
                AnnotationBuilder::new()
                    .with_target(SelectorBuilder::CompositeSelector(
                        textmatch
                            .textselections()
                            .iter()
                            .map(|textselection| {
                                SelectorBuilder::textselector(
                                    textmatch.resource().handle(),
                                    Offset::from(textselection),
                                )
                            })
                            .collect(),
                    ))
                    .with_data_builder(databuilder)
            }
        })
        .collect();
    //now we add the actual annotations (can't be combined with previous step because we can't have mutability during iteration)
    for annotation in annotations {
        store
            .annotate(annotation)
            .map_err(|err| format!("Failed to add annotation: {}", err))?;
    }
    Ok(())
}
