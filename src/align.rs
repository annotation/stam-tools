use stam::*;

use seal::pair::{AlignmentSet, InMemoryAlignmentMatrix, NeedlemanWunsch, SmithWaterman, Step};

const TRIM_CHARS: [char; 4] = [' ', '\n', '\t', '\r'];

pub struct AlignmentConfig {
    /// Case-insensitive matching has more performance overhead
    pub case_sensitive: bool,

    // The Alignment algorithm
    pub algorithm: AlignmentAlgorithm,

    /// Prefix to use when assigning annotation IDs. The actual ID will have a random component
    pub annotation_id_prefix: Option<String>,

    /// Strip leading and trailing whitespace/newlines from aligned text selections, keeping them as minimal as possible (default is to be as greedy as possible in selecting)
    /// Setting this may lead to certain whitespaces not being covered even though they may align.
    pub trim: bool,

    /// Only allow for alignments that consist of one contiguous text selection on either side. This is a so-called simple transposition.
    pub simple_only: bool,

    /// Output alignments to standard output in a TSV format
    pub verbose: bool,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self {
            case_sensitive: true,
            algorithm: AlignmentAlgorithm::default(),
            annotation_id_prefix: None,
            trim: false,
            simple_only: false,
            verbose: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AlignmentAlgorithm {
    /// Needleman-Wunsch, global sequence alignment
    NeedlemanWunsch {
        equal: isize,
        align: isize,
        insert: isize,
        delete: isize,
    },
    /// Smith-Waterman, local sequence alignment
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
    queries2: Vec<Query<'store>>,
    use_var: Option<&str>,
    use_var2: Option<&str>,
    config: &AlignmentConfig,
) -> Result<Vec<AnnotationHandle>, StamError> {
    let mut buildtranspositions = Vec::new();
    {
        let iter = store.query(query)?;
        for resultrow in iter {
            if let Ok(result) = resultrow.get_by_name_or_last(use_var) {
                for (i, query2raw) in queries2.iter().enumerate() {
                    //MAYBE TODO: this could be parallellized (but memory may be a problem then)
                    eprintln!("Aligning #{}/{}...", i + 1, queries2.len());
                    let (text, query2) = match result {
                        QueryResultItem::TextResource(resource) => ( resource.clone().to_textselection(), query2raw.clone().with_resourcevar(use_var.unwrap_or("resource"), resource)),
                        QueryResultItem::Annotation(annotation) => {
                            if let Some(tsel) = annotation.textselections().next() {
                                (tsel, query2raw.clone().with_annotationvar(use_var.unwrap_or("annotation"), annotation))
                            } else {
                                return Err(StamError::OtherError("Annotation references multiple texts, this is not supported yet by stam align"));
                            }
                        }
                        QueryResultItem::TextSelection(tsel) => ( tsel.clone(), query2raw.clone().with_textvar(use_var.unwrap_or("text"), tsel)),
                        _ => return Err(StamError::OtherError("Obtained result type can not by used by stam align, expected ANNOTATION, RESOURCE or TEXT"))
                    };

                    let iter2 = store.query(query2)?;
                    for resultrow2 in iter2 {
                        if let Ok(result) = resultrow2.get_by_name_or_last(use_var2) {
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
                            let builders = align_texts(&text, &text2, config)?;
                            buildtranspositions.extend(builders);
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

#[derive(Clone, PartialEq, Debug)]
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
    ) -> Result<bool, StamError> {
        let (offset1, offset2) = self.to_offsets(); //will get shadowed eventually
        let mut textsel1 = text.textselection(&offset1)?;
        let mut textsel2 = text2.textselection(&offset2)?;
        let mut textstring1 = textsel1.text();
        let mut textstring2 = textsel2.text();
        //TODO: This check shouldn't really be necessary but sometimes something goes wrong and this patches it
        if textstring1 != textstring2 {
            if self.length > 1 {
                //ugly patch: try shortening the fragment and rematch (this often works)
                let mut shorterfragment = self.clone();
                shorterfragment.length = self.length - 1;
                return shorterfragment.publish(select1, select2, text, text2, config);
            } else if config.verbose {
                eprintln!(
                    "Notice: Skipping failed alignment fragment: \"{}\" vs \"{}\"",
                    textstring1
                        .replace("\"", "\\\"")
                        .replace("\t", "\\t")
                        .replace("\n", "\\n"),
                    textstring2
                        .replace("\"", "\\\"")
                        .replace("\t", "\\t")
                        .replace("\n", "\\n")
                );
            }
            return Ok(false);
        }
        if config.trim {
            if let Ok(trimmed) = text.textselection(&offset1)?.trim_text(&TRIM_CHARS) {
                textsel1 = trimmed;
                textstring1 = textsel1.text();
            } else {
                //nothing left to align
                return Ok(false);
            }
            if let Ok(trimmed) = text2.textselection(&offset2)?.trim_text(&TRIM_CHARS) {
                textsel2 = trimmed;
                textstring2 = textsel2.text();
            } else {
                //nothing left to align
                return Ok(false);
            }
        };
        if config.verbose {
            println!(
                "{}\t{}-{}\t{}\t{}-{}\t\"{}\"\t\"{}\"",
                text.resource().id().unwrap_or("-"),
                &textsel1.begin(),
                &textsel1.end(),
                text2.resource().id().unwrap_or("-"),
                &textsel2.begin(),
                &textsel2.end(),
                textstring1
                    .replace("\"", "\\\"")
                    .replace("\t", "\\t")
                    .replace("\n", "\\n"),
                textstring2
                    .replace("\"", "\\\"")
                    .replace("\t", "\\t")
                    .replace("\n", "\\n")
            );
        }
        let offset1: Offset = textsel1.inner().into();
        let offset2: Offset = textsel2.inner().into();
        select1.push(SelectorBuilder::TextSelector(
            text.resource().handle().into(),
            offset1,
        ));
        select2.push(SelectorBuilder::TextSelector(
            text2.resource().handle().into(),
            offset2,
        ));
        Ok(true)
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
            let alignment = match config.algorithm {
                AlignmentAlgorithm::SmithWaterman { .. } => alignment_set.local_alignment(),
                AlignmentAlgorithm::NeedlemanWunsch { .. } => alignment_set.global_alignment(),
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
                            fragment.publish(&mut select1, &mut select2, text, text2, config)?;
                        }
                    }
                }
            }
            if let Some(fragment) = fragment.take() {
                fragment.publish(&mut select1, &mut select2, text, text2, config)?;
            }

            if select1.is_empty() || (config.simple_only && select1.len() > 1) {
                //no alignment found
                //MAYBE TODO: compute and constrain by score?
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

pub fn alignments_tsv_out<'a>(
    store: &'a AnnotationStore,
    query: Query<'a>,
    use_var: Option<&str>,
) -> Result<(), StamError> {
    let iter = store.query(query)?;
    for resultrow in iter {
        if let Ok(result) = resultrow.get_by_name_or_last(use_var) {
            if let QueryResultItem::Annotation(annotation) = result {
                print_transposition(annotation);
            } else {
                return Err(StamError::OtherError(
                    "Only queries that return ANNOTATION are supported when outputting aligments",
                ));
            }
        }
    }
    Ok(())
}

pub fn print_transposition<'a>(annotation: &ResultItem<'a, Annotation>) {
    let mut annoiter = annotation.annotations_in_targets(AnnotationDepth::One);
    if let (Some(left), Some(right)) = (annoiter.next(), annoiter.next()) {
        //complex transposition
        for (text1, text2) in left.textselections().zip(right.textselections()) {
            print_alignment(annotation, &text1, &text2)
        }
    } else {
        //simple transposition
        let mut textiter = annotation.textselections();
        if let (Some(text1), Some(text2)) = (textiter.next(), textiter.next()) {
            print_alignment(annotation, &text1, &text2)
        }
    }
}

fn print_alignment<'a>(
    annotation: &ResultItem<'a, Annotation>,
    text1: &ResultTextSelection<'a>,
    text2: &ResultTextSelection<'a>,
) {
    println!(
        "{}\t{}\t{}-{}\t{}\t{}-{}\t\"{}\"\t\"{}\"\t{}",
        annotation.id().unwrap_or("-"),
        text1.resource().id().unwrap_or("-"),
        text1.begin(),
        text1.end(),
        text2.resource().id().unwrap_or("-"),
        text2.begin(),
        text2.end(),
        text1
            .text()
            .replace("\"", "\\\"")
            .replace("\t", "\\t")
            .replace("\n", "\\n"),
        text2
            .text()
            .replace("\"", "\\\"")
            .replace("\t", "\\t")
            .replace("\n", "\\n"),
        {
            let ids: Vec<_> = annotation
                .annotations_in_targets(AnnotationDepth::One)
                .map(|a| a.id().unwrap_or("-"))
                .collect();
            ids.join("|")
        }
    );
}
