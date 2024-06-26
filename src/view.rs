use stam::*;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

use std::str::Chars;

use crate::query::textselection_from_queryresult;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Determines whether to display a Tag when highlighting annotations,
/// and what information to show in it.
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

#[derive(Clone, Debug)]
/// Represent a highlight action, represented by a query and a tag to show how to visualize it.
pub struct Highlight<'a> {
    tag: Tag<'a>,
    style: Option<String>,
    query: Option<Query<'a>>,
    label: Option<&'a str>,
    hide: bool, //hide the highlight?
}

impl<'a> Default for Highlight<'a> {
    fn default() -> Self {
        Self {
            label: None,
            query: None,
            tag: Tag::None,
            style: None,
            hide: false,
        }
    }
}

impl<'a> Highlight<'a> {
    /// Create a highlight by parsing a query from string
    pub fn parse_query(
        mut query: &'a str,
        store: &'a AnnotationStore,
        seqnr: usize,
    ) -> Result<Self, String> {
        let mut attribs: Vec<(&str, Option<&str>)> = Vec::new();
        while query.starts_with("@") {
            let pos = query
                .find(' ')
                .ok_or("query syntax error: expected space after @ATTRIBUTE")?;
            let attribname = &query[0..pos];
            if let Some(pos2) = attribname.find('=') {
                let attribvalue = &attribname[pos2 + 1..];
                attribs.push((&attribname[0..pos2], Some(attribvalue)));
            } else {
                attribs.push((attribname, None));
            }
            query = &query[pos + 1..query.len()];
        }
        let (query, _) = stam::Query::parse(query).map_err(|err| err.to_string())?;

        let mut tag = Tag::None;
        let mut style = None;
        let mut hide = false;

        //prepared for a future in which we may have multiple attribs
        for (attribname, attribvalue) in attribs.iter() {
            match *attribname {
                "@KEYTAG" | "@KEY" => {
                    let n = attribvalue
                        .map(|x| x.parse::<usize>().unwrap_or_else(|_|{
                            eprintln!("Warning: Query has @KEYTAG={} but '{}' is not a number, ignoring...", x, x);
                            1}))
                        .unwrap_or(1) - 1;
                    let key = get_key_from_query(&query, n, store);
                    if let Some(key) = key.as_ref() {
                        tag = Tag::Key(key.clone())
                    } else {
                        eprintln!("Warning: Query has @KEYTAG attribute but no key was found in query constraints of query {}, ignoring...", seqnr);
                    }
                }
                "@KEYVALUETAG" | "@KEYVALUE" => {
                    let n = attribvalue
                        .map(|x| x.parse::<usize>().unwrap_or_else(|_|{
                            eprintln!("Warning: Query has @KEYVALUETAG={} but '{}' is not a number, ignoring...", x, x);
                            1}))
                        .unwrap_or(1) - 1;
                    let key = get_key_from_query(&query, n, store);
                    if let Some(key) = key.as_ref() {
                        tag = Tag::KeyAndValue(key.clone())
                    } else {
                        eprintln!("Warning: Query has @KEYVALUETAG attribute but no key was found in query constraints of query {}, ignoring...", seqnr);
                    }
                }
                "@VALUETAG" | "@VALUE" => {
                    let n = attribvalue
                        .map(|x| x.parse::<usize>().unwrap_or_else(|_|{
                            eprintln!("Warning: Query has @VALUETAG={} but '{}' is not a number, ignoring...", x, x);
                            1}))
                        .unwrap_or(1) - 1;
                    let key = get_key_from_query(&query, n, store);
                    if let Some(key) = key.as_ref() {
                        tag = Tag::Value(key.clone())
                    } else {
                        eprintln!("Warning: Query has @VALUETAG attribute but no key was found in query constraints of query {}, ignoring...", seqnr);
                    }
                }
                "@IDTAG" | "@ID" => tag = Tag::Id,
                "@STYLE" | "@CLASS" => style = attribvalue.map(|s| s.to_owned()),
                "@HIDE" | "@HIDDEN" => hide = true,
                x => eprintln!("Warning: Unknown attribute ignored: {}", x),
            }
        }

        Ok(Highlight {
            tag,
            style,
            query: Some(query),
            label: None,
            hide,
        })
    }

    pub fn with_tag(mut self, tag: Tag<'a>) -> Self {
        self.tag = tag;
        self
    }

    pub fn with_query(mut self, query: Query<'a>) -> Self {
        self.query = Some(query);
        self
    }

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

/// Holds all information necessary to visualize annotations as HTML.
/// The writer can be run (= output HTML) via the [`Display`] trait.
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
    /// Output annotations and data in a `<script>` block (javascript)
    output_data: bool,
    /// html header
    header: Option<&'a str>,
    /// html footer
    footer: Option<&'a str>,
    /// Output legend?
    legend: bool,
    /// Output titles (identifiers) for the primary selection?
    titles: bool,
    /// Use javascript for interactive elements
    interactive: bool,
    /// Auto-collapse all tags on document load
    autocollapse: bool,
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
    margin-right: 10%;
    line-height: 1.5em;
    max-width: 1200px;
    margin-left: auto;
    margin-right: auto;
}
:root {
    --hi1: #00aa00; /* green */
    --hi2: #aa0000; /* red */
    --hi3: #0000aa; /* blue */
    --hi4: #aaaa00; /* yellow */
    --hi5: #00aaaa; /* ayan */
    --hi6: #aa00aa; /* magenta */
    --hiX: #666666; /* gray */
}
body {
    background: #b7c8c7;
}
.a { /* annotation */
    /* background: #dedede;  light gray */
    vertical-align: bottom;
}
label {
    font-weight: bold;
    display: inline-block;
    margin-top: 5px;
}
label em {
    color: white;
    display: inline-block;
    font-size: 70%;
    padding-left: 5px;
    padding-right: 5px;
    vertical-align: bottom;
}
/* highlights for labels/tags */
label.tag1 {
    background: var(--hi1);
}
label.tag2 {
    background: var(--hi2);
}
label.tag3 {
    background: var(--hi3);
}
label.tag4 {
    background: var(--hi4);
}
label.tag5 {
    background: var(--hi5);
}
label.tag6 {
    background: var(--hi6);
}
label.tag7, label.tag8, label.tag9, label.tag10, label.tag11, label.tag12, label.tag13, label.tag14, label.tag15, label.tag16 {
    background: var(--hiX);
}
span.l1, span.l2, span.l3, span.l4, span.l5, span.l6, span.l7, span.l8, span.l9, span.l10, span.l11, span.l12, span.l13, span.l14 {
    display: inline-block;
    border-bottom: 3px solid white;
}
.hi1 span.l1 {
    border-bottom: 3px solid var(--hi1);
}
.hi2 span.l2 {
    border-bottom: 3px solid var(--hi2);
}
.hi3 span.l3 {
    border-bottom: 3px solid var(--hi3);
}
.hi4 span.l4 {
    border-bottom: 3px solid var(--hi4);
}
.hi5 span.l5 {
    border-bottom: 3px solid var(--hi5);
}
.hi6 span.l6 {
    border-bottom: 3px solid var(--hi6);
    }
.hi7 span.l7 {
    border-bottom: 3px solid var(--hiX);
}

div#legend {
    background: white;
    color: black;
    width: 40%;
    min-width: 320px;
    margin-left: auto;
    margin-right: auto;
    font-family: sans-serif;
    padding: 5px;
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
    font-weight: bold;
    min-height: 15px;
}
div#legend li:hover {
    font-weight: bold;
}
div#legend li.hidetags {
    text-decoration: none;
    font-style: normal;
    color: #333;
}
div#legend span.hi1 {
    background: var(--hi1);
}
div#legend span.hi2 {
    background: var(--hi2);
}
div#legend span.hi3 {
    background: var(--hi3);
}
div#legend span.hi4 {
    background: var(--hi4);
}
div#legend span.hi5 {
    background: var(--hi5);
}
div#legend span.hi6 {
    background: var(--hi6);
}
div#legend span.hi7 {
    background: var(--hi7);
}
div#legend li {
    cursor: pointer;
    text-decoration: underline;
    font-style: italic;
}
body>h2 {
    color: black;
    font-size: 1.1em;
    font-family: sans-serif;
}
label.h em {
    display: none;
}
span:hover + label.h em {
    position: absolute;
    display: block;
    padding: 2px;
    background: black;
    color: white;
}
label.h em, {
    font-weight: bold;
}
span:hover + label + label.h em {
    position: absolute;
    margin-top: 20px;
    display: block;
    padding: 2px;
    background: black;
}
span:hover + label + label + label.h em {
    position: absolute;
    margin-top: 40px;
    display: block;
    padding: 2px;
    background: black;
}
/* generic style classes */
.italic, .italics { font-style: italic; }
.bold { font-weight: bold; }
.normal { font-weight: normal; font-style: normal; }
.red { color: #ff0000; }
.green { color: #00ff00; }
.blue { color: #0000ff; }
.yellow { color: #ffff00; }
.super, .small { vertical-align: top; font-size: 60%; };
    </style>
</head>
<body>
";

const HTML_SCRIPT: &str = r###"<script>
document.addEventListener('DOMContentLoaded', function() {
    for (let i = 1; i <= 8; i++) {
        let e = document.getElementById("legend" + i);
        if (e) {
            e.addEventListener('click', () => {
                if (e.classList.contains("hidetags")) {
                    document.querySelectorAll('label.tag' + i).forEach((tag,i) => { tag.classList.remove("h")} );
                    e.classList.remove("hidetags");
                } else {
                    document.querySelectorAll('label.tag' + i).forEach((tag,i) => { tag.classList.add("h")} );
                    e.classList.add("hidetags");
                }
            });
            if (autocollapse) {
                e.click();
            }
        }
    }
});
</script>"###;

const HTML_FOOTER: &str = "
</body></html>";

impl<'a> HtmlWriter<'a> {
    /// Instantiates an HtmlWriter, uses a builder pattern via the ``with*()`` methods
    /// to assign data.
    pub fn new(store: &'a AnnotationStore, selectionquery: Query<'a>) -> Self {
        Self {
            store,
            selectionquery,
            selectionvar: None,
            highlights: Vec::new(),
            output_annotation_ids: false,
            output_data_ids: false,
            output_key_ids: false,
            output_offset: true,
            output_data: false,
            prune: false,
            header: Some(HTML_HEADER),
            footer: Some(HTML_FOOTER),
            legend: true,
            titles: true,
            interactive: true,
            autocollapse: true,
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
    pub fn with_interactive(mut self, value: bool) -> Self {
        self.interactive = value;
        self
    }
    pub fn with_autocollapse(mut self, value: bool) -> Self {
        self.autocollapse = value;
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
    n: usize,
    store: &'a AnnotationStore,
) -> Option<ResultItem<'a, DataKey>> {
    if let Some(constraint) = query
        .iter()
        .filter(|c| matches!(c, Constraint::DataKey { .. } | Constraint::KeyValue { .. }))
        .nth(n)
    {
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
    if let Some(key) = get_key_from_query(query, 0, store) {
        //TODO: translate to queries now highlights are no longer supported
        highlights.push(Highlight::default().with_tag(Tag::KeyAndValue(key)))
    } else if let Some(subquery) = query.subquery() {
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
            // write the HTML header
            write!(f, "{}", header)?;
        }
        if self.interactive {
            // in interactive mode, we can '(un)collapse' the tags using a toggle in the legend
            write!(
                f,
                "<script>autocollapse = {};</script>",
                if self.autocollapse { "true" } else { "false" }
            )?;
            write!(f, "{}", HTML_SCRIPT)?;
        }
        // output the STAMQL queries as a comment, just for reference
        write!(
            f,
            "<!-- Selection Query:\n\n{}\n\n-->\n",
            self.selectionquery
                .to_string()
                .unwrap_or_else(|err| format!("{}", err))
        )?;

        // pre-assign class names and layer opening tags so we can borrow later
        let mut classnames: Vec<String> = Vec::with_capacity(self.highlights.len());
        let mut layertags: Vec<String> = Vec::with_capacity(self.highlights.len());
        for (i, highlight) in self.highlights.iter().enumerate() {
            layertags.push(format!("<span class=\"l{}\">", i + 1));
            classnames.push(format!("hi{}", i + 1));

            // and output the highlight queries as comments, just for reference
            if let Some(query) = highlight.query.as_ref() {
                write!(
                    f,
                    "<!-- Highlight Query #{}:\n\n{}\n\n-->\n",
                    i + 1,
                    query.to_string().unwrap_or_else(|err| format!("{}", err))
                )?;
            }
        }

        // output the legend
        if self.legend && self.highlights.iter().any(|hl| hl.query.is_some()) {
            write!(f, "<div id=\"legend\" title=\"Click the items in this legend to toggle visibility of tags (if any)\"><ul>")?;
            for (i, highlight) in self.highlights.iter().enumerate() {
                if !highlight.hide {
                    if let Some(hlq) = highlight.query.as_ref() {
                        write!(
                            f,
                            "<li id=\"legend{}\"{}><span class=\"hi{}\"></span> {}</li>",
                            i + 1,
                            if self.interactive {
                                " title=\"Click to toggle visibility of tags (if any)\""
                            } else {
                                ""
                            },
                            i + 1,
                            hlq.name().unwrap_or("(untitled)").replace("_", " ")
                        )?;
                    }
                }
            }
            write!(f, "</ul></div>")?;
        }

        //run the selection query (the primary query), bail out on error
        let results = self.store.query(self.selectionquery.clone()).map_err(|e| {
            eprintln!("{}", e);
            std::fmt::Error
        })?;

        // iterate over the selection query results
        let names = results.names();
        let mut prevresult = None;

        // pre-allocate buffers that we will reuse
        let mut openingtags = String::new();
        let mut pendingnewlines: String = String::new();
        let mut classes: Vec<&str> = vec![];
        // This buffer will hold all annotations that apply at a certain moment
        // we dynamically add and remove from this as we iterate over all segments
        let mut span_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
        let mut close_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();

        // holds zerowidth annotations
        let mut zerowidth_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();

        for (resultnr, selectionresult) in results.enumerate() {
            // obtain the text selection from the query result
            match textselection_from_queryresult(&selectionresult, self.selectionvar, &names) {
                Err(msg) => return self.output_error(f, msg),
                Ok((resulttextselection, whole_resource, id)) => {
                    if prevresult.as_ref() == Some(&resulttextselection) {
                        //prevent duplicates (especially relevant when --use is set)
                        continue;
                    }
                    prevresult = Some(resulttextselection.clone());

                    // write title and per-result container span (either for a full resource or any textselection)
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

                    // Process all highlight queries and store the resulting annotations handles
                    // so we can later associate the appropriate rendering for the highlight
                    // when an annotation corresponds to one
                    highlights_results = get_highlights_results(
                        &self.selectionquery,
                        &selectionresult,
                        self.selectionvar,
                        self.store,
                        &self.highlights,
                        &names,
                        highlights_results,
                    );

                    // Clear buffer: This buffer will hold all annotations that apply at a certain moment.
                    // We dynamically add and remove from this as we iterate over all segments.
                    span_annotations.clear();

                    let resource = resulttextselection.resource();

                    // Loop over all segments (non-overlapping textselections) in the current result
                    for segment in resulttextselection.segmentation() {
                        zerowidth_annotations.clear();
                        // Gather position info for the begin point of our segment
                        if let Some(beginpositionitem) = resource.as_ref().position(segment.begin())
                        {
                            // find and add annotations that begin with this segment
                            // and do not end after our main selection
                            if segment.end() <= resulttextselection.end() {
                                for (_, textselectionhandle) in beginpositionitem.iter_begin2end() {
                                    let textselection = resource
                                        .as_ref()
                                        .get(*textselectionhandle)
                                        .unwrap()
                                        .as_resultitem(resource.as_ref(), self.store);
                                    span_annotations.extend(
                                        textselection
                                            .annotations()
                                            .inspect(|a| {
                                                // are there any zero-width annotations at the end of the segment?
                                                // collect them they will get special treatment later
                                                if let Some(ts) = a.textselections().next() {
                                                    if ts.begin() == ts.end()
                                                        && ts.end() == textselection.end()
                                                    {
                                                        zerowidth_annotations.insert(a.handle());
                                                    }
                                                }
                                            })
                                            .map(|a| a.handle()),
                                    );
                                    if self.output_data {
                                        //all_annotations.extend(new_span_annotations.iter());
                                    }
                                }
                            }
                        }

                        if self.prune {
                            // prune everything that is not highlighted
                            span_annotations.retain(|a| {
                                for highlights_annotations in highlights_results.iter() {
                                    if highlights_annotations.contains(a) {
                                        return true;
                                    }
                                }
                                false
                            })
                        }

                        if !span_annotations.is_empty()
                            && segment.end() <= resulttextselection.end()
                        {
                            // output the opening <span> layer tags for the current segment
                            // this covers all the annotations we are spanning
                            // not just annotations that start here
                            classes.clear();
                            classes.push("a");
                            for (j, (highlight, highlights_annotations)) in self
                                .highlights
                                .iter()
                                .zip(highlights_results.iter())
                                .enumerate()
                            {
                                if span_annotations
                                    .intersection(&highlights_annotations)
                                    .filter(|a| !zerowidth_annotations.contains(a))
                                    .next()
                                    .is_some()
                                {
                                    if !highlight.hide {
                                        classes.push(&classnames[j]);
                                    }
                                    if let Some(style) = &self.highlights[j].style {
                                        classes.push(style);
                                    }
                                }
                            }
                            openingtags.clear(); //this is a buffer that may be referenced later (for newline processing), start it anew
                            openingtags += "<span";
                            if !classes.is_empty() {
                                openingtags += format!(" class=\"{}\"", classes.join(" ")).as_str();
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
                                                .unwrap_or_else(|| annotation.temp_id().unwrap())
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" "),
                                )
                                .as_str();
                            }

                            // buffer is incomplete but we output already
                            write!(f, "{}", openingtags.as_str())?;
                            if self.output_offset {
                                //we don't want this in the openingtags buffer because it'd be behind if we reuse the opening tags later
                                write!(f, " data-offset=\"{}\"", segment.end())?;
                            }
                            openingtags += ">";
                            write!(f, ">")?;
                            // output all the <span> layers
                            for (l, highlight) in self.highlights.iter().enumerate() {
                                if !highlight.hide {
                                    openingtags += &layertags[l]; //<span class="l$i">
                                    write!(f, "{}", &layertags[l])?;
                                }
                            }
                        } //end processing opening tags for current segment

                        let mut needclosure = true; //close </span> layers?
                        let text = segment.text();

                        // Linebreaks require special handling in rendering, we can't nest
                        // them in the various <span> layers we have but have to pull them out
                        // to the top level. Even if they are in the middle of some text!
                        for (subtext, texttype, done) in LinebreakIter::new(text) {
                            match texttype {
                                BufferType::Text => {
                                    write!(f, "{}", html_escape::encode_text(subtext))?;
                                }
                                BufferType::Whitespace => {
                                    if !span_annotations.is_empty() {
                                        for highlight in self.highlights.iter() {
                                            if !highlight.hide {
                                                write!(f, "</span>")?;
                                            }
                                        }
                                        write!(f, "</span>")?;
                                        write!(f, "{}", openingtags)?;
                                    }
                                    write!(
                                        f,
                                        "{}",
                                        html_escape::encode_text(subtext)
                                            .replace(" ", "&ensp;")
                                            .replace('\t', "&nbsp;&nbsp;&nbsp;&nbsp;")
                                            .as_str()
                                    )?;
                                }
                                BufferType::NewLines => {
                                    if !span_annotations.is_empty() {
                                        for highlight in self.highlights.iter() {
                                            if !highlight.hide {
                                                write!(f, "</span>")?;
                                            }
                                        }
                                        write!(f, "</span>")?;
                                        if !done {
                                            write!(
                                                f,
                                                "{}",
                                                subtext.replace("\n", "<br/>").as_str()
                                            )?;
                                            //open spans again for the next subtext
                                            write!(f, "{}", openingtags)?;
                                        } else {
                                            // we already handled the </span> closure here, prevent doing it again later
                                            needclosure = false;
                                            //set pending newlines, we don't output immediately because there might be a tag to output first
                                            pendingnewlines = subtext.replace("\n", "<br/>");
                                        }
                                    }
                                }
                                BufferType::None => {}
                            }
                        }

                        // Close </span> layers for this segment (if not already done during newline handling)
                        if !span_annotations.is_empty() && needclosure {
                            for highlight in self.highlights.iter() {
                                if !highlight.hide {
                                    write!(f, "</span>")?;
                                }
                            }
                            write!(f, "</span>")?;
                        }

                        // Gather position info for the end point of our segment
                        if let Some(endpositionitem) = resource.as_ref().position(segment.end()) {
                            classes.clear();

                            // Identify which annotations amongst the ones we are spanning are
                            // annotations that we want to highlight, populate the list of CSS classes.
                            for (j, (highlight, highlights_annotations)) in self
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
                                    if !highlight.hide {
                                        classes.push(&classnames[j]);
                                    }
                                    if let Some(style) = &self.highlights[j].style {
                                        classes.push(style.as_str());
                                    }
                                }
                            }

                            // Find all textselections that end at this position
                            close_annotations.clear(); //clear buffer
                            close_annotations.extend(zerowidth_annotations.iter()); //add zero-width annotations
                            for (_, textselectionhandle) in endpositionitem.iter_end2begin() {
                                let textselection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .unwrap()
                                    .as_resultitem(resource.as_ref(), self.store);
                                // Gather annotation handles for all annotations we need to close at this position
                                close_annotations
                                    .extend(textselection.annotations().map(|a| a.handle()));
                            }

                            // Identify which annotations amongst the ones we are spanning
                            // are being closed. Remove them from the span list and output tags if needed.
                            span_annotations.retain(|a| {
                                if close_annotations.contains(a) {
                                    for (j, (highlights, highlights_results)) in self
                                        .highlights
                                        .iter()
                                        .zip(highlights_results.iter())
                                        .enumerate()
                                    {
                                        if highlights_results.contains(a) {
                                            if let Some(annotation) = self.store.annotation(*a) {
                                                // Get the appropriate tag representation
                                                // and output the tag for this highlight
                                                let tag = highlights.get_tag(annotation);
                                                if !tag.is_empty() {
                                                    if zerowidth_annotations.contains(a) {
                                                        write!(
                                                            f,
                                                            "<label class=\"zw tag{} {}\">",
                                                            j + 1,
                                                            classes.join(" ")
                                                        )
                                                        .ok();
                                                    } else {
                                                        write!(
                                                            f,
                                                            "<label class=\"tag{} {}\">",
                                                            j + 1,
                                                            classes.join(" ")
                                                        )
                                                        .ok();
                                                    }
                                                    for (l, highlight) in
                                                        self.highlights.iter().enumerate()
                                                    {
                                                        if !highlight.hide {
                                                            write!(
                                                                f,
                                                                "{}",
                                                                &layertags[l], //<span class="l$i">
                                                            )
                                                            .ok();
                                                        }
                                                    }
                                                    write!(f, "<em>{}</em>", tag,).ok();
                                                    for highlight in self.highlights.iter() {
                                                        if !highlight.hide {
                                                            write!(f, "</span>").ok();
                                                        }
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

                            if !pendingnewlines.is_empty() {
                                write!(f, "{}", pendingnewlines)?;
                                pendingnewlines.clear();
                            }
                        }
                    }
                    writeln!(f, "\n</div>")?;
                    if self.output_data {
                        //MAYBE TODO: implement later?
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

/// Holds all information necessary to visualize annotations as text with ANSI escape codes, for terminal output
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
    /// Instantiates an AnsiWriter, uses a builder pattern via the ``with*()`` methods
    /// to assign data.
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

    fn writeansicol<W: std::io::Write>(
        &self,
        writer: &mut W,
        i: usize,
        s: &str,
    ) -> Result<(), std::io::Error> {
        let color = if i > 6 { 30 } else { 30 + i };
        writer.write(b"\x1b[")?;
        writer.write(&format!("{}", color).into_bytes())?;
        writer.write(b"m")?;
        writer.flush()?;
        write!(writer, "{}", s)?;
        writer.write(b"\x1b[m")?;
        writer.flush()
    }

    fn writeansicol_bold<W: std::io::Write>(
        &self,
        writer: &mut W,
        i: usize,
        s: &str,
    ) -> Result<(), std::io::Error> {
        let color = if i > 6 { 30 } else { 30 + i };
        writer.write(b"\x1b[")?;
        writer.write(&format!("{}", color).into_bytes())?;
        writer.write(b";1m")?;
        writer.flush()?;
        write!(writer, "{}", s)?;
        writer.write(b"\x1b[m")?;
        writer.flush()
    }

    fn writeheader<W: std::io::Write>(
        &self,
        writer: &mut W,
        s: &str,
    ) -> Result<(), std::io::Error> {
        writer.write(b"\x1b[37;1m")?;
        writer.flush()?;
        write!(writer, "{}", s)?;
        writer.write(b"\x1b[m")?;
        writer.flush()
    }

    /// Write output
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        let mut highlights_results: Vec<BTreeSet<AnnotationHandle>> = Vec::new();
        for _ in 0..self.highlights.len() {
            highlights_results.push(BTreeSet::new());
        }
        if self.legend && self.highlights.iter().any(|hl| hl.query.is_some()) {
            writeln!(writer, "Legend:")?;
            for (i, highlight) in self.highlights.iter().enumerate() {
                if let Some(hlq) = highlight.query.as_ref() {
                    let s = format!(
                        "       {}. {}\n",
                        i + 1,
                        hlq.name().unwrap_or("(untitled)").replace("_", " ")
                    );
                    self.writeansicol_bold(writer, i + 1, s.as_str())?;
                }
            }
            writeln!(writer)?;
        }

        let results = self
            .store
            .query(self.selectionquery.clone())
            .expect("query failed");

        //buffers
        let mut span_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
        let mut new_span_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
        let mut close_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
        let mut zerowidth_annotations: BTreeSet<AnnotationHandle> = BTreeSet::new();
        let mut endnewlines = String::new();

        let names = results.names();
        let mut prevresult = None;
        for (resultnr, selectionresult) in results.enumerate() {
            match textselection_from_queryresult(&selectionresult, self.selectionvar, &names) {
                Err(msg) => return Ok(self.output_error(msg)),
                Ok((resulttextselection, _, id)) => {
                    if prevresult.as_ref() == Some(&resulttextselection) {
                        //prevent duplicates (especially relevant when --use is set)
                        continue;
                    }
                    prevresult = Some(resulttextselection.clone());
                    if self.titles {
                        if let Some(id) = id {
                            let s = format!("----------------------------------- {}. {} -----------------------------------\n", resultnr + 1, id,);
                            self.writeheader(writer, s.as_str())?;
                        }
                    }
                    highlights_results = get_highlights_results(
                        &self.selectionquery,
                        &selectionresult,
                        self.selectionvar,
                        self.store,
                        &self.highlights,
                        &names,
                        highlights_results,
                    );
                    let resource = resulttextselection.resource();
                    span_annotations.clear();
                    for segment in resulttextselection.segmentation() {
                        new_span_annotations.clear();
                        if let Some(beginpositionitem) = resource.as_ref().position(segment.begin())
                        {
                            //find and add annotations that begin here
                            for (_, textselectionhandle) in beginpositionitem.iter_begin2end() {
                                let textselection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .unwrap()
                                    .as_resultitem(resource.as_ref(), self.store);
                                new_span_annotations.extend(
                                    textselection
                                        .annotations()
                                        .inspect(|a| {
                                            // are there any zero-width annotations at the end of the segment?
                                            // collect them they will get special treatment later
                                            if let Some(ts) = a.textselections().next() {
                                                if ts.begin() == ts.end()
                                                    && ts.end() == textselection.end()
                                                {
                                                    zerowidth_annotations.insert(a.handle());
                                                }
                                            }
                                        })
                                        .map(|a| a.handle()),
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

                        if !new_span_annotations.is_empty()
                            && segment.end() <= resulttextselection.end()
                        {
                            //output the opening tags
                            for (j, highlights_annotations) in highlights_results.iter().enumerate()
                            {
                                if new_span_annotations
                                    .intersection(&highlights_annotations)
                                    .filter(|a| !zerowidth_annotations.contains(a))
                                    .next()
                                    .is_some()
                                {
                                    self.writeansicol_bold(writer, j + 1, "[")?;
                                }
                            }
                            //the text is outputted alongside the closing tag
                        }
                        span_annotations.extend(new_span_annotations.iter());

                        //output text
                        endnewlines.clear();
                        if segment.text().ends_with("\n") {
                            for c in segment.text().chars().rev() {
                                if c == '\n' {
                                    endnewlines.push(c);
                                } else {
                                    break;
                                }
                            }
                            write!(writer, "{}", segment.text().trim_end_matches('\n'))?;
                        } else {
                            write!(writer, "{}", segment.text())?;
                        }

                        if let Some(endpositionitem) = resource.as_ref().position(segment.end()) {
                            for (_, textselectionhandle) in endpositionitem.iter_end2begin() {
                                let textselection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .unwrap()
                                    .as_resultitem(resource.as_ref(), self.store);
                                close_annotations.clear();
                                close_annotations.extend(zerowidth_annotations.iter()); //add zero-width annotations
                                close_annotations
                                    .extend(textselection.annotations().map(|a| a.handle()));
                                span_annotations.retain(|a| {
                                    if close_annotations.contains(a) {
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
                                                        if zerowidth_annotations.contains(a) {
                                                            self.writeansicol_bold(
                                                                writer,
                                                                j + 1,
                                                                "[",
                                                            )
                                                            .unwrap();
                                                            self.writeansicol(
                                                                writer,
                                                                j + 1,
                                                                format!("{}", &tag).as_str(),
                                                            )
                                                            .unwrap();
                                                        } else {
                                                            self.writeansicol(
                                                                writer,
                                                                j + 1,
                                                                format!("|{}", &tag).as_str(),
                                                            )
                                                            .unwrap();
                                                        }
                                                    }
                                                    self.writeansicol_bold(writer, j + 1, "]")
                                                        .unwrap();
                                                }
                                            }
                                        }
                                        false
                                    } else {
                                        true
                                    }
                                });
                            }
                        }

                        if !endnewlines.is_empty() {
                            write!(writer, "{}", endnewlines)?;
                        }
                    }
                }
            }
            writeln!(writer)?;
        }
        Ok(())
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
                        hlquery.bind_annotationvar(varname, x);
                    }
                    QueryResultItem::TextSelection(x) => {
                        hlquery.bind_textvar(varname, x);
                    }
                    QueryResultItem::AnnotationData(x) => {
                        hlquery.bind_datavar(varname, x);
                    }
                    QueryResultItem::TextResource(x) => {
                        hlquery.bind_resourcevar(varname, x);
                    }
                    QueryResultItem::AnnotationDataSet(x) => {
                        hlquery.bind_datasetvar(varname, x);
                    }
                    QueryResultItem::DataKey(x) => {
                        hlquery.bind_keyvar(varname, x);
                    }
                }
            } else {
                eprintln!(
                    "WARNING: unable to retrieve variable {} from main query",
                    varname
                );
            }
            //process result of highlight query and extra annotations
            for results in store.query(hlquery).expect("query failed") {
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
        }
    }
    highlights_results
}

#[derive(Copy, PartialEq, Clone, Debug)]
enum BufferType {
    None,
    /// Buffer contains only newlines
    NewLines,
    /// Buffer contains only whitespace
    Whitespace,
    /// Buffer contains text without newlines
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
                    || ((c.is_whitespace() && c != '\n')
                        && self.buffertype == BufferType::Whitespace)
                    || (c != '\n' && !c.is_whitespace() && self.buffertype == BufferType::Text)
                {
                    //same type as buffer, carry on
                    self.curbytepos += c.len_utf8();
                } else {
                    //switching buffers, yield result
                    let resultbuffertype = self.buffertype;
                    if c == '\n' {
                        self.buffertype = BufferType::NewLines;
                    } else if c.is_whitespace() {
                        self.buffertype = BufferType::Whitespace;
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
