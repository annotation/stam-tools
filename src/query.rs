use clap::{Arg, ArgAction};
use stam::*;

pub fn query_arguments<'a>(help: &'static str) -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("query")
            .long("query")
            .short('q')
            .help(help)
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("use")
            .long("use")
            .help(
                "Name of the variable from --query to use for the main output. If not set, the last defined subquery will be used (still pertaining to the first --query statement!)"
            )
            .takes_value(true)
    );
    args
}

pub fn textselection_from_queryresult<'a>(
    resultitems: &QueryResultItems<'a>,
    var: Option<&str>,
    names: &QueryNames,
) -> Result<(ResultTextSelection<'a>, bool, Option<&'a str>), &'a str> {
    //convert query result to text selection
    let resultitem = if let Some(var) = var {
        resultitems.get_by_name(names, var).ok()
    } else {
        resultitems.iter().last()
    };
    let (resulttextselection, whole_resource, id) = match resultitem {
        Some(QueryResultItem::TextSelection(textselection)) => (textselection.clone(), false, None),
        Some(QueryResultItem::TextResource(resource)) => (
            resource
                .textselection(&Offset::whole())
                .expect("textselection must succeed"),
            true,
            resource.id(),
        ),
        Some(QueryResultItem::Annotation(annotation)) => {
            let mut iter = annotation.textselections();
            if let Some(textselection) = iter.next() {
                if iter.next().is_some() {
                    return Err("Resulting annotation does not reference any text");
                }
                (textselection, false, annotation.id())
            } else {
                return Err("Resulting annotation does not reference any text");
            }
        }
        Some(QueryResultItem::AnnotationData(_)) => {
            return Err("Query produced result of type DATA, but this does not reference any text");
        }
        Some(QueryResultItem::DataKey(_)) => {
            return Err("Query produced result of type KEY, but this does not reference any text");
        }
        Some(QueryResultItem::AnnotationDataSet(_)) => {
            return Err("Query produced result of type SET, but this does not reference any text");
        }
        None | Some(QueryResultItem::None) => {
            return Err("Query produced no results");
        }
    };
    Ok((resulttextselection, whole_resource, id))
}

pub fn to_json<'a>(store: &'a AnnotationStore, query: Query<'a>) -> Result<(), StamError> {
    let iter = store.query(query);
    let names = iter.names();
    let names_ordered = names.enumerate();
    print!("[");
    for (i, resultrow) in iter.enumerate() {
        if i > 0 {
            println!(",\n{{\n");
        } else {
            println!("{{\n");
        }
        for (j, result) in resultrow.iter().enumerate() {
            let varname = names_ordered.get(j).map(|x| x.1);
            let json = match result {
                QueryResultItem::None => "null".to_string(),
                QueryResultItem::Annotation(annotation) => {
                    annotation.as_ref().to_json_string(store)?
                }
                QueryResultItem::AnnotationData(data) => {
                    data.as_ref().to_json(data.set().as_ref())?
                }
                QueryResultItem::DataKey(key) => key.as_ref().to_json()?,
                QueryResultItem::TextResource(resource) => resource.as_ref().to_json_string()?,
                QueryResultItem::AnnotationDataSet(dataset) => dataset.as_ref().to_json_string(
                    &Config::default().with_dataformat(DataFormat::Json { compact: false }),
                )?,
                QueryResultItem::TextSelection(tsel) => tsel.to_json()?,
            };
            let varnum = format!("{}", j + 1);
            println!(
                "\"{}\": {}{}",
                if let Some(varname) = varname {
                    varname
                } else {
                    &varnum
                },
                json,
                if i < resultrow.len() - 1 { ",\n" } else { "\n" }
            );
        }
        print!("}}");
    }
    println!("]");
    Ok(())
}

pub fn to_w3anno<'a>(
    store: &'a AnnotationStore,
    query: Query<'a>,
    use_var: &str,
    config: WebAnnoConfig,
) {
    let iter = store.query(query);
    let names = iter.names();
    for resultrow in iter {
        if let Ok(result) = resultrow.get_by_name(&names, use_var) {
            match result {
                QueryResultItem::None => {}
                QueryResultItem::Annotation(annotation) => {
                    println!("{}", annotation.to_webannotation(&config));
                }
                QueryResultItem::TextSelection(tsel) => {
                    for annotation in tsel.annotations() {
                        println!("{}", annotation.to_webannotation(&config));
                    }
                }
                _ => {
                    eprintln!("Error: Obtained result type can not be serialised to Web Annotation, only ANNOTATION and TEXT work.");
                }
            }
        } else {
            eprintln!("Error: No result found for variable {}", use_var);
            return;
        }
    }
}
