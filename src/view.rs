use stam::*;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

use std::io::Write;
use std::str::Chars;

use crate::query::textselection_from_queryresult;

const WRITEFAILURE: &'static str = "ERROR: Buffer write failure";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tag<'a> {
    ///Highlight only, no tag
    None,

    //Show tag with public identifier
    Id,

    ///Show tag with key
    Key(ResultItem<'a, DataKey>), //or label if set

    ///Show tag with key and value
    KeyAndValue(ResultItem<'a, DataKey>), //or label if set

    ///Show tag with value (for a given key)
    Value(ResultItem<'a, DataKey>),
}

pub struct Highlight<'a> {
    tag: Tag<'a>,
    query: Option<Query<'a>>,
    label: Option<&'a str>,
}

impl<'a> Default for Highlight<'a> {
    fn default() -> Self {
        Self {
            label: None,
            query: None,
            tag: Tag::None,
        }
    }
}

impl<'a> Highlight<'a> {
    pub fn parse_query(
        mut query: &'a str,
        store: &'a AnnotationStore,
        seqnr: usize,
    ) -> Result<Self, String> {
        let mut attribs: Vec<&str> = Vec::new();
        while query.starts_with("@") {
            let pos = query
                .find(' ')
                .ok_or("query syntax error: expected space after @ATTRIBUTE")?;
            attribs.push(&query[0..pos]);
            query = &query[pos..query.len()];
        }
        let (query, _) = stam::Query::parse(query).map_err(|err| err.to_string())?;

        let mut tag = Tag::None;
        let key = get_key_from_query(&query, store);

        //prepared for a future in which we may have multiple attribs
        for attrib in attribs.iter() {
            match *attrib {
                "@KEYTAG" | "@KEY" => {
                    if let Some(key) = key.as_ref() {
                        tag = Tag::Key(key.clone())
                    } else {
                        eprintln!("Warning: Query has @KEYTAG attribute but no key was found in query constraints of query {}, ignoring...", seqnr);
                    }
                }
                "@KEYVALUETAG" | "@KEYVALUE" => {
                    if let Some(key) = key.as_ref() {
                        tag = Tag::KeyAndValue(key.clone())
                    } else {
                        eprintln!("Warning: Query has @KEYVALUETAG attribute but no key was found in query constraints of query {}, ignoring...", seqnr);
                    }
                }
                "@VALUETAG" | "@VALUE" => {
                    if let Some(key) = key.as_ref() {
                        tag = Tag::Value(key.clone())
                    } else {
                        eprintln!("Warning: Query has @VALUETAG attribute but no key was found in query constraints of query {}, ignoring...", seqnr);
                    }
                }
                "@IDTAG" | "ID" => tag = Tag::Id,
                _ => {}
            }
        }

        Ok(Highlight {
            tag,
            query: Some(query),
            label: None,
        })
    }

    pub fn with_tag(mut self, tag: Tag<'a>) -> Self {
        self.tag = tag;
        self
    }

    #[allow(dead_code)]
    pub fn with_query(mut self, query: Query<'a>) -> Self {
        self.query = Some(query);
        self
    }

    #[allow(dead_code)]
    pub fn with_label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Serializes the tag to string, given an annotation
    pub fn get_tag(&self, annotation: ResultItem<'a, Annotation>) -> Cow<'a, str> {
        match &self.tag {
            Tag::Key(key) => Cow::Borrowed(self.label.unwrap_or(key.as_str())),
            Tag::KeyAndValue(key) => {
                if let Some(data) = annotation.data().filter_key(key).next() {
                    Cow::Owned(format!(
                        "{}: {}",
                        self.label.unwrap_or(key.as_str()),
                        data.value()
                    ))
                } else {
                    Cow::Borrowed(self.label.unwrap_or(key.as_str()))
                }
            }
            Tag::Value(key) => {
                if let Some(data) = annotation.data().filter_key(key).next() {
                    Cow::Owned(data.value().to_string())
                } else {
                    Cow::Borrowed(self.label.unwrap_or(key.as_str()))
                }
            }
            Tag::Id => Cow::Borrowed(annotation.id().unwrap_or("")),
            Tag::None => Cow::Borrowed(""),
        }
    }
}

pub struct HtmlWriter<'a> {
    store: &'a AnnotationStore,
    selectionquery: Query<'a>,
    selectionvar: Option<&'a str>,
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
    /// html footer
    footer: Option<&'a str>,
    /// Output legend?
    legend: bool,
    /// Output titles (identifiers) for the primary selection?
    titles: bool,
}

const HTML_HEADER: &str = "<!DOCTYPE html>
<html>
<head>
    <meta charset=\"UTF-8\" />
    <meta name=\"generator\" content=\"stam view\" />
    <style type=\"text/css\">
div.resource, div.textselection {
    color: black;
    background: white;
    font-family: monospace;
    border: 1px solid black;
    padding: 10px;
    margin: 10px;
    line-height: 1.5em;
}
.a { /* annotation */
    /* background: #dedede;  light gray */
    vertical-align: bottom;
}
label {
    display: inline-block;
    margin-top: 10px;
    border-radius: 0px 20px 0px 0px;
}
label em {
    display: inline-block;
    font-size: 70%;
    padding-left: 5px;
    padding-right: 5px;
    vertical-align: bottom;
}
/* highlights for labels/tags */
label.tag1 {
    color: #1d610d;
    border-right: 5px solid #b4e0aa;
    background: #b4e0aa77;
}
label.tag2 {
    color: #181c6b;
    border-right: 5px solid #aaace0;
    background: #aaace077;
}
label.tag3 {
    color: #661818;
    border-right: 5px solid #e19898;
    background: #e1989877;
}
label.tag4 {
    color: #585712;
    border-right: 5px solid #e1e098;
    background: #e1e09877;
}
label.tag5 {
    color: #126460;
    border-right: 5px solid #126460;
    background: #12646077;
}
label.tag6 {
    color: #5e1457;
    border-right: 5px solid #dcc6da;
    background: #dcc6da77;
}
label.tag7 {
    color: #5d3f14;
    border-right: 5px solid #e1c398;
    background: #e1c39877;
}
label.tag8 {
    color: #1a570b;
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
span.l1, span.l2, span.l3, span.l4, span.l5, span.l6, span.l7, span.l8, span.l9, span.l10, span.l11, span.l12, span.l13, span.l14 {
    display: inline-block;
    border-bottom: 3px solid white;
}
.hi1 span.l1 {
    border-bottom: 3px solid #b4e0aa; /* green */
}
.hi2 span.l2 {
    border-bottom: 3px solid #aaace0; /* blueish/purple */
}
.hi3 span.l3 {
    border-bottom: 3px solid #e19898; /* red */
}
.hi4 span.l4 {
    border-bottom: 3px solid #e1e098; /* yellow */
}
.hi5 span.l5 {
    border-bottom: 3px solid #98e1dd; /* cyan */
}
.hi6 span.l6 {
    border-bottom: 3px solid #dcc6da; /* pink */
}
.hi7 span.l7 {
    border-bottom: 3px solid #e1c398; /* orange */
}

div#legend {
    color: black;
    width: 40%;
    min-width: 320px;
    margin-left: auto;
    margin-right: auto;
    font-family: sans-serif;
    padding: 5px;
    border: 1px dashed #ccc;
    border-radius: 20px;
}
div#legend ul {
    list-style: none;
}
div#legend ul li span {
    display: inline-block;
    width: 15px;
    border-radius: 15px;
    border: 1px #555 solid;
    min-height: 15px;
}
div#legend span.hi1 {
    background: #b4e0aa; /* green */
}
div#legend span.hi2 {
    background: #aaace0; /* blueish/purple */
}
div#legend span.hi3 {
    background: #e19898; /*red*/
}
div#legend span.hi4 {
    background: #e1e098; /*yellow */
}
div#legend span.hi5 {
    background: #98e1dd; /*cyan*/
}
div#legend span.hi6 {
    background: #dcc6da; /*pink*/
}
div#legend span.hi7 {
    background: #e1c398; /*orange*/
}
div#legend span.hi8 {
    background: #6faa61; /*green*/
}
body>h2 {
    color: black;
    font-size: 1.1em;
    font-family: sans-serif;
}
    </style>
</head>
<body>
";

const HTML_FOOTER: &str = "
</body></html>";

#[allow(dead_code)] //TODO: remove this at a later point when more of this is exposed as a library
impl<'a> HtmlWriter<'a> {
    pub fn new(store: &'a AnnotationStore, selectionquery: Query<'a>) -> Self {
        Self {
            store,
            selectionquery,
            selectionvar: None,
            highlights: Vec::new(),
            output_annotation_ids: true,
            output_data_ids: false,
            output_key_ids: false,
            output_offset: true,
            output_data: false,
            prune: false,
            header: Some(HTML_HEADER),
            footer: Some(HTML_FOOTER),
            legend: true,
            titles: true,
        }
    }

    pub fn with_highlight(mut self, highlight: Highlight<'a>) -> Self {
        self.highlights.push(highlight);
        self
    }
    pub fn with_legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn with_titles(mut self, value: bool) -> Self {
        self.titles = value;
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

    pub fn with_selectionvar(mut self, var: &'a str) -> Self {
        self.selectionvar = Some(var);
        self
    }

    fn output_error(&self, f: &mut Formatter, msg: &str) -> std::fmt::Result {
        write!(f, "<span class=\"error\">{}</span>", msg)?;
        if let Some(footer) = self.footer {
            write!(f, "{}", footer)?;
        }
        return Ok(());
    }

    pub fn add_highlights_from_query(&mut self) {
        helper_add_highlights_from_query(&mut self.highlights, &self.selectionquery, self.store);
    }
}

fn get_key_from_query<'a>(
    query: &Query<'a>,
    store: &'a AnnotationStore,
) -> Option<ResultItem<'a, DataKey>> {
    for constraint in query.iter() {
        if let Constraint::DataKey { set, key, .. } | Constraint::KeyValue { set, key, .. } =
            constraint
        {
            if let Some(key) = store.key(*set, *key) {
                return Some(key);
            }
        }
    }
    None
}

fn helper_add_highlights_from_query<'a>(
    highlights: &mut Vec<Highlight<'a>>,
    query: &Query<'a>,
    store: &'a AnnotationStore,
) {
    if let Some(key) = get_key_from_query(query, store) {
        highlights.push(Highlight::default().with_tag(Tag::KeyAndValue(key)))
    }
    if let Some(subquery) = query.subquery() {
        helper_add_highlights_from_query(highlights, subquery, store);
    }
}

impl<'a> Display for HtmlWriter<'a> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let mut highlights_results: Vec<BTreeSet<AnnotationHandle>> = Vec::new();
        for _ in 0..self.highlights.len() {
            highlights_results.push(BTreeSet::new());
        }
        if let Some(header) = self.header {
            write!(f, "{}", header)?;
        }
        write!(
            f,
            "<!-- Selection Query:\n\n{}\n\n-->\n",
            self.selectionquery
                .to_string()
                .unwrap_or_else(|err| format!("{}", err))
        )?;
        for (i, highlight) in self.highlights.iter().enumerate() {
            if let Some(query) = highlight.query.as_ref() {
                write!(
                    f,
                    "<!-- Highlight Query #{}:\n\n{}\n\n-->\n",
                    i + 1,
                    query.to_string().unwrap_or_else(|err| format!("{}", err))
                )?;
            }
        }
        if self.legend && self.highlights.iter().any(|hl| hl.query.is_some()) {
            write!(f, "<div id=\"legend\"><ul>")?;
            for (i, highlight) in self.highlights.iter().enumerate() {
                if let Some(hlq) = highlight.query.as_ref() {
                    write!(
                        f,
                        "<li><span class=\"hi{}\"></span> {}</li>",
                        i + 1,
                        hlq.name().unwrap_or("(untitled)").replace("_", " ")
                    )?;
                }
            }
            write!(f, "</ul></div>")?;
        }
        let results = self.store.query(self.selectionquery.clone());
        let names = results.names();
        let mut prevresult = None;
        let mut openingtags = String::new();
        let mut classes = vec![];
        for (resultnr, selectionresult) in results.enumerate() {
            //MAYBE TODO: the clone is a bit unfortunate but no big deal
            match textselection_from_queryresult(&selectionresult, self.selectionvar, &names) {
                Err(msg) => return self.output_error(f, msg),
                Ok((resulttextselection, whole_resource, id)) => {
                    if prevresult == Some(resulttextselection.clone()) {
                        //prevent duplicates (especially relevant when --use is set)
                        continue;
                    }
                    prevresult = Some(resulttextselection.clone());
                    if self.titles {
                        if let Some(id) = id {
                            write!(f, "<h2>{}. <span>{}</span></h2>\n", resultnr + 1, id,)?;
                        }
                    }
                    if whole_resource {
                        write!(
                            f,
                            "<div class=\"resource\" data-resource=\"{}\">\n",
                            resulttextselection.resource().id().unwrap_or("undefined"),
                        )?;
                    } else {
                        write!(
                            f,
                            "<div class=\"textselection\" data-resource=\"{}\" data-begin=\"{}\" data-end=\"{}\">\n",
                            resulttextselection.resource().id().unwrap_or("undefined"),
                            resulttextselection.begin(),
                            resulttextselection.end(),
                        )?;
                    }
                    let mut span_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
                    let resource = resulttextselection.resource();
                    let mut begin: usize = resulttextselection.begin();
                    let mut positions: Vec<_> = resulttextselection
                        .positions(stam::PositionMode::Both)
                        .copied()
                        .collect();
                    positions.push(resulttextselection.end());
                    for i in positions {
                        let mut needclosure = true;
                        if i > begin {
                            //output text
                            let text = resource
                                .text_by_offset(&Offset::simple(begin, i))
                                .expect("offset should be valid");
                            for (subtext, texttype, done) in LinebreakIter::new(text) {
                                match texttype {
                                    BufferType::Text => {
                                        write!(
                                            f,
                                            "{}",
                                            html_escape::encode_text(subtext)
                                                .replace(" ", "&ensp;")
                                                .as_str()
                                        )?;
                                    }
                                    BufferType::NewLines => {
                                        if !span_annotations.is_empty() {
                                            for _ in 0..self.highlights.len() {
                                                write!(f, "</span>")?;
                                            }
                                            write!(f, "</span>")?;
                                            write!(
                                                f,
                                                "{}",
                                                subtext.replace("\n", "<br/>").as_str()
                                            )?;
                                            if !done {
                                                //open spans again for the next subtext
                                                write!(f, "{}", openingtags)?;
                                            } else {
                                                needclosure = false;
                                            }
                                        }
                                    }
                                    BufferType::None => {}
                                }
                            }
                            begin = i;
                        }
                        if !span_annotations.is_empty() && needclosure {
                            for _ in 0..self.highlights.len() {
                                write!(f, "</span>")?;
                            }
                            write!(f, "</span>")?;
                        }

                        if let Some(position) = resource.as_ref().position(i) {
                            for (_, textselectionhandle) in position.iter_end2begin() {
                                let textselection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .unwrap()
                                    .as_resultitem(resource.as_ref(), self.store);
                                let close: Vec<_> =
                                    textselection.annotations().map(|a| a.handle()).collect();
                                let mut classes = vec![];
                                for (j, (_, highlights_annotations)) in self
                                    .highlights
                                    .iter()
                                    .zip(highlights_results.iter())
                                    .enumerate()
                                {
                                    if span_annotations
                                        .intersection(&highlights_annotations)
                                        .next()
                                        .is_some()
                                    {
                                        classes.push(format!("hi{}", j + 1));
                                    }
                                }
                                span_annotations.retain(|a| {
                                    if close.contains(a) {
                                        //close tags and add labels
                                        for (j, (highlights, highlights_results)) in self
                                            .highlights
                                            .iter()
                                            .zip(highlights_results.iter())
                                            .enumerate()
                                        {
                                            if highlights_results.contains(a) {
                                                if let Some(annotation) = self.store.annotation(*a)
                                                {
                                                    let tag = highlights.get_tag(annotation);
                                                    if !tag.is_empty() {
                                                        write!(
                                                            f,
                                                            "<label class=\"tag{} {}\">",
                                                            j + 1,
                                                            classes.join(" ")
                                                        )
                                                        .ok();
                                                        for i in 0..self.highlights.len() {
                                                            write!(
                                                                f,
                                                                "<span class=\"l{}\">",
                                                                i + 1
                                                            )
                                                            .ok();
                                                        }
                                                        write!(f, "<em>{}</em>", tag,).ok();
                                                        for _ in 0..self.highlights.len() {
                                                            write!(f, "</span>").ok();
                                                        }
                                                        write!(f, "</label>",).ok();
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

                            if i != resulttextselection.end() {
                                //find and add annotations that begin here
                                for (_, textselectionhandle) in position.iter_begin2end() {
                                    let textselection = resource
                                        .as_ref()
                                        .get(*textselectionhandle)
                                        .unwrap()
                                        .as_resultitem(resource.as_ref(), self.store);
                                    let new_span_annotations: BTreeSet<AnnotationHandle> =
                                        textselection.annotations().map(|a| a.handle()).collect();
                                    span_annotations.extend(new_span_annotations.iter());
                                    if self.output_data {
                                        //all_annotations.extend(new_span_annotations.iter());
                                    }
                                }

                                if self.prune || !span_annotations.is_empty() {
                                    //gather annotations for the textselection under consideration
                                    highlights_results = get_highlights_results(
                                        &self.selectionquery,
                                        &selectionresult,
                                        self.selectionvar,
                                        self.store,
                                        &self.highlights,
                                        &names,
                                        &span_annotations,
                                        highlights_results,
                                    );
                                }
                            }

                            if self.prune {
                                //prunes everything that is not highlighted
                                span_annotations.retain(|a| {
                                    for highlights_annotations in highlights_results.iter() {
                                        if highlights_annotations.contains(a) {
                                            return true;
                                        }
                                    }
                                    false
                                })
                            }
                            if !span_annotations.is_empty() && i != resulttextselection.end() {
                                //output the opening tags
                                classes.clear();
                                classes.push("a".to_string());
                                for (j, highlights_annotations) in
                                    highlights_results.iter().enumerate()
                                {
                                    if span_annotations
                                        .intersection(&highlights_annotations)
                                        .next()
                                        .is_some()
                                    {
                                        classes.push(format!("hi{}", j + 1));
                                    }
                                }
                                openingtags.clear();
                                openingtags += "<span";
                                if !classes.is_empty() {
                                    openingtags +=
                                        format!(" class=\"{}\"", classes.join(" ")).as_str();
                                }
                                if self.output_annotation_ids {
                                    openingtags += format!(
                                        " data-annotations=\"{}\"",
                                        span_annotations
                                            .iter()
                                            .map(|a_handle| {
                                                let annotation = self.store.get(*a_handle).unwrap();
                                                annotation
                                                    .id()
                                                    .map(|x| x.to_string())
                                                    .unwrap_or_else(|| {
                                                        annotation.temp_id().unwrap()
                                                    })
                                            })
                                            .collect::<Vec<_>>()
                                            .join(" "),
                                    )
                                    .as_str();
                                }
                                //incomplete but we output already
                                write!(f, "{}", openingtags.as_str())?;
                                if self.output_offset {
                                    //we don't want this in the openingtags buffer because it'd be behind if we reuse the opening tags later
                                    write!(f, " data-offset=\"{}\"", i)?;
                                }
                                openingtags += ">";
                                write!(f, ">")?;
                                for i in 0..self.highlights.len() {
                                    let layer = format!("<span class=\"l{}\">", i + 1);
                                    openingtags += layer.as_str();
                                    write!(f, "{}", layer)?;
                                }
                                //the text is outputted alongside the closing tag
                            }
                        }
                    }
                    writeln!(f, "\n</div>")?;
                    if self.output_data {
                        //TODO: call data_to_json()
                    }
                }
            }
        }
        if let Some(footer) = self.footer {
            write!(f, "{}", footer)?;
        }
        Ok(())
    }
}

pub struct AnsiWriter<'a> {
    store: &'a AnnotationStore,
    selectionquery: Query<'a>,
    selectionvar: Option<&'a str>,
    highlights: Vec<Highlight<'a>>,
    /// Prune the data so only the highlights are expressed, nothing else
    prune: bool,
    /// Output legend?
    legend: bool,
    /// Output titles (identifiers) for the primary selection?
    titles: bool,
}

impl<'a> AnsiWriter<'a> {
    pub fn new(store: &'a AnnotationStore, selectionquery: Query<'a>) -> Self {
        Self {
            store,
            selectionquery,
            selectionvar: None,
            highlights: Vec::new(),
            prune: false,
            legend: true,
            titles: true,
        }
    }

    pub fn with_highlight(mut self, highlight: Highlight<'a>) -> Self {
        self.highlights.push(highlight);
        self
    }
    pub fn with_legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn with_titles(mut self, value: bool) -> Self {
        self.titles = value;
        self
    }

    pub fn with_prune(mut self, value: bool) -> Self {
        self.prune = value;
        self
    }

    pub fn with_selectionvar(mut self, var: &'a str) -> Self {
        self.selectionvar = Some(var);
        self
    }

    fn output_error(&self, msg: &str) {
        eprintln!("ERROR: {}", msg);
    }

    pub fn add_highlights_from_query(&mut self) {
        helper_add_highlights_from_query(&mut self.highlights, &self.selectionquery, self.store);
    }

    fn writeansicol(&self, i: usize, s: &str) {
        let color = if i > 6 { 30 } else { 30 + i };
        let mut stdout = std::io::stdout();
        stdout.write(b"\x1b[").expect(WRITEFAILURE);
        stdout
            .write(&format!("{}", color).into_bytes())
            .expect(WRITEFAILURE);
        stdout.write(b"m").expect(WRITEFAILURE);
        stdout.flush().expect(WRITEFAILURE);
        print!("{}", s);
        stdout.write(b"\x1b[m").expect(WRITEFAILURE);
        stdout.flush().expect(WRITEFAILURE);
    }

    fn writeansicol_bold(&self, i: usize, s: &str) {
        let color = if i > 6 { 30 } else { 30 + i };
        let mut stdout = std::io::stdout();
        stdout.write(b"\x1b[").expect(WRITEFAILURE);
        stdout
            .write(&format!("{}", color).into_bytes())
            .expect(WRITEFAILURE);
        stdout.write(b";1m").expect(WRITEFAILURE);
        stdout.flush().expect(WRITEFAILURE);
        print!("{}", s);
        stdout.write(b"\x1b[m").expect(WRITEFAILURE);
        stdout.flush().expect(WRITEFAILURE);
    }

    fn writeheader(&self, s: &str) {
        let mut stdout = std::io::stdout();
        stdout.write(b"\x1b[37;1m").expect(WRITEFAILURE);
        stdout.flush().expect(WRITEFAILURE);
        print!("{}", s);
        stdout.write(b"\x1b[m").expect(WRITEFAILURE);
        stdout.flush().expect(WRITEFAILURE);
    }

    pub fn print(&self) {
        let mut highlights_results: Vec<BTreeSet<AnnotationHandle>> = Vec::new();
        for _ in 0..self.highlights.len() {
            highlights_results.push(BTreeSet::new());
        }
        if self.legend && self.highlights.iter().any(|hl| hl.query.is_some()) {
            println!("Legend:");
            for (i, highlight) in self.highlights.iter().enumerate() {
                if let Some(hlq) = highlight.query.as_ref() {
                    let s = format!(
                        "       {}. {}\n",
                        i + 1,
                        hlq.name().unwrap_or("(untitled)").replace("_", " ")
                    );
                    self.writeansicol_bold(i + 1, s.as_str());
                }
            }
            println!();
        }
        let results = self.store.query(self.selectionquery.clone());
        let names = results.names();
        let mut prevresult = None;
        for (resultnr, selectionresult) in results.enumerate() {
            //MAYBE TODO: the clone is a bit unfortunate but no big deal
            match textselection_from_queryresult(&selectionresult, self.selectionvar, &names) {
                Err(msg) => return self.output_error(msg),
                Ok((resulttextselection, _, id)) => {
                    if prevresult == Some(resulttextselection.clone()) {
                        //prevent duplicates (especially relevant when --use is set)
                        continue;
                    }
                    prevresult = Some(resulttextselection.clone());
                    if self.titles {
                        if let Some(id) = id {
                            let s = format!("----------------------------------- {}. {} -----------------------------------\n", resultnr + 1, id,);
                            self.writeheader(s.as_str());
                        }
                    }
                    let resource = resulttextselection.resource();
                    let mut begin: usize = resulttextselection.begin();
                    let mut positions: Vec<_> = resulttextselection
                        .positions(stam::PositionMode::Both)
                        .copied()
                        .collect();
                    positions.push(resulttextselection.end());
                    let mut span_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
                    for i in positions {
                        if i > begin {
                            //output text in buffer
                            let text = resource
                                .text_by_offset(&Offset::simple(begin, i))
                                .expect("offset should be valid");
                            print!("{}", text);
                            begin = i;
                        }

                        if let Some(position) = resource.as_ref().position(i) {
                            for (_, textselectionhandle) in position.iter_end2begin() {
                                let textselection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .unwrap()
                                    .as_resultitem(resource.as_ref(), self.store);
                                let close: Vec<_> =
                                    textselection.annotations().map(|a| a.handle()).collect();
                                span_annotations.retain(|a| {
                                    if close.contains(a) {
                                        //close tags and add labels
                                        for (j, (highlights, highlights_results)) in self
                                            .highlights
                                            .iter()
                                            .zip(highlights_results.iter())
                                            .enumerate()
                                        {
                                            if highlights_results.contains(a) {
                                                if let Some(annotation) = self.store.annotation(*a)
                                                {
                                                    //closing highlight after adding tag if needed
                                                    let tag = highlights.get_tag(annotation);
                                                    if !tag.is_empty() {
                                                        self.writeansicol(
                                                            j + 1,
                                                            format!("|{}", &tag).as_str(),
                                                        );
                                                    }
                                                    self.writeansicol_bold(j + 1, "]");
                                                }
                                            }
                                        }
                                        false
                                    } else {
                                        true
                                    }
                                });
                            }

                            let mut new_span_annotations: BTreeSet<AnnotationHandle> =
                                BTreeSet::new();

                            if i != resulttextselection.end() {
                                //find and add annotations that begin here
                                for (_, textselectionhandle) in position.iter_begin2end() {
                                    let textselection = resource
                                        .as_ref()
                                        .get(*textselectionhandle)
                                        .unwrap()
                                        .as_resultitem(resource.as_ref(), self.store);
                                    new_span_annotations
                                        .extend(textselection.annotations().map(|a| a.handle()));
                                }

                                if self.prune || !new_span_annotations.is_empty() {
                                    //gather annotations for the textselection under consideration
                                    highlights_results = get_highlights_results(
                                        &self.selectionquery,
                                        &selectionresult,
                                        self.selectionvar,
                                        self.store,
                                        &self.highlights,
                                        &names,
                                        &new_span_annotations,
                                        highlights_results,
                                    )
                                }
                            }
                            if self.prune {
                                //prunes everything that is not highlighted
                                span_annotations.retain(|a| {
                                    for highlights_annotations in highlights_results.iter() {
                                        if highlights_annotations.contains(a) {
                                            return true;
                                        }
                                    }
                                    false
                                })
                            }
                            if !new_span_annotations.is_empty() && i != resulttextselection.end() {
                                //output the opening tags
                                for (j, highlights_annotations) in
                                    highlights_results.iter().enumerate()
                                {
                                    if new_span_annotations
                                        .intersection(&highlights_annotations)
                                        .next()
                                        .is_some()
                                    {
                                        self.writeansicol_bold(j + 1, "[");
                                    }
                                }
                                //the text is outputted alongside the closing tag
                            }
                            span_annotations.extend(new_span_annotations.into_iter());
                        }
                    }
                }
            }
            println!();
        }
    }
}

#[inline]
fn get_highlights_results<'a>(
    selectionquery: &Query<'a>,
    selectionresult: &QueryResultItems,
    selectionvar: Option<&str>,
    store: &'a AnnotationStore,
    highlights: &[Highlight],
    names: &QueryNames,
    span_annotations: &BTreeSet<AnnotationHandle>,
    mut highlights_results: Vec<BTreeSet<AnnotationHandle>>,
) -> Vec<BTreeSet<AnnotationHandle>> {
    //gather annotations for the textselection under consideration
    for (j, highlights) in highlights.iter().enumerate() {
        if let Some(mut hlquery) = highlights.query.clone() {
            let mut annotations = BTreeSet::new();
            //make variable from selection query available in the highlight query:
            let varname = selectionvar.unwrap_or(
                selectionquery
                    .name()
                    .expect("you must name the variables in your SELECT statements"),
            );
            if let Some(parentresult) = selectionresult.get_by_name(names, varname).ok() {
                match parentresult {
                    QueryResultItem::None => {}
                    QueryResultItem::Annotation(x) => {
                        hlquery.bind_annotationvar(varname, x.clone());
                    }
                    QueryResultItem::TextSelection(x) => {
                        hlquery.bind_textvar(varname, x.clone());
                    }
                    QueryResultItem::AnnotationData(x) => {
                        hlquery.bind_datavar(varname, x.clone());
                    }
                    QueryResultItem::TextResource(x) => {
                        hlquery.bind_resourcevar(varname, x.clone());
                    }
                    QueryResultItem::AnnotationDataSet(x) => {
                        hlquery.bind_datasetvar(varname, x.clone());
                    }
                    QueryResultItem::DataKey(x) => {
                        hlquery.bind_keyvar(varname, x.clone());
                    }
                }
            } else {
                eprintln!(
                    "WARNING: unable to retrieve variable {} from main query",
                    varname
                );
            }
            //process result of highlight query and extra annotations
            for results in store.query(hlquery) {
                if let Some(result) = results.iter().last() {
                    match result {
                        &QueryResultItem::Annotation(ref annotation) => {
                            annotations.insert(annotation.handle());
                        }
                        &QueryResultItem::TextSelection(ref ts) => {
                            annotations.extend(ts.annotations().map(|x| x.handle()));
                        }
                        &QueryResultItem::AnnotationData(ref data) => {
                            annotations.extend(data.annotations().map(|x| x.handle()));
                        }
                        _ => {
                            eprintln!(
                                "WARNING: query for highlight {} has invalid resulttype",
                                j + 1
                            )
                        }
                    }
                }
            }
            highlights_results[j] = FromHandles::new(annotations.iter().copied(), store)
                .map(|x| x.handle())
                .collect();
        } else if let Tag::Key(key) | Tag::KeyAndValue(key) | Tag::Value(key) = &highlights.tag {
            //no query defined, only a tagkey, that's ok, we obtain the annotations directly:
            highlights_results[j] = FromHandles::new(span_annotations.iter().copied(), store)
                .filter_key(key)
                .map(|x| x.handle())
                .collect();
        }
    }
    highlights_results
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

#[derive(Copy, PartialEq, Clone, Debug)]
enum BufferType {
    None,
    NewLines,
    Text,
}

struct LinebreakIter<'a> {
    iter: Chars<'a>,
    text: &'a str,
    curbytepos: usize,
    beginbytepos: usize,
    buffertype: BufferType,
    done: bool,
}

impl<'a> LinebreakIter<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            iter: text.chars(),
            text,
            curbytepos: 0,
            beginbytepos: 0,
            buffertype: BufferType::None,
            done: false,
        }
    }
}

impl<'a> Iterator for LinebreakIter<'a> {
    type Item = (&'a str, BufferType, bool);
    fn next(&mut self) -> Option<Self::Item> {
        while !self.done {
            if let Some(c) = self.iter.next() {
                if (c == '\n' && self.buffertype == BufferType::NewLines)
                    || (self.buffertype == BufferType::Text)
                {
                    //same type as buffer, carry on
                    self.curbytepos += c.len_utf8();
                } else {
                    //switching buffers, yield result
                    let resultbuffertype = self.buffertype;
                    if c == '\n' {
                        self.buffertype = BufferType::NewLines;
                    } else {
                        self.buffertype = BufferType::Text;
                    }
                    if self.curbytepos > self.beginbytepos {
                        let result = &self.text[self.beginbytepos..self.curbytepos];
                        self.beginbytepos = self.curbytepos;
                        self.curbytepos += c.len_utf8();
                        return Some((result, resultbuffertype, self.done));
                    } else {
                        self.curbytepos += c.len_utf8();
                    }
                }
            } else {
                //return last buffer (if any)
                if self.curbytepos > self.beginbytepos && !self.done {
                    let result = &self.text[self.beginbytepos..];
                    self.done = true;
                    return Some((result, self.buffertype, self.done));
                } else {
                    return None;
                }
            }
        }
        None
    }
}
