use crate::align::print_transposition as print_translation;
use serde::Deserialize;
use stam::*;
use std::borrow::Cow;
use std::collections::HashMap;
use toml;

pub fn translate<'store>(
    store: &'store mut AnnotationStore,
    mut translation_queries: Vec<Query<'store>>,
    queries: Vec<Query<'store>>,
    use_translation_var: Option<&str>,
    use_var: Option<&str>,
    id_prefix: Option<String>,
    idstrategy: IdStrategy,
    ignore_errors: bool,
    verbose: bool,
    config: TranslateConfig,
) -> Result<Vec<AnnotationHandle>, StamError> {
    let mut builders = Vec::new();
    while translation_queries.len() < queries.len() {
        let query = translation_queries
            .get(translation_queries.len() - 1)
            .expect("there must be translation queries");
        translation_queries.push(query.clone());
    }
    for (translation_query, query) in translation_queries.into_iter().zip(queries.into_iter()) {
        let iter = store.query(translation_query)?;
        let mut translation: Option<ResultItem<Annotation>> = None;
        let translationdata = store
            .find_data(
                "https://w3id.org/stam/extensions/stam-translate/",
                "Translation",
                DataOperator::Null,
            )
            .next()
            .ok_or_else(|| {
                StamError::OtherError(
                    "No translations at all were found in the annotation store (the STAM translate vocabulary is not present in the store)",
                )
            })?;
        for resultrow in iter {
            if let Ok(QueryResultItem::Annotation(annotation)) =
                resultrow.get_by_name_or_last(use_translation_var)
            {
                if !annotation.has_data(&translationdata) {
                    return Err(StamError::OtherError(
                        "The retrieved annotation is not explicitly marked as a translation, refusing to use",
                    ));
                }
                translation = Some(annotation.clone());
                break;
            }
        }
        if let Some(translation) = translation {
            let iter = store.query(query)?;
            for resultrow in iter {
                if let Ok(QueryResultItem::Annotation(annotation)) =
                    resultrow.get_by_name_or_last(use_var)
                {
                    let mut config = config.clone();
                    if let Some(id) = annotation.id() {
                        let randomid = generate_id("", "");
                        config.translation_id = if let Some(id_prefix) = &id_prefix {
                            Some(format!("{}{}-translation-{}", id_prefix, id, randomid))
                        } else {
                            Some(format!("{}-translation-{}", id, randomid))
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
                    match annotation.translate(&translation, config) {
                        Ok(results) => builders.extend(results),
                        Err(StamError::NoText(_)) => {
                            eprintln!(
                                "WARNING: Skipping translation of annotation that references no text: {}",
                                annotation.id().unwrap_or("(no id)"),
                            );
                        }
                        Err(err) => {
                            eprintln!(
                                "WARNING: Failed to translate annotation {}: {}",
                                annotation.id().unwrap_or("(no id)"),
                                err
                            );
                            if !ignore_errors {
                                return Err(StamError::OtherError(
                                    "Failed to translate annotation",
                                ));
                            }
                        }
                    }
                } else {
                    return Err(StamError::OtherError(
                            "Query should return instances of ANNOTATION to translate, got something else instead",
                        ));
                }
            }
        } else {
            return Err(StamError::OtherError(
                "Translation queries should return an ANNOTATION that is a translation, none found",
            ));
        }
    }
    let mut annotations = Vec::with_capacity(builders.len());
    for builder in builders {
        let annotation_handle = if !config.modify_existing {
            //add new annotation
            let annotation_handle = store.annotate(builder)?;
            annotations.push(annotation_handle);
            annotation_handle
        } else {
            //modify existing annotation
            store.reannotate(builder, ReannotateMode::default())?
        };
        if verbose {
            let annotation = store
                .annotation(annotation_handle)
                .expect("annotation was just added");
            let translationdata = store
                .find_data(
                    "https://w3id.org/stam/extensions/stam-translate/",
                    "Translation",
                    DataOperator::Null,
                )
                .next()
                .ok_or_else(|| {
                    StamError::OtherError(
                        "No translations at all were found in the annotation store (the STAM translate vocabulary is not present in the store)",
                    )
                })?;
            if annotation.has_data(&translationdata) {
                print_translation(&annotation);
            } else if config.modify_existing {
                eprintln!(
                    "# updated annotation {}",
                    annotation.id().expect("annotation must have ID")
                );
            } else {
                eprintln!(
                    "# added annotation {}",
                    annotation.id().expect("annotation must have ID")
                );
            }
        }
    }
    if verbose {
        if !config.modify_existing {
            eprintln!("{} annotations(s) created", annotations.len());
        } else {
            eprintln!("{} annotations(s) updated", annotations.len());
        }
    }
    Ok(annotations)
}

#[derive(Clone, Default, Deserialize, Debug)]
pub struct TranslateTextRule {
    source: Option<String>,
    target: String,
    left: Option<String>,
    right: Option<String>,

    #[serde(default = "f_true")]
    case_sensitive: bool,

    #[serde(default)]
    invert_context_match: bool,

    #[serde(default)]
    constraints: Vec<TranslateTextConstraint>,

    #[serde(skip)]
    source_regex: Option<Regex>,
    #[serde(skip)]
    left_regex: Option<Regex>,
    #[serde(skip)]
    right_regex: Option<Regex>,
}

// well, this is a bit silly, used in macro above
fn f_true() -> bool {
    true
}

#[derive(Clone, Default, Deserialize, Debug)]
pub struct TranslateTextConstraint {
    query: String,

    #[serde(default)]
    test: Option<String>,

    #[serde(default)]
    invert: bool,
}

pub struct MatchedRule<'a> {
    source: &'a str,
    target: Cow<'a, str>,
}

impl TranslateTextRule {
    /// Tests whether this rule matches the text at the specified cursor
    pub fn test<'a>(&'a self, text: &'a str, bytecursor: usize) -> Option<MatchedRule<'a>> {
        if let Some(source_regex) = self.source_regex.as_ref() {
            //check if text under cursor matches (regular expression test)
            if let Some(m) = source_regex.find(&text[bytecursor..]) {
                if self.test_context(text, bytecursor, m.len()) {
                    return Some(MatchedRule {
                        target: self.get_target(m.as_str()),
                        source: m.as_str(),
                    });
                }
            }
        } else if let Some(source) = self.source.as_ref() {
            //check if text under cursor matches (normal test)
            if bytecursor + source.len() <= text.len() {
                if let Some(candidate) = text.get(bytecursor..bytecursor + source.len()) {
                    if ((self.case_sensitive && candidate == *source)
                        || (!self.case_sensitive && candidate.to_lowercase() == *source))
                        && self.test_context(text, bytecursor, source.len())
                    {
                        return Some(MatchedRule {
                            target: self.get_target(source.as_str()),
                            source: source.as_str().into(),
                        });
                    }
                }
            }
        }
        None
    }

    /// See if context constaints match
    fn test_context(&self, text: &str, bytecursor: usize, matchbytelen: usize) -> bool {
        if let Some(left_regex) = self.left_regex.as_ref() {
            //match left context using regular expressiong
            let leftcontext = &text[..bytecursor];
            if !left_regex.is_match(leftcontext) {
                if self.invert_context_match {
                    return true;
                } else {
                    return false;
                }
            }
        } else if let Some(left_pattern) = self.left.as_ref() {
            //match left context normally
            let leftcontext = &text[..bytecursor];
            if (self.case_sensitive && !leftcontext.ends_with(left_pattern))
                || (!self.case_sensitive
                    && leftcontext[std::cmp::min(0, bytecursor - left_pattern.len())..]
                        .to_lowercase()
                        != left_pattern.to_lowercase())
            {
                if self.invert_context_match {
                    return true;
                } else {
                    return false;
                }
            }
        }
        if let Some(right_regex) = self.right_regex.as_ref() {
            //match right context using regular expression
            let rightcontext = &text[bytecursor + matchbytelen..];
            if !right_regex.is_match(rightcontext) {
                if self.invert_context_match {
                    return true;
                } else {
                    return false;
                }
            }
        } else if let Some(right_pattern) = self.right.as_ref() {
            //match right context normally
            let rightcontext = &text[bytecursor + matchbytelen..];
            if (self.case_sensitive && !rightcontext.starts_with(right_pattern))
                || (!self.case_sensitive
                    && rightcontext[..std::cmp::min(rightcontext.len(), right_pattern.len())]
                        .to_lowercase()
                        != right_pattern.to_lowercase())
            {
                if self.invert_context_match {
                    return true;
                } else {
                    return false;
                }
            }
        }
        if self.invert_context_match {
            return false;
        } else {
            return true;
        }
    }

    fn get_target<'a>(&'a self, source: &'a str) -> Cow<'a, str> {
        match self.target.as_str() {
            "$UPPER" => source.to_uppercase().into(),
            "$LOWER" => source.to_lowercase().into(),
            "$REVERSED" => Cow::Owned(source.chars().rev().collect::<String>()),
            _ => Cow::Borrowed(self.target.as_str()),
        }
    }
}

#[derive(Clone, Default, Deserialize, Debug)]
pub struct TranslateTextConfig {
    rules: Vec<TranslateTextRule>,

    /// ID Suffix for translated resources
    #[serde(default)]
    id_suffix: Option<String>,

    /// When no rules match, discard that part of the text entirely? By default it will just be copied and linked verbatim at character-level
    #[serde(default)]
    discard_unmatched: bool,

    /// Do generate any annotations, use this if you just want the text copied and don't mind losing all ties with the original
    #[serde(default)]
    no_annotations: bool,

    /// Create any texts and annotations even if the translation turn out exactly the same as the original
    #[serde(default)]
    force_when_unchanged: bool,

    #[serde(default)]
    debug: bool,
}

impl TranslateTextConfig {
    /// Parse the configuration from a TOML string (load the data from file yourself).
    pub fn from_toml_str(tomlstr: &str, debug: bool) -> Result<Self, String> {
        let mut config: Self = toml::from_str(tomlstr).map_err(|e| format!("{}", e))?;
        config.debug = debug;
        config.compile_regexps()?;
        Ok(config)
    }

    /// A suffix to assign when minting new IDs for resources and translations
    pub fn with_id_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.id_suffix = Some(suffix.into());
        self
    }

    /// Create any texts and annotations even if the translation turn out exactly the same as the original
    pub fn with_force_when_unchanged(mut self) -> Self {
        self.force_when_unchanged = true;
        self
    }

    /// A suffix to assign when minting new IDs for resources and translations
    pub fn with_debug(mut self, value: bool) -> Self {
        self.debug = value;
        self
    }

    fn compile_regexps<'a>(&'a mut self) -> Result<(), String> {
        for rule in self.rules.iter_mut() {
            if let Some(v) = rule.source.as_ref() {
                if v.starts_with('/') && v.ends_with('/') && v.len() > 1 {
                    let regex = format!("^{}", &v[1..v.len() - 1]);
                    rule.source_regex = Some(
                        RegexBuilder::new(&regex)
                            .case_insensitive(!rule.case_sensitive)
                            .build()
                            .map_err(|e| {
                                format!("Invalid regular expression for source: {}: {}", regex, e)
                            })?,
                    );
                    if self.debug {
                        eprintln!(
                            "[stam translatetext] compiled source regex {:?}",
                            rule.source_regex
                        )
                    }
                }
            }
            if let Some(v) = rule.left.as_ref() {
                if v.starts_with('/') && v.ends_with('/') && v.len() > 1 {
                    let regex = format!(".*{}$", &v[1..v.len() - 1]);
                    rule.left_regex = Some(
                        RegexBuilder::new(&regex)
                            .case_insensitive(!rule.case_sensitive)
                            .build()
                            .map_err(|e| {
                                format!(
                                    "Invalid regular expression for left context: {}: {}",
                                    regex, e
                                )
                            })?,
                    );
                    if self.debug {
                        eprintln!(
                            "[stam translatetext] compiled left context regex {:?}",
                            rule.left_regex
                        )
                    }
                }
            }
            if let Some(v) = rule.right.as_ref() {
                if v.starts_with('/') && v.ends_with('/') && v.len() > 1 {
                    let regex = format!("^{}.*", &v[1..v.len() - 1]);
                    rule.right_regex = Some(
                        RegexBuilder::new(&regex)
                            .case_insensitive(!rule.case_sensitive)
                            .build()
                            .map_err(|e| {
                                format!(
                                    "Invalid regular expression for right context: {}: {}",
                                    regex, e
                                )
                            })?,
                    );
                    if self.debug {
                        eprintln!(
                            "[stam translatetext] compiled right context regex {:?}",
                            rule.right_regex
                        )
                    }
                }
            }
            if rule.source.is_none() {
                return Err("Translation rules must have both a source".into());
            }
        }
        if self.debug {
            eprintln!("[stam translatetext] {} rules read", self.rules.len())
        }
        Ok(())
    }

    pub fn compile_queries<'a>(&'a self) -> Result<HashMap<String, Query<'a>>, String> {
        let mut compiled_queries = HashMap::new();
        for rule in self.rules.iter() {
            for constraint in rule.constraints.iter() {
                if !compiled_queries.contains_key(constraint.query.as_str()) {
                    compiled_queries.insert(
                        constraint.query.clone(),
                        stam::Query::parse(constraint.query.as_str())
                            .map_err(|err| format!("{}", err))?
                            .0,
                    );
                }
            }
        }
        Ok(compiled_queries)
    }
}

/// Translates a text given a configuration containing translation rules, returns a vector of TextResourceBuilders and a vector of AnnotationBuilders which build a single annotation per resource that maps source to target.
pub fn translate_text<'store>(
    store: &'store AnnotationStore,
    queries: Vec<Query<'store>>,
    usevar: Option<&'store str>,
    config: &TranslateTextConfig,
) -> Result<(Vec<TextResourceBuilder>, Vec<AnnotationBuilder<'static>>), String> {
    let mut annotations = Vec::new();
    let mut resourcebuilders = Vec::new();
    let constraint_queries = config.compile_queries()?;

    let mut seqnr = 0;
    for query in queries.into_iter() {
        let iter = store.query(query).map_err(|e| format!("{}", e))?;
        for resultrow in iter {
            if let Ok(result) = resultrow.get_by_name_or_last(usevar) {
                match result {
                    QueryResultItem::TextResource(resource) => {
                        let resource_id = resource.id().expect("resource must have ID");
                        let new_resource_id = format!(
                            "{}.{}{}",
                            if resource_id.ends_with(".txt") {
                                &resource_id[..resource_id.len() - 4]
                            } else if resource_id.ends_with(".md") {
                                &resource_id[..resource_id.len() - 3]
                            } else {
                                resource_id
                            },
                            config
                                .id_suffix
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("translation"),
                            if resource_id.ends_with(".txt") {
                                ".txt"
                            } else if resource_id.ends_with(".md") {
                                ".md"
                            } else {
                                ""
                            }
                        );
                        let new_filename = if let Some(filename) = resource.as_ref().filename() {
                            Some(format!(
                                "{}.{}.txt",
                                if filename.ends_with(".txt") {
                                    &filename[..filename.len() - 4]
                                } else if filename.ends_with(".md") {
                                    &filename[..filename.len() - 3]
                                } else {
                                    filename
                                },
                                config
                                    .id_suffix
                                    .as_ref()
                                    .map(|s| s.as_str())
                                    .unwrap_or("translation")
                            ))
                        } else {
                            None
                        };
                        translate_text_helper(
                            config,
                            store,
                            resource.text(),
                            resource,
                            0,
                            new_resource_id,
                            new_filename,
                            &mut resourcebuilders,
                            &mut annotations,
                            &constraint_queries,
                        )?;
                    }
                    QueryResultItem::TextSelection(textselection) => {
                        seqnr += 1;
                        let resource = textselection.resource();
                        let new_resource_id = format!(
                            "{}.{}.{}",
                            resource.id().expect("resource must have ID"),
                            config
                                .id_suffix
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("translation"),
                            seqnr
                        );
                        let new_filename = if let Some(filename) = resource.as_ref().filename() {
                            Some(format!(
                                "{}.{}.{}.txt",
                                if filename.ends_with(".txt") {
                                    &filename[..filename.len() - 4]
                                } else if filename.ends_with(".md") {
                                    &filename[..filename.len() - 3]
                                } else {
                                    filename
                                },
                                config
                                    .id_suffix
                                    .as_ref()
                                    .map(|s| s.as_str())
                                    .unwrap_or("translation"),
                                seqnr
                            ))
                        } else {
                            None
                        };
                        translate_text_helper(
                            config,
                            store,
                            textselection.text(),
                            &resource,
                            textselection.begin(),
                            new_resource_id,
                            new_filename,
                            &mut resourcebuilders,
                            &mut annotations,
                            &constraint_queries,
                        )?;
                    }
                    _ => {
                        return Err(
                            "translatetext is only implemented for resources and text selections at the moment"
                                .into(),
                        );
                    }
                }
            }
        }
    }

    Ok((resourcebuilders, annotations))
}

fn translate_text_helper<'store, 'a>(
    config: &TranslateTextConfig,
    store: &'store AnnotationStore,
    text: &'store str,
    resource: &ResultItem<'store, TextResource>,
    baseoffset: usize,
    new_resource_id: String,
    new_filename: Option<String>,
    resourcebuilders: &mut Vec<TextResourceBuilder>,
    annotations: &mut Vec<AnnotationBuilder<'static>>,
    constraint_queries: &HashMap<String, Query<'a>>,
) -> Result<(), String> {
    let mut new_text =
        String::with_capacity(text.len() + (0.1 * text.len() as f64).round() as usize); //reserve 10% extra capacity

    let mut sourceselectors: Vec<SelectorBuilder<'static>> = Vec::new();
    let mut targetselectors: Vec<SelectorBuilder<'static>> = Vec::new();

    let mut skipbytes = 0;
    let mut targetcharpos = 0;
    for (charpos, (bytepos, c)) in text.char_indices().enumerate() {
        if skipbytes > 0 {
            skipbytes -= c.len_utf8();
            continue;
        }
        let mut foundrule = false;
        for rule in config.rules.iter().rev() {
            if let Some(m) = rule.test(text, bytepos) {
                if !rule.constraints.is_empty() {
                    let mut constraints_match = true; //falsify
                    let sourcecharlen = m.source.chars().count();
                    let source = resource
                        .textselection(&Offset::simple(charpos, sourcecharlen))
                        .map_err(|e| format!("Failed to extract source: {}", e))?;
                    let left = resource
                        .textselection(&Offset::new(
                            Cursor::BeginAligned(0),
                            Cursor::BeginAligned(charpos),
                        ))
                        .map_err(|e| format!("Failed to extract left context: {}", e))?;
                    let right = resource
                        .textselection(&Offset::new(
                            Cursor::BeginAligned(charpos + sourcecharlen), //MAYBE TODO: check if this holds for final char as well?
                            Cursor::EndAligned(0),
                        ))
                        .map_err(|e| format!("Failed to extract right context: {}", e))?;
                    for constraint in rule.constraints.iter() {
                        //match constraint
                        let mut query = constraint_queries
                            .get(constraint.query.as_str())
                            .expect("constraint query should have been compiled earlier")
                            .clone();
                        query.bind_resourcevar("resource", resource);
                        query.bind_textvar("source", &source);
                        query.bind_textvar("left", &left);
                        query.bind_textvar("right", &right);
                        let mut iter = store
                            .query(query)
                            .map_err(|e| format!("Constraint query failed: {}", e))?;
                        if let Some(result) = iter.next() {
                            //only one iteration suffices (for now)
                            if let Some(testvar) = constraint.test.as_ref() {
                                if result.get_by_name(testvar.as_str()).is_ok() {
                                    if constraint.invert {
                                        constraints_match = false;
                                        break;
                                    }
                                } else if !constraint.invert {
                                    constraints_match = false;
                                    break;
                                }
                            } else if constraint.invert {
                                //results (no specific test variable)
                                constraints_match = false;
                                break;
                            }
                        } else if !constraint.invert {
                            //no results
                            constraints_match = false;
                            break;
                        }
                    }
                    if !constraints_match {
                        if config.debug {
                            eprintln!(
                                "[stam translatetext] @{} failed to matched rule {:?} -> {:?} because of unmet constraints",
                                charpos, m.source, m.target
                            )
                        }
                        continue; //skip to next rule
                    }
                }

                skipbytes += m.source.len() - c.len_utf8(); //skip the remainder (everything except the char we're already covering)

                if config.debug {
                    eprintln!(
                        "[stam translatetext] @{} (byte {}) matched rule {:?} -> {:?}",
                        charpos, bytepos, m.source, m.target
                    )
                }

                new_text += &m.target;

                if !config.no_annotations {
                    sourceselectors.push(SelectorBuilder::TextSelector(
                        resource.handle().into(),
                        Offset::simple(
                            baseoffset + charpos,
                            baseoffset + charpos + m.source.chars().count(),
                        ),
                    ));
                    let targetlen = m.target.chars().count();
                    targetselectors.push(SelectorBuilder::TextSelector(
                        new_resource_id.clone().into(),
                        Offset::simple(targetcharpos, targetcharpos + targetlen),
                    ));
                    targetcharpos += targetlen;
                }

                foundrule = true;
                continue; //stop at first matching rule (last in config file as we reversed order)
            }
        }

        if !foundrule && !config.discard_unmatched {
            if config.debug {
                eprintln!(
                    "[stam translatetext] @{} (byte {}) no rule matches {:?}, falling back",
                    charpos, bytepos, c
                )
            }
            //no rule matches, translate character verbatim
            new_text.push(c);
            if !config.no_annotations {
                sourceselectors.push(SelectorBuilder::TextSelector(
                    resource.handle().into(),
                    Offset::simple(baseoffset + charpos, baseoffset + charpos + 1),
                ));
                targetselectors.push(SelectorBuilder::TextSelector(
                    new_resource_id.clone().into(),
                    Offset::simple(targetcharpos, targetcharpos + 1),
                ));
            }
            targetcharpos += 1;
        }
    }

    if !config.force_when_unchanged && new_text.as_str() == text {
        eprintln!(
            "[stam translatetext] text for {} has not changed after translation, skipping..",
            new_resource_id
        );
        return Ok(());
    }

    let mut resourcebuilder = TextResourceBuilder::new()
        .with_text(new_text)
        .with_id(new_resource_id.clone());
    if let Some(new_filename) = new_filename {
        resourcebuilder = resourcebuilder.with_filename(new_filename);
    }
    resourcebuilders.push(resourcebuilder);

    if !config.no_annotations {
        annotations.push(
            AnnotationBuilder::new()
                .with_id(format!("{}.translation-source", new_resource_id.as_str()))
                .with_target(SelectorBuilder::DirectionalSelector(sourceselectors)),
        );
        annotations.push(
            AnnotationBuilder::new()
                .with_id(format!("{}.translation-target", new_resource_id.as_str()))
                .with_target(SelectorBuilder::DirectionalSelector(targetselectors)),
        );
        annotations.push(
            AnnotationBuilder::new()
                .with_id(format!("{}.translation", new_resource_id.as_str()))
                .with_data(
                    "https://w3id.org/stam/extensions/stam-translate/",
                    "Translation",
                    DataValue::Null,
                )
                .with_target(SelectorBuilder::DirectionalSelector(vec![
                    SelectorBuilder::AnnotationSelector(
                        format!("{}.translation-source", &new_resource_id).into(),
                        None,
                    ),
                    SelectorBuilder::AnnotationSelector(
                        format!("{}.translation-target", &new_resource_id).into(),
                        None,
                    ),
                ])),
        );
    }
    Ok(())
}
