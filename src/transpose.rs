use crate::align::print_transposition;
use stam::*;

pub fn transpose<'store>(
    store: &'store mut AnnotationStore,
    mut transposition_queries: Vec<Query<'store>>,
    queries: Vec<Query<'store>>,
    use_transposition_var: Option<&str>,
    use_var: Option<&str>,
    id_prefix: Option<String>,
    idstrategy: IdStrategy,
    ignore_errors: bool,
    verbose: bool,
    config: TransposeConfig,
) -> Result<Vec<AnnotationHandle>, StamError> {
    let mut builders = Vec::new();
    while transposition_queries.len() < queries.len() {
        let query = transposition_queries
            .get(transposition_queries.len() - 1)
            .expect("there must be transposition queries");
        transposition_queries.push(query.clone());
    }
    for (transposition_query, query) in transposition_queries.into_iter().zip(queries.into_iter()) {
        let iter = store.query(transposition_query)?;
        let mut transposition: Option<ResultItem<Annotation>> = None;
        let transpositiondata = store
            .find_data(
                "https://w3id.org/stam/extensions/stam-transpose/",
                "Transposition",
                DataOperator::Null,
            )
            .next()
            .ok_or_else(|| {
                StamError::OtherError(
                    "No transpositions at all were found in the annotation store (the STAM Transpose vocabulary is not present in the store)",
                )
            })?;
        for resultrow in iter {
            if let Ok(QueryResultItem::Annotation(annotation)) =
                resultrow.get_by_name_or_last(use_transposition_var)
            {
                if !annotation.has_data(&transpositiondata) {
                    return Err(StamError::OtherError(
                        "The retrieved annotation is not explicitly marked as a Transposition, refusing to use",
                    ));
                }
                transposition = Some(annotation.clone());
                break;
            }
        }
        if let Some(transposition) = transposition {
            let iter = store.query(query)?;
            for resultrow in iter {
                if let Ok(QueryResultItem::Annotation(annotation)) =
                    resultrow.get_by_name_or_last(use_var)
                {
                    let mut config = config.clone();
                    if let Some(id) = annotation.id() {
                        let randomid = generate_id("", "");
                        config.transposition_id = if let Some(id_prefix) = &id_prefix {
                            Some(format!("{}{}-transposition-{}", id_prefix, id, randomid))
                        } else {
                            Some(format!("{}-transposition-{}", id, randomid))
                        };
                        config.resegmentation_id = if let Some(id_prefix) = &id_prefix {
                            Some(format!("{}{}-resegmentation-{}", id_prefix, id, randomid))
                        } else {
                            Some(format!("{}-resegmentation-{}", id, randomid))
                        };
                        config.source_side_id = Some(id.to_string());
                        config.existing_source_side = true;
                        config.target_side_ids = vec![if let Some(id_prefix) = &id_prefix {
                            format!("{}{}", id_prefix, regenerate_id(id, &idstrategy))
                        } else {
                            regenerate_id(id, &idstrategy)
                        }];
                    } else {
                        config.existing_source_side = false;
                    }
                    match annotation.transpose(&transposition, config) {
                        Ok(results) => builders.extend(results),
                        Err(StamError::NoText(_)) => {
                            eprintln!(
                                "WARNING: Skipping transposition of annotation that references no text: {}",
                                annotation.id().unwrap_or("(no id)"),
                            );
                        }
                        Err(err) => {
                            eprintln!(
                                "WARNING: Failed to transpose annotation {}: {}",
                                annotation.id().unwrap_or("(no id)"),
                                err
                            );
                            if !ignore_errors {
                                return Err(StamError::OtherError(
                                    "Failed to transpose annotation",
                                ));
                            }
                        }
                    }
                } else {
                    return Err(StamError::OtherError(
                            "Query should return instances of ANNOTATION to transpose, got something else instead",
                        ));
                }
            }
        } else {
            return Err(StamError::OtherError(
                "First query should return an ANNOTATION that is a transposition, none found",
            ));
        }
    }
    let mut annotations = Vec::with_capacity(builders.len());
    for builder in builders {
        let annotation_handle = store.annotate(builder)?;
        annotations.push(annotation_handle);
        if verbose {
            let annotation = store
                .annotation(annotation_handle)
                .expect("annotation was just added");
            let transpositiondata = store
                .find_data(
                    "https://w3id.org/stam/extensions/stam-transpose/",
                    "Transposition",
                    DataOperator::Null,
                )
                .next()
                .ok_or_else(|| {
                    StamError::OtherError(
                        "No transpositions at all were found in the annotation store (the STAM Transpose vocabulary is not present in the store)",
                    )
                })?;
            if annotation.has_data(&transpositiondata) {
                print_transposition(&annotation);
            } else {
                eprintln!(
                    "# added annotation {}",
                    annotation.id().expect("annotation must have ID")
                );
            }
        }
    }
    eprintln!("{} annotations(s) created", annotations.len());
    Ok(annotations)
}
