use stam::*;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::process::exit;

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum TagKind {
    None,
    Key,         //or label if set
    KeyAndValue, //or label if set
    Value,
}

pub struct Highlight<'a> {
    annotations: BTreeSet<AnnotationHandle>,
    key: Option<ResultItem<'a, DataKey>>,
    label: Option<&'a str>,
    kind: TagKind,
}

impl<'a> Highlight<'a> {
    pub fn new(annotations: BTreeSet<AnnotationHandle>) -> Self {
        Self {
            annotations,
            key: None,
            label: None,
            kind: TagKind::KeyAndValue,
        }
    }

    pub fn with_key(mut self, key: ResultItem<'a, DataKey>) -> Self {
        self.key = Some(key);
        self
    }

    pub fn get_tag(&self, annotation: ResultItem<'a, Annotation>) -> Cow<'a, str> {
        if let Some(key) = &self.key {
            match self.kind {
                TagKind::Key => Cow::Borrowed(self.label.unwrap_or(key.as_str())),
                TagKind::KeyAndValue => {
                    if let Some(data) = annotation.data().filter_key(key).next() {
                        Cow::Owned(format!(
                            "{}: <em>{}</em>",
                            self.label.unwrap_or(key.as_str()),
                            data.value()
                        ))
                    } else {
                        Cow::Borrowed(self.label.unwrap_or(key.as_str()))
                    }
                }
                TagKind::Value => {
                    if let Some(data) = annotation.data().filter_key(key).next() {
                        Cow::Owned(format!("<em>{}</em>", data.value().to_string()))
                    } else {
                        Cow::Borrowed(self.label.unwrap_or(key.as_str()))
                    }
                }
                _ => Cow::Borrowed(""),
            }
        } else {
            Cow::Borrowed("")
        }
    }
}

pub struct HtmlWriter<'a> {
    store: &'a AnnotationStore,
    resource_ids: Vec<&'a str>,
    highlights: Vec<Highlight<'a>>,
    /// Output annotation IDs in the data-annotations attribute
    output_annotation_ids: bool,
    /// Output annotation data IDs in data-annotationdata attribute
    output_data_ids: bool,
    /// Output key IDs in data-keys attribute
    output_key_ids: bool,
    /// Output position in data-pos attribute
    output_offset: bool,
    /// Prune the data so only the highlights are expressed, nothing else
    prune: bool,
    /// Output annotations and data in a <script> block (javascript)
    output_data: bool,
    /// html header
    header: Option<&'a str>,
    /// html header
    footer: Option<&'a str>,
}

const HTML_HEADER: &str = "<html>
<head>
    <meta charset=\"UTF-8\" />
</head>
<style>
div.resource {
    background: white;
    font-family: monospace;
    border: 1px solid black;
    padding: 10px;
    margin: 10px;
    line-height: 1.5em;
}
.a { /* annotation */
    background: #dedede; /* light gray */
}
label {
    font-size: 70%;
    background: #ddd !important;
    border-radius: 0px 10px 0px 0px;
    font-weight: bold;
    padding-left: 2px;
    padding-right: 2px;
}
/* highlights */
span.hi1 {
    background: #b4e0aa; /*green*/
}
label.hi1 {
    color: #1d610d;
    border-bottom: 2px solid #b4e0aa;
    border-right: 5px solid #b4e0aa;
    background: #b4e0aa77;
}
span.hi2 {
    background: #aaace0; /*blue */
}
label.hi2 {
    color: #181c6b;
    border-bottom: 2px solid #aaace0;
    border-right: 5px solid #aaace0;
    background: #aaace077;
}
span.hi3 {
    background: #e19898; /*red*/
}
label.hi3 {
    color: #661818;
    border-bottom: 2px solid #e19898;
    border-right: 5px solid #e19898;
    background: #e1989877;
}
span.hi4 {
    background: #e1e098; /*yellow */
}
label.hi4 {
    color: #585712;
    border-bottom: 2px solid #e1e098;
    border-right: 5px solid #e1e098;
    background: #e1e09877;
}
span.hi5 {
    background: #98e1dd; /*cyan*/
}
label.hi5 {
    color: #126460;
    border-bottom: 2px solid #126460;
    border-right: 5px solid #126460;
    background: #12646077;
}
span.hi6 {
    background: #dcc6da; /*pink*/
}
label.hi6 {
    color: #5e1457;
    border-bottom: 2px solid #dcc6da;
    border-right: 5px solid #dcc6da;
    background: #dcc6da77;
}
span.hi7 {
    background: #e1c398; /*orange*/
}
label.hi7 {
    color: #5d3f14;
    border-bottom: 2px solid #e1c398;
    border-right: 5px solid #e1c398;
    background: #e1c39877;
}
span.hi8 {
    background: #6faa61; /*green*/
}
label.hi8 {
    color: #1a570b;
    border-bottom: 2px solid #6faa61;
    border-right: 5px solid #6faa61;
    background: #6faa6177;
}
span.hi9 {
    background: #79a3cb; /*blue */
}
span.hi10 {
    background: #bc5858; /*red*/
}
span.hi11 {
    background: #b2b158; /*yellow */
}
span.hi12 {
    background: #49b2ac; /*cyan*/
}
span.hi13 {
    background: #b977b3; /*pink*/
}
span.hi14 {
    background: #b9a161; /*orange*/
}
</style>
<body>";

const HTML_FOOTER: &str = "</body></html>";

impl<'a> HtmlWriter<'a> {
    pub fn new(store: &'a AnnotationStore, resource_ids: Vec<&'a str>) -> Self {
        Self {
            store,
            resource_ids,
            highlights: Vec::new(),
            output_annotation_ids: true,
            output_data_ids: false,
            output_key_ids: false,
            output_offset: true,
            output_data: false,
            prune: false,
            header: Some(HTML_HEADER),
            footer: Some(HTML_FOOTER),
        }
    }

    pub fn with_highlight(mut self, highlight: Highlight<'a>) -> Self {
        self.highlights.push(highlight);
        self
    }

    pub fn with_annotation_ids(mut self, value: bool) -> Self {
        self.output_annotation_ids = value;
        self
    }
    pub fn with_data_ids(mut self, value: bool) -> Self {
        self.output_data_ids = value;
        self
    }
    pub fn with_key_ids(mut self, value: bool) -> Self {
        self.output_key_ids = value;
        self
    }
    pub fn with_pos(mut self, value: bool) -> Self {
        self.output_offset = value;
        self
    }
    pub fn with_prune(mut self, value: bool) -> Self {
        self.prune = value;
        self
    }
    pub fn with_header(mut self, html: Option<&'a str>) -> Self {
        self.header = html;
        self
    }
    pub fn with_footer(mut self, html: Option<&'a str>) -> Self {
        self.footer = html;
        self
    }
    pub fn with_data_script(mut self, value: bool) -> Self {
        self.output_data = value;
        self
    }
}

impl<'a> Display for HtmlWriter<'a> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        if let Some(header) = self.header {
            write!(f, "{}", header)?;
        }
        for resource_id in self.resource_ids.iter() {
            let mut all_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
            if let Some(resource) = self.store.resource(*resource_id) {
                write!(
                    f,
                    "<div class=\"resource\" data-resource=\"{}\">\n",
                    resource_id
                )?;
                let mut span_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
                let mut begin: usize = 0;
                for i in resource.as_ref().positions(stam::PositionMode::Both) {
                    if *i > begin {
                        let text = resource
                            .text_by_offset(&Offset::simple(begin, *i))
                            .expect("offset should be valid");
                        write!(
                            f,
                            "{}",
                            html_escape::encode_text(text.replace("\n", "<br/>").as_str())
                        )?;
                        begin = *i;
                    }
                    if !span_annotations.is_empty() {
                        write!(f, "</span>")?;
                    }
                    if let Some(position) = resource.as_ref().position(*i) {
                        for (_, textselection) in position.iter_end2begin() {
                            let textselection = resource
                                .as_ref()
                                .get(*textselection)
                                .unwrap()
                                .as_resultitem(resource.as_ref(), self.store);
                            let close: Vec<_> =
                                textselection.annotations().map(|a| a.handle()).collect();
                            span_annotations.retain(|a| {
                                if close.contains(a) {
                                    for (j, highlights) in self.highlights.iter().enumerate() {
                                        if highlights.annotations.contains(a) {
                                            if let Some(annotation) = self.store.annotation(*a) {
                                                let tag = highlights.get_tag(annotation);
                                                if !tag.is_empty() {
                                                    write!(
                                                        f,
                                                        "<label class=\"hi{}\">{}</label>",
                                                        j + 1,
                                                        tag
                                                    )
                                                    .ok();
                                                }
                                            }
                                        }
                                    }
                                    false
                                } else {
                                    true
                                }
                            });
                        }
                        for (_, textselection) in position.iter_begin2end() {
                            let textselection = resource
                                .as_ref()
                                .get(*textselection)
                                .unwrap()
                                .as_resultitem(resource.as_ref(), self.store);
                            let new_span_annotations: BTreeSet<AnnotationHandle> =
                                textselection.annotations().map(|a| a.handle()).collect();
                            span_annotations.extend(new_span_annotations.iter());
                            if self.output_data {
                                all_annotations.extend(new_span_annotations.iter());
                            }
                        }
                        if self.prune {
                            span_annotations.retain(|a| {
                                for highlights in self.highlights.iter() {
                                    if highlights.annotations.contains(a) {
                                        return true;
                                    }
                                }
                                false
                            })
                        }
                        if !span_annotations.is_empty() {
                            let mut classes = vec!["a".to_string()];
                            for (j, highlights) in self.highlights.iter().enumerate() {
                                if span_annotations
                                    .intersection(&highlights.annotations)
                                    .next()
                                    .is_some()
                                {
                                    classes.push(format!("hi{}", j + 1));
                                }
                            }
                            write!(f, "<span")?;
                            if !classes.is_empty() {
                                write!(f, " class=\"{}\"", classes.join(" "))?;
                            }
                            if self.output_annotation_ids {
                                write!(
                                    f,
                                    " data-annotations=\"{}\"",
                                    span_annotations
                                        .iter()
                                        .map(|a_handle| {
                                            let annotation = self.store.get(*a_handle).unwrap();
                                            annotation
                                                .id()
                                                .map(|x| x.to_string())
                                                .unwrap_or_else(|| annotation.temp_id().unwrap())
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" "),
                                )?;
                            }
                            if self.output_offset {
                                write!(f, " data-offset=\"{}\"", i)?;
                            }
                            write!(f, ">")?;
                        }
                    }
                }
                if resource.textlen() > begin {
                    let text = resource
                        .text_by_offset(&Offset::simple(begin, resource.textlen()))
                        .expect("offset should be valid");
                    write!(
                        f,
                        "{}",
                        html_escape::encode_text(text.replace("\n", "<br/>").as_str())
                    )?;
                }
                write!(f, "</div>")?;
            } else {
                eprintln!("Error: Resource with ID {} does not exist", resource_id);
                exit(1);
            }
            if self.output_data {
                //TODO: call data_to_json()
            }
        }
        if let Some(footer) = self.footer {
            write!(f, "{}", footer)?;
        }
        Ok(())
    }
}

/*
fn data_to_json(store: &AnnotationStore, annotations: impl Iterator<Item = AnnotationHandle>) -> String {
        print!("annotations = {{");
        for a_handle in all_annotations.iter() {
            let annotation = store.get(*a_handle).unwrap();
            print!("  \"\"
        }
        print!("}}");
}
*/
