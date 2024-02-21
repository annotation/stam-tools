use clap::{Arg, ArgAction};
use stam::*;

use seal::pair::{AlignmentSet, InMemoryAlignmentMatrix, NeedlemanWunsch, SmithWaterman, Step};

pub fn align_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("use2")
            .long("use2")
            .help(
                "Name of the variable from the *second* --query to use. If not set, the last defined subquery will be used (still pertaining to the second --query statement!)"
            )
            .takes_value(true)
    );
    args.push(
        Arg::with_name("resource")
            .long("resource")
            .short('r')
            .help(
                "The ID of the resource to align; specify this argument twice. It is an alternative to specifying two full --query parameters"
            )
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("ignore-case")
            .long("ignore-case")
            .help("Do case-insensitive matching, this has more performance overhead"),
    );
    args.push(
        Arg::with_name("global")
            .long("global")
            .help("Perform global alignment instead of local"),
    );
    args.push(
        Arg::with_name("algorithm")
            .long("algorithm")
            .takes_value(true)
            .default_value("smith_waterman")
            .help("Alignment algorithm, can be smith_waterman (default) or needleman_wunsch"),
    );
    args.push(
        Arg::with_name("id-prefix")
            .long("id-prefix")
            .takes_value(true)
            .help("Prefix to use when assigning annotation IDs. The actual ID will have a random component."),
    );
    args.push(
        Arg::with_name("match-score")
            .long("match-score")
            .takes_value(true)
            .default_value("2")
            .help("Score for matching alignments, positive integer"),
    );
    args.push(
        Arg::with_name("mismatch-score")
            .long("mismatch-score")
            .takes_value(true)
            .default_value("-1")
            .help("Score for mismatching alignments, negative integer"),
    );
    args.push(
        Arg::with_name("insertion-score")
            .long("insertion-score")
            .takes_value(true)
            .default_value("-1")
            .help("Score for insertions (gap penalty), negative integer"),
    );
    args.push(
        Arg::with_name("deletion-score")
            .long("deletion-score")
            .takes_value(true)
            .default_value("-1")
            .help("Score for deletions (gap penalty), negative integer"),
    );
    args
}

pub struct AlignmentConfig {
    pub case_sensitive: bool,
    pub algorithm: AlignmentAlgorithm,
    pub alignment_scope: AlignmentScope,
    pub annotation_id_prefix: Option<String>,
    pub verbose: bool,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self {
            case_sensitive: true,
            alignment_scope: AlignmentScope::Local,
            algorithm: AlignmentAlgorithm::default(),
            annotation_id_prefix: None,
            verbose: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AlignmentAlgorithm {
    NeedlemanWunsch {
        equal: isize,
        align: isize,
        insert: isize,
        delete: isize,
    },
    SmithWaterman {
        equal: isize,
        align: isize,
        insert: isize,
        delete: isize,
    },
}

impl Default for AlignmentAlgorithm {
    fn default() -> Self {
        Self::SmithWaterman {
            equal: 2,
            align: -1,
            insert: -1,
            delete: -1,
        }
    }
}

/// Aligns the texts of two queries
/// and adds transposition annotations for each possible combination of the two
/// Returns the transpositions added
pub fn align<'store>(
    store: &'store mut AnnotationStore,
    query: Query<'store>,
    query2: Query<'store>,
    use_var: Option<&str>,
    use_var2: Option<&str>,
    config: &AlignmentConfig,
) -> Result<Vec<AnnotationHandle>, StamError> {
    let mut buildtranspositions = Vec::new();
    {
        let iter = store.query(query);
        let names = iter.names();
        for resultrow in iter {
            if let Ok(result) = resultrow.get_by_name_or_last(&names, use_var) {
                let (text, query2) = match result {
                    QueryResultItem::TextResource(resource) => ( resource.clone().to_textselection(), query2.clone().with_resourcevar(use_var.unwrap_or("resource"), resource.clone())),
                    QueryResultItem::Annotation(annotation) => {
                        if let Some(tsel) = annotation.textselections().next() {
                            (tsel, query2.clone().with_annotationvar(use_var.unwrap_or("annotation"), annotation.clone()))
                        } else {
                            return Err(StamError::OtherError("Annotation references multiple texts, this is not supported yet by stam align"));
                        }
                    }
                    QueryResultItem::TextSelection(tsel) => ( tsel.clone(), query2.clone().with_textvar(use_var.unwrap_or("text"), tsel.clone())),
                    _ => return Err(StamError::OtherError("Obtained result type can not by used by stam align, expected ANNOTATION, RESOURCE or TEXT"))
                };

                let iter2 = store.query(query2);
                let names2 = iter2.names();
                for resultrow2 in iter2 {
                    if let Ok(result) = resultrow2.get_by_name_or_last(&names2, use_var2) {
                        let text2 = match result {
                            QueryResultItem::TextResource(resource) => resource.clone().to_textselection(),
                            QueryResultItem::Annotation(annotation) => {
                                if let Some(tsel) = annotation.textselections().next() {
                                    tsel
                                } else {
                                    return Err(StamError::OtherError("Annotation references multiple texts, this is not supported yet by stam align"));
                                }
                            }
                            QueryResultItem::TextSelection(tsel) => tsel.clone(),
                            _ => return Err(StamError::OtherError("Obtained result type can not by used by stam align, expected ANNOTATION, RESOURCE or TEXT"))
                        };

                        buildtranspositions.extend(align_texts(&text, &text2, config)?);
                    } else if let Some(use_var2) = use_var2 {
                        return Err(StamError::QuerySyntaxError(
                            format!(
                                "No result found for variable {}, so nothing to align",
                                use_var2
                            ),
                            "(align)",
                        ));
                    }
                }
            } else if let Some(use_var) = use_var {
                return Err(StamError::QuerySyntaxError(
                    format!(
                        "No result found for variable {}, so nothing to align",
                        use_var
                    ),
                    "(align)",
                ));
            }
        }
    }
    let mut transpositions = Vec::with_capacity(buildtranspositions.len());
    for builder in buildtranspositions {
        transpositions.push(store.annotate(builder)?);
    }
    eprintln!("{} annotations(s) created", transpositions.len());
    Ok(transpositions)
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum AlignmentScope {
    Local,
    Global,
}

struct AlignedFragment {
    begin1: usize,
    begin2: usize,
    length: usize,
}

impl AlignedFragment {
    fn to_offsets<'a>(&self) -> (Offset, Offset) {
        (
            Offset::simple(self.begin1, self.begin1 + self.length),
            Offset::simple(self.begin2, self.begin2 + self.length),
        )
    }

    fn publish<'store>(
        &self,
        select1: &mut Vec<SelectorBuilder<'static>>,
        select2: &mut Vec<SelectorBuilder<'static>>,
        text: &ResultTextSelection<'store>,
        text2: &ResultTextSelection<'store>,
        config: &AlignmentConfig,
    ) -> Result<(), StamError> {
        let (offset1, offset2) = self.to_offsets();
        if config.verbose {
            println!(
                "{}\t{}-{}\t{}\t{}-{}\t\"{}\"\t\"{}\"",
                text.resource().id().unwrap_or("-"),
                &offset1.begin,
                &offset1.end,
                text2.resource().id().unwrap_or("-"),
                &offset2.begin,
                &offset2.end,
                text.textselection(&offset1)?
                    .text()
                    .replace("\"", "\\\"")
                    .replace("\t", "\\t")
                    .replace("\n", "\\n"),
                text2
                    .textselection(&offset2)?
                    .text()
                    .replace("\"", "\\\"")
                    .replace("\t", "\\t")
                    .replace("\n", "\\n")
            );
        }
        select1.push(SelectorBuilder::TextSelector(
            text.resource().handle().into(),
            offset1,
        ));
        select2.push(SelectorBuilder::TextSelector(
            text2.resource().handle().into(),
            offset2,
        ));
        Ok(())
    }
}

/// Find an alignment between two texts and creates a transposition
/// Returns builders for the transposition, you still have to add it to the store.
pub fn align_texts<'store>(
    text: &ResultTextSelection<'store>,
    text2: &ResultTextSelection<'store>,
    config: &AlignmentConfig,
) -> Result<Vec<AnnotationBuilder<'static>>, StamError> {
    let mut builders = Vec::with_capacity(3);
    let seq1: Vec<char> = text.text().chars().collect();
    let seq2: Vec<char> = text2.text().chars().collect();

    let alignment_set: Result<AlignmentSet<InMemoryAlignmentMatrix>, _> = match config.algorithm {
        AlignmentAlgorithm::SmithWaterman {
            equal,
            align,
            insert,
            delete,
        } => {
            let algorithm = SmithWaterman::new(equal, align, insert, delete);
            AlignmentSet::new(seq1.len(), seq2.len(), algorithm, |x, y| {
                if config.case_sensitive {
                    seq1[x] == seq2[y]
                } else {
                    seq1[x].to_lowercase().to_string() == seq2[y].to_lowercase().to_string()
                }
            })
        }
        AlignmentAlgorithm::NeedlemanWunsch {
            equal,
            align,
            insert,
            delete,
        } => {
            let algorithm = NeedlemanWunsch::new(equal, align, insert, delete);
            AlignmentSet::new(seq1.len(), seq2.len(), algorithm, |x, y| {
                if config.case_sensitive {
                    seq1[x] == seq2[y]
                } else {
                    seq1[x].to_lowercase().to_string() == seq2[y].to_lowercase().to_string()
                }
            })
        }
    };

    match alignment_set {
        Ok(alignment_set) => {
            let alignment = match config.alignment_scope {
                AlignmentScope::Local => alignment_set.local_alignment(),
                AlignmentScope::Global => alignment_set.global_alignment(),
            };
            let mut select1: Vec<SelectorBuilder<'static>> = Vec::new();
            let mut select2: Vec<SelectorBuilder<'static>> = Vec::new();

            let mut fragment: Option<AlignedFragment> = None;
            for step in alignment.steps() {
                match step {
                    Step::Align { x, y } => {
                        if let Some(fragment) = fragment.as_mut() {
                            fragment.length += 1;
                        } else {
                            fragment = Some(AlignedFragment {
                                begin1: x,
                                begin2: y,
                                length: 1,
                            });
                        }
                    }
                    _ => {
                        if let Some(fragment) = fragment.take() {
                            fragment.publish(&mut select1, &mut select2, text, text2, config)?
                        }
                    }
                }
            }
            if let Some(fragment) = fragment.take() {
                fragment.publish(&mut select1, &mut select2, text, text2, config)?
            }

            if select1.is_empty() {
                //no alignment found
                //TODO: compute a score?
                return Ok(builders);
            }

            let id = if let Some(prefix) = config.annotation_id_prefix.as_ref() {
                generate_id(&format!("{}transposition-", prefix), "")
            } else {
                generate_id("transposition-", "")
            };
            if select1.len() == 1 {
                //simple transposition
                builders.push(
                    AnnotationBuilder::new()
                        .with_id(id.clone())
                        .with_data(
                            "https://w3id.org/stam/extensions/stam-transpose/",
                            "Transposition",
                            DataValue::Null,
                        )
                        .with_target(SelectorBuilder::DirectionalSelector(vec![
                            select1.into_iter().next().unwrap(),
                            select2.into_iter().next().unwrap(),
                        ])),
                );
            } else {
                //complex transposition
                let annotation1id = format!("{}-side1", id);
                builders.push(
                    AnnotationBuilder::new()
                        .with_id(annotation1id.clone())
                        .with_data(
                            "https://w3id.org/stam/extensions/stam-transpose/",
                            "TranspositionSide",
                            DataValue::Null,
                        )
                        .with_target(SelectorBuilder::DirectionalSelector(select1)),
                );
                let annotation2id = format!("{}-side2", id);
                builders.push(
                    AnnotationBuilder::new()
                        .with_id(annotation2id.clone())
                        .with_data(
                            "https://w3id.org/stam/extensions/stam-transpose/",
                            "TranspositionSide",
                            DataValue::Null,
                        )
                        .with_target(SelectorBuilder::DirectionalSelector(select2)),
                );
                builders.push(
                    AnnotationBuilder::new()
                        .with_id(id.clone())
                        .with_data(
                            "https://w3id.org/stam/extensions/stam-transpose/",
                            "Transposition",
                            DataValue::Null,
                        )
                        .with_target(SelectorBuilder::DirectionalSelector(vec![
                            SelectorBuilder::AnnotationSelector(annotation1id.into(), None),
                            SelectorBuilder::AnnotationSelector(annotation2id.into(), None),
                        ])),
                );
            }
            Ok(builders)
        }
        Err(error) => {
            eprintln!("ALIGNMENT ERROR: {:?}", error);
            return Err(StamError::OtherError(
                "Failed to generated alignment set due to error",
            ));
        }
    }
}
