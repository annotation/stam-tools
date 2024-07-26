use stam::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
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
    /// The variable this highlight is bound to
    varname: &'a str,
    label: Option<&'a str>,
    hide: bool, //hide the highlight? useful when you only want to assign a custom style and nothing else
}

impl<'a> Default for Highlight<'a> {
    fn default() -> Self {
        Self {
            label: None,
            varname: "",
            tag: Tag::None,
            style: None,
            hide: false,
        }
    }
}

impl<'a> Highlight<'a> {
    pub fn with_tag(mut self, tag: Tag<'a>) -> Self {
        self.tag = tag;
        self
    }

    pub fn new(var: &'a str) -> Self {
        Self {
            varname: var,
            ..Self::default()
        }
    }

    pub fn with_label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Serializes the tag to string, given an annotation
    pub fn get_tag(&self, annotation: &ResultItem<'a, Annotation>) -> Cow<'a, str> {
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
    query: Query<'a>,
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
    pub fn new(
        store: &'a AnnotationStore,
        query: Query<'a>,
        selectionvar: Option<&'a str>,
    ) -> Result<Self, String> {
        Ok(Self {
            store,
            highlights: highlights_from_query(&query, store, selectionvar)?,
            query,
            selectionvar,
            output_annotation_ids: false,
            output_data_ids: false,
            output_key_ids: false,
            output_offset: true,
            output_data: false,
            header: Some(HTML_HEADER),
            footer: Some(HTML_FOOTER),
            legend: true,
            titles: true,
            interactive: true,
            autocollapse: true,
        })
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
}

fn get_key_from_constraint<'a>(
    constraint: &Constraint<'a>,
    store: &'a AnnotationStore,
) -> Option<ResultItem<'a, DataKey>> {
    if let Constraint::DataKey { set, key, .. } | Constraint::KeyValue { set, key, .. } = constraint
    {
        if let Some(key) = store.key(*set, *key) {
            return Some(key);
        }
    }
    None
}

/// Parse highlight information from the query's attributes
fn highlights_from_query<'a>(
    query: &Query<'a>,
    store: &'a AnnotationStore,
    selectionvar: Option<&'a str>,
) -> Result<Vec<Highlight<'a>>, String> {
    let mut highlights = Vec::new();
    helper_highlights_from_query(&mut highlights, query, store, selectionvar)?;
    Ok(highlights)
}

fn helper_highlights_from_query<'a>(
    highlights: &mut Vec<Highlight<'a>>,
    query: &Query<'a>,
    store: &'a AnnotationStore,
    selectionvar: Option<&'a str>,
) -> Result<(), String> {
    for subquery in query.subqueries() {
        if let Some(varname) = subquery.name() {
            let mut highlight = Highlight::new(varname);
            for attrib in subquery.attributes() {
                let (attribname, attribvalue) = if let Some(pos) = attrib.find('=') {
                    (&attrib[..pos], &attrib[pos + 1..])
                } else {
                    (*attrib, "")
                };
                match attribname {
                    "@HIDE" => highlight.hide = true,
                    "@IDTAG" | "@ID" => highlight.tag = Tag::Id,
                    "@STYLE" | "@CLASS" => highlight.style = Some(attribvalue.to_string()),
                    "@KEYTAG" | "@VALUETAG" | "@KEYVALUETAG" => {
                        //old-style tags ahead of the whole query (backward compatibility)
                        for constraint in subquery.constraints() {
                            if let Some(key) = get_key_from_constraint(&constraint, store) {
                                match attribname {
                                    "@KEYTAG" => highlight.tag = Tag::Key(key),
                                    "@VALUETAG" => highlight.tag = Tag::Value(key),
                                    "@KEYVALUETAG" => highlight.tag = Tag::Value(key),
                                    _ => unreachable!("invalid tag"),
                                }
                            }
                        }
                    }
                    other => {
                        return Err(format!("Query syntax error - Unknown attribute: {}", other));
                    }
                }
            }
            for (constraint, attributes) in subquery.constraints_with_attributes() {
                for attrib in attributes {
                    /*
                    let (attribname, attribvalue) = if let Some(pos) = attrib.find('=') {
                        (&attrib[..pos], &attrib[pos + 1..])
                    } else {
                        (*attrib, "")
                    };
                    */
                    match *attrib {
                        "@KEYTAG" => {
                            if let Some(key) = get_key_from_constraint(&constraint, store) {
                                highlight.tag = Tag::Key(key);
                            }
                        }
                        "@VALUETAG" => {
                            if let Some(key) = get_key_from_constraint(&constraint, store) {
                                highlight.tag = Tag::Value(key);
                            }
                        }
                        "@KEYVALUETAG" => {
                            if let Some(key) = get_key_from_constraint(&constraint, store) {
                                highlight.tag = Tag::KeyAndValue(key);
                            }
                        }
                        other => {
                            return Err(format!(
                                "Query syntax error - Unknown constraint attribute: {}",
                                other
                            ));
                        }
                    }
                }
            }
            highlights.push(highlight);
        }
        helper_highlights_from_query(highlights, subquery, store, selectionvar)?;
    }
    Ok(())
}

pub(crate) struct SelectionWithHighlightsIterator<'a> {
    iter: QueryIter<'a>,
    selectionvar: Option<&'a str>,
    highlights: &'a Vec<Highlight<'a>>,

    //the following are buffers:
    highlight_results: Vec<HighlightResults>,
    previous: Option<ResultTextSelection<'a>>,
    whole_resource: bool,
    id: Option<&'a str>,
}

impl<'a> SelectionWithHighlightsIterator<'a> {
    pub fn new(
        iter: QueryIter<'a>,
        selectionvar: Option<&'a str>,
        highlights: &'a Vec<Highlight<'a>>,
    ) -> Self {
        Self {
            iter,
            selectionvar,
            highlights,
            highlight_results: Self::new_highlight_results(highlights.len()),
            previous: None,
            whole_resource: false,
            id: None,
        }
    }

    fn new_highlight_results(len: usize) -> Vec<HighlightResults> {
        let mut highlight_results: Vec<HighlightResults> = Vec::with_capacity(len);
        for _ in 0..len {
            highlight_results.push(HighlightResults::new());
        }
        highlight_results
    }
}

type HighlightResults = BTreeMap<TextSelection, Option<AnnotationHandle>>;

#[derive(Debug, Clone)]
pub(crate) struct SelectionWithHighlightResult<'a> {
    textselection: ResultTextSelection<'a>,
    highlights: Vec<HighlightResults>,
    whole_resource: bool,
    id: Option<&'a str>,
}

impl<'a> Iterator for SelectionWithHighlightsIterator<'a> {
    type Item = Result<SelectionWithHighlightResult<'a>, &'a str>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(queryresultitems) = self.iter.next() {
                match textselection_from_queryresult(&queryresultitems, self.selectionvar) {
                    Err(msg) => return Some(Err(msg)),
                    Ok((resulttextselection, whole_resource, id)) => {
                        //resulttextselection may match the same item multiple times with other highlights (subqueries)
                        //if so, we need to aggregate these highlights
                        if self.previous.is_some()
                            && Some(&resulttextselection) != self.previous.as_ref()
                        {
                            //we start a new resultselection, so prepare to return the previous one:
                            let previous_highlights = std::mem::replace(
                                &mut self.highlight_results,
                                Self::new_highlight_results(self.highlights.len()),
                            );

                            //get the previous results
                            let previous_whole_resource = self.whole_resource;
                            let previous_id = self.id;

                            //store the new metadata for next iteration
                            self.whole_resource = whole_resource;
                            self.id = id;
                            //mark highlights in buffer for new results:
                            get_highlights_results(
                                &queryresultitems,
                                &self.highlights,
                                &mut self.highlight_results, //will be appended to
                            );

                            //return the previous one:
                            return Some(Ok(SelectionWithHighlightResult {
                                textselection: std::mem::replace(
                                    &mut self.previous,
                                    Some(resulttextselection),
                                )
                                .unwrap(),
                                highlights: previous_highlights,
                                whole_resource: previous_whole_resource,
                                id: previous_id,
                            }));
                        } else {
                            //buffer metadata
                            self.previous = Some(resulttextselection);
                            self.whole_resource = whole_resource;
                            self.id = id;
                            //same text selection result, mark highlights in buffer:
                            get_highlights_results(
                                &queryresultitems,
                                &self.highlights,
                                &mut self.highlight_results, //will be appended to
                            );
                        }
                    }
                }
            } else if let Some(resulttextselection) = self.previous.take() {
                //don't forget the last item
                let return_highlight_results = std::mem::replace(
                    &mut self.highlight_results,
                    Self::new_highlight_results(self.highlights.len()),
                );
                return Some(Ok(SelectionWithHighlightResult {
                    textselection: resulttextselection,
                    highlights: return_highlight_results,
                    whole_resource: self.whole_resource,
                    id: self.id,
                }));
            } else {
                //iterator done
                return None;
            }
        }
    }
}

impl<'a> Display for HtmlWriter<'a> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
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
        // output the STAMQL query as a comment, just for reference
        write!(
            f,
            "<!-- Query:\n\n{}\n\n-->\n",
            self.query
                .to_string()
                .unwrap_or_else(|err| format!("{}", err))
        )?;

        // pre-assign class names and layer opening tags so we can borrow later
        let mut classnames: Vec<String> = Vec::with_capacity(self.highlights.len());
        let mut layertags: Vec<String> = Vec::with_capacity(self.highlights.len());
        let mut close_annotations: Vec<Vec<ResultItem<Annotation>>> = Vec::new();

        for (i, _highlight) in self.highlights.iter().enumerate() {
            layertags.push(format!("<span class=\"l{}\">", i + 1));
            classnames.push(format!("hi{}", i + 1));
            close_annotations.push(Vec::new());
        }

        // output the legend
        if self.legend && !self.highlights.is_empty() {
            write!(f, "<div id=\"legend\" title=\"Click the items in this legend to toggle visibility of tags (if any)\"><ul>")?;
            for (i, highlight) in self.highlights.iter().enumerate() {
                if !highlight.hide {
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
                        if let Some(label) = highlight.label {
                            label.to_string()
                        } else {
                            highlight.varname.replace("_", " ")
                        }
                    )?;
                }
            }
            write!(f, "</ul></div>")?;
        }

        //run the query, bail out on error (this is lazy, doesn't return yet until we iterate over the results)
        let results = self.store.query(self.query.clone()).map_err(|e| {
            eprintln!("{}", e);
            std::fmt::Error
        })?;

        // pre-allocate buffers that we will reuse
        let mut openingtags = String::new();
        let mut pendingnewlines: String = String::new();
        let mut classes: Vec<&str> = vec![];

        let mut active_highlights: BTreeSet<usize> = BTreeSet::new();
        let mut close_highlights: BTreeSet<usize> = BTreeSet::new();

        for (resultnr, result) in
            SelectionWithHighlightsIterator::new(results, self.selectionvar, &self.highlights)
                .enumerate()
        {
            // obtain the text selection from the query result
            match result {
                Err(msg) => return self.output_error(f, msg),
                Ok(result) => {
                    active_highlights.clear();
                    close_highlights.clear();
                    classes.clear();
                    let resource = result.textselection.resource();

                    // write title and per-result container span (either for a full resource or any textselection)
                    if self.titles {
                        if let Some(id) = result.id {
                            write!(f, "<h2>{}. <span>{}</span></h2>\n", resultnr + 1, id,)?;
                        }
                    }
                    if result.whole_resource {
                        write!(
                            f,
                            "<div class=\"resource\" data-resource=\"{}\">\n",
                            resource.id().unwrap_or("undefined"),
                        )?;
                    } else {
                        write!(
                            f,
                            "<div class=\"textselection\" data-resource=\"{}\" data-begin=\"{}\" data-end=\"{}\">\n",
                            resource.id().unwrap_or("undefined"),
                            result.textselection.begin(),
                            result.textselection.end(),
                        )?;
                    }

                    for segment in result.textselection.segmentation() {
                        // Gather position info for the begin point of our segment
                        if let Some(beginpositionitem) = resource.as_ref().position(segment.begin())
                        {
                            // what highlights start at this segment?
                            for (_, textselectionhandle) in beginpositionitem.iter_begin2end() {
                                let textselection: &TextSelection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .expect("text selection must exist");
                                for (j, highlighted_selections) in
                                    result.highlights.iter().enumerate()
                                {
                                    if highlighted_selections.contains_key(textselection) {
                                        active_highlights.insert(j);
                                    }
                                }
                            }
                        }

                        let text = segment.text();
                        let text_is_whitespace =
                            !segment.text().chars().any(|c| !c.is_whitespace());

                        if !active_highlights.is_empty()
                            && segment.end() <= result.textselection.end()
                        {
                            // output the opening <span> layer tags for the current segment
                            // this covers all the highlighted text selections we are spanning
                            // not just those pertaining to annotations that start here
                            classes.clear();
                            classes.push("a");
                            for j in active_highlights.iter() {
                                if !self.highlights[*j].hide {
                                    classes.push(&classnames[*j]);
                                }
                                if let Some(style) = self.highlights[*j].style.as_ref() {
                                    classes.push(style);
                                }
                            }
                            openingtags.clear(); //this is a buffer that may be referenced later (for newline processing), start it anew
                            openingtags += "<span";
                            if !classes.is_empty() {
                                openingtags += format!(" class=\"{}\"", classes.join(" ")).as_str();
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
                        } else if !text_is_whitespace {
                            //opening if there are no active highlights
                            openingtags.clear();
                            openingtags += "<span>";
                            write!(f, "<span>")?;
                            // output all the <span> layers
                            for (l, highlight) in self.highlights.iter().enumerate() {
                                if !highlight.hide {
                                    openingtags += &layertags[l]; //<span class="l$i">
                                    write!(f, "{}", &layertags[l])?;
                                }
                            }
                        }

                        let mut needclosure = !active_highlights.is_empty() || !text_is_whitespace; //close </span> layers?

                        // Linebreaks require special handling in rendering, we can't nest
                        // them in the various <span> layers we have but have to pull them out
                        // to the top level. Even if they are in the middle of some text!
                        for (subtext, texttype, done) in LinebreakIter::new(text) {
                            match texttype {
                                BufferType::Text => {
                                    write!(f, "{}", html_escape::encode_text(subtext))?;
                                }
                                BufferType::Whitespace => {
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
                                    for highlight in self.highlights.iter() {
                                        if !highlight.hide {
                                            write!(f, "</span>")?;
                                        }
                                    }
                                    write!(f, "</span>")?;
                                    if !done {
                                        write!(f, "{}", subtext.replace("\n", "<br/>").as_str())?;
                                        //open spans again for the next subtext
                                        write!(f, "{}", openingtags)?;
                                    } else {
                                        // we already handled the </span> closure here, prevent doing it again later
                                        needclosure = false;
                                        //set pending newlines, we don't output immediately because there might be a tag to output first
                                        pendingnewlines = subtext.replace("\n", "<br/>");
                                    }
                                }
                                BufferType::None => {}
                            }
                        }

                        // Close </span> layers for this segment (if not already done during newline handling)
                        if needclosure {
                            for highlight in self.highlights.iter() {
                                if !highlight.hide {
                                    write!(f, "</span>")?;
                                }
                            }
                            write!(f, "</span>")?;
                        }

                        // Gather position info for the end point of our segment
                        if let Some(endpositionitem) = resource.as_ref().position(segment.end()) {
                            // what highlights stop at this segment?
                            close_highlights.clear();
                            for annotations in close_annotations.iter_mut() {
                                annotations.clear();
                            }
                            for (_, textselectionhandle) in endpositionitem.iter_end2begin() {
                                let textselection: &TextSelection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .expect("text selection must exist");
                                for (j, highlighted_selections) in
                                    result.highlights.iter().enumerate()
                                {
                                    if let Some(a_handle) =
                                        highlighted_selections.get(textselection)
                                    {
                                        close_highlights.insert(j);
                                        // Identify which highlighted annotations are being closed
                                        if let Some(a_handle) = a_handle {
                                            let annotation = self
                                                .store
                                                .annotation(*a_handle)
                                                .expect("annotation must exist");
                                            //confirm that the annotation really closes here (or at least one of its text selections does if it's non-contingent):
                                            //MAYBE TODO: this may not be performant enough for large MultiSelectors!
                                            if annotation
                                                .textselections()
                                                .any(|ts| ts.end() == segment.end())
                                            {
                                                close_annotations[j].push(annotation);
                                            }
                                        }
                                    }
                                }
                            }
                            classes.clear();
                            for j in active_highlights.iter() {
                                if !self.highlights[*j].hide {
                                    classes.push(&classnames[*j]);
                                }
                                if let Some(style) = self.highlights[*j].style.as_ref() {
                                    classes.push(style);
                                }
                            }

                            // output tags for annotations that close here (if requested)
                            for (j, annotations) in close_annotations.iter().enumerate() {
                                let highlight = &self.highlights[j];
                                for annotation in annotations {
                                    // Get the appropriate tag representation
                                    // and output the tag for this highlight
                                    let tag = highlight.get_tag(&annotation);
                                    if !tag.is_empty() {
                                        //check for zero-width
                                        if annotation
                                            .textselections()
                                            .any(|ts| ts.begin() == ts.end())
                                        //MAYBE TODO: potentially expensive on large MultiSelectors!
                                        {
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
                                        for (l, highlight) in self.highlights.iter().enumerate() {
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

                            if !pendingnewlines.is_empty() {
                                write!(f, "{}", pendingnewlines)?;
                                pendingnewlines.clear();
                            }

                            //process the closing highlights
                            active_highlights.retain(|hl| !close_highlights.contains(hl));
                        }
                    }
                    writeln!(f, "\n</div>")?;
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
    query: Query<'a>,
    selectionvar: Option<&'a str>,
    highlights: Vec<Highlight<'a>>,
    /// Output legend?
    legend: bool,
    /// Output titles (identifiers) for the primary selection?
    titles: bool,
}

impl<'a> AnsiWriter<'a> {
    /// Instantiates an AnsiWriter, uses a builder pattern via the ``with*()`` methods
    /// to assign data.
    pub fn new(
        store: &'a AnnotationStore,
        query: Query<'a>,
        selectionvar: Option<&'a str>,
    ) -> Result<Self, String> {
        Ok(Self {
            store,
            selectionvar: None,
            highlights: highlights_from_query(&query, store, selectionvar)?,
            query,
            legend: true,
            titles: true,
        })
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

    pub fn with_selectionvar(mut self, var: &'a str) -> Self {
        self.selectionvar = Some(var);
        self
    }

    fn output_error(&self, msg: &str) {
        eprintln!("ERROR: {}", msg);
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

    /// Write ANSI output
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        if self.legend && !self.highlights.is_empty() {
            writeln!(writer, "Legend:")?;
            for (i, highlight) in self.highlights.iter().enumerate() {
                if !highlight.hide {
                    let s = format!(
                        "       {}. {}\n",
                        i + 1,
                        highlight.varname.replace("_", " ")
                    );
                    self.writeansicol_bold(writer, i + 1, s.as_str())?;
                }
            }
            writeln!(writer)?;
        }

        let results = self.store.query(self.query.clone()).map_err(|e| {
            eprintln!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "STAM query error")
        })?;

        let mut close_annotations: Vec<Vec<ResultItem<Annotation>>> = Vec::new();
        for _ in 0..self.highlights.len() {
            close_annotations.push(Vec::new());
        }

        let mut pendingnewlines = String::new();

        let mut active_highlights: BTreeSet<usize> = BTreeSet::new();
        let mut close_highlights: BTreeSet<usize> = BTreeSet::new();

        for (resultnr, result) in
            SelectionWithHighlightsIterator::new(results, self.selectionvar, &self.highlights)
                .enumerate()
        {
            // obtain the text selection from the query result
            match result {
                Err(msg) => return Ok(self.output_error(msg)),
                Ok(result) => {
                    active_highlights.clear();
                    let resource = result.textselection.resource();

                    if self.titles {
                        if let Some(id) = result.id {
                            let s = format!("----------------------------------- {}. {} -----------------------------------\n", resultnr + 1, id,);
                            self.writeheader(writer, s.as_str())?;
                        }
                    }

                    for segment in result.textselection.segmentation() {
                        // Gather position info for the begin point of our segment
                        if let Some(beginpositionitem) = resource.as_ref().position(segment.begin())
                        {
                            // what highlights start at this segment?
                            for (_, textselectionhandle) in beginpositionitem.iter_begin2end() {
                                let textselection: &TextSelection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .expect("text selection must exist");
                                for (j, highlighted_selections) in
                                    result.highlights.iter().enumerate()
                                {
                                    if highlighted_selections.contains_key(textselection) {
                                        active_highlights.insert(j);
                                    }
                                }
                            }
                        }

                        if !active_highlights.is_empty()
                            && segment.end() <= result.textselection.end()
                        {
                            //output the opening tags
                            for j in active_highlights.iter().rev() {
                                self.writeansicol_bold(writer, j + 1, "[")?;
                            }
                        }

                        //output text
                        pendingnewlines.clear();
                        if segment.text().ends_with("\n") {
                            for c in segment.text().chars().rev() {
                                if c == '\n' {
                                    pendingnewlines.push(c);
                                } else {
                                    break;
                                }
                            }
                            write!(writer, "{}", segment.text().trim_end_matches('\n'))?;
                        } else {
                            write!(writer, "{}", segment.text())?;
                        }

                        // Gather position info for the end point of our segment
                        if let Some(endpositionitem) = resource.as_ref().position(segment.end()) {
                            // what highlights stop at this segment?
                            close_highlights.clear();
                            for annotations in close_annotations.iter_mut() {
                                annotations.clear();
                            }
                            for (_, textselectionhandle) in endpositionitem.iter_end2begin() {
                                let textselection: &TextSelection = resource
                                    .as_ref()
                                    .get(*textselectionhandle)
                                    .expect("text selection must exist");
                                for (j, highlighted_selections) in
                                    result.highlights.iter().enumerate()
                                {
                                    if let Some(a_handle) =
                                        highlighted_selections.get(textselection)
                                    {
                                        close_highlights.insert(j);
                                        // Identify which highlighted annotations are being closed
                                        if let Some(a_handle) = a_handle {
                                            let annotation = self
                                                .store
                                                .annotation(*a_handle)
                                                .expect("annotation must exist");
                                            //confirm that the annotation really closes here (or at least one of its text selections does if it's non-contingent):
                                            //MAYBE TODO: this may not be performant enough for large MultiSelectors!
                                            if annotation
                                                .textselections()
                                                .any(|ts| ts.end() == segment.end())
                                            {
                                                close_annotations[j].push(annotation);
                                            }
                                        }
                                    }
                                }
                            }

                            // output tags for annotations that close here (if requested)
                            for (j, annotations) in close_annotations.iter().enumerate() {
                                let highlight = &self.highlights[j];
                                for annotation in annotations {
                                    //closing highlight after adding tag if needed
                                    let tag = highlight.get_tag(&annotation);
                                    if !tag.is_empty() {
                                        if annotation
                                            .textselections()
                                            .any(|ts| ts.begin() == ts.end())
                                        {
                                            //MAYBE TODO: potentially expensive on large MultiSelectors!
                                            self.writeansicol_bold(writer, j + 1, "[").unwrap();
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
                                    self.writeansicol_bold(writer, j + 1, "]").unwrap();
                                }
                            }
                        }

                        if !pendingnewlines.is_empty() {
                            write!(writer, "{}", pendingnewlines)?;
                        }

                        //process the closing highlights
                        active_highlights.retain(|hl| !close_highlights.contains(hl));
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
    resultitems: &QueryResultItems<'a>,
    highlights: &[Highlight],
    highlight_results: &mut Vec<HighlightResults>,
) {
    //gather annotations for the textselection under consideration
    for (j, highlight) in highlights.iter().enumerate() {
        if highlight_results.len() <= j {
            highlight_results.push(HighlightResults::new());
        }

        if let Some(highlight_results) = highlight_results.get_mut(j) {
            if let Ok(result) = resultitems.get_by_name(highlight.varname) {
                match result {
                    &QueryResultItem::Annotation(ref annotation) => {
                        for ts in annotation.textselections() {
                            highlight_results.insert(ts.inner().clone(), Some(annotation.handle()));
                        }
                    }
                    &QueryResultItem::TextSelection(ref ts) => {
                        highlight_results.insert(ts.inner().clone(), None);
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
    }
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
