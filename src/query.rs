use stam::*;

pub(crate) fn textselection_from_queryresult<'a>(
    resultitems: &QueryResultItems<'a>,
    var: Option<&str>,
) -> Result<(ResultTextSelection<'a>, bool, Option<&'a str>), &'a str> {
    //convert query result to text selection
    let resultitem = if let Some(var) = var {
        resultitems.get_by_name(var).ok()
    } else {
        resultitems.iter().next()
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

/// Run a query and outputs the results as STAM JSON to standard output
pub fn to_json<'a, W: std::io::Write>(
    store: &'a AnnotationStore,
    writer: &mut W,
    query: Query<'a>,
) -> Result<(), StamError> {
    let iter = store.query(query)?;
    write!(writer, "[")?;
    for (i, resultrow) in iter.enumerate() {
        if i > 0 {
            writeln!(writer, ",\n{{\n")?;
        } else {
            writeln!(writer, "{{\n")?;
        }
        for (j, (result, varname)) in resultrow.iter().zip(resultrow.names()).enumerate() {
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
            writeln!(
                writer,
                "\"{}\": {}{}",
                if let Some(varname) = varname {
                    varname
                } else {
                    &varnum
                },
                json,
                if i < resultrow.len() - 1 { ",\n" } else { "\n" }
            )?;
        }
        write!(writer, "}}")?;
    }
    writeln!(writer, "]")?;
    Ok(())
}

/// Run a query and outputs the results as W3C Web Annotation to standard output.
/// Each annotation will be formatted in JSON-LD on a single line, so the output is JSONL.
pub fn to_w3anno<'a, W: std::io::Write>(
    store: &'a AnnotationStore,
    writer: &mut W,
    query: Query<'a>,
    use_var: &str,
    config: WebAnnoConfig,
) {
    let iter = store.query(query).expect("query failed");
    for resultrow in iter {
        if let Ok(result) = resultrow.get_by_name(use_var) {
            match result {
                QueryResultItem::None => {}
                QueryResultItem::Annotation(annotation) => {
                    writeln!(writer, "{}", annotation.to_webannotation(&config))
                        .expect("writer failed");
                }
                QueryResultItem::TextSelection(tsel) => {
                    for annotation in tsel.annotations() {
                        writeln!(writer, "{}", annotation.to_webannotation(&config))
                            .expect("writer failed");
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
