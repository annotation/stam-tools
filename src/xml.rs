use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt::Display;
use std::fs::read_to_string;
use std::path::Path;

use roxmltree::{Attribute, Document, Node, NodeId, ParsingOptions};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use stam::*;
use toml;
use upon::Engine;

const NS_XML: &str = "http://www.w3.org/XML/1998/namespace";

fn default_set() -> String {
    "urn:stam-fromxml".into()
}

#[derive(Deserialize)]
/// Holds the configuration for mapping a specific XML format to STAM
pub struct XmlConversionConfig {
    #[serde(default)]
    /// Holds configurations for mapping specific XML elements to STAM, evaluated in reverse-order, so put more generic rules before specific ones
    elements: Vec<XmlElementConfig>,

    #[serde(default)]
    /// Maps XML prefixes to namespace
    namespaces: HashMap<String, String>,

    #[serde(default = "XmlWhitespaceHandling::collapse")]
    /// Default whitespace handling
    whitespace: XmlWhitespaceHandling,

    #[serde(default)]
    /// Sets additional context variables that can be used in templates
    context: HashMap<String, String>,

    #[serde(default)]
    /// Inject a DTD (for XML entity resolution)
    inject_dtd: Option<String>,

    #[serde(default = "default_set")]
    default_set: String,

    #[serde(default)]
    /// A prefix to assign when setting annotation IDs, within this string you can use the special variable `{resource}` to use the resource ID.
    id_prefix: Option<String>,

    #[serde(skip_deserializing)]
    debug: bool,
}

impl XmlConversionConfig {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            namespaces: HashMap::new(),
            context: HashMap::new(),
            whitespace: XmlWhitespaceHandling::Collapse,
            default_set: default_set(),
            inject_dtd: None,
            id_prefix: None,
            debug: false,
        }
    }

    /// Parse the configuration from a TOML string (load the data from file yourself).
    pub fn from_toml_str(tomlstr: &str) -> Result<Self, String> {
        toml::from_str(tomlstr).map_err(|e| format!("{}", e))
    }

    pub fn with_debug(mut self, value: bool) -> Self {
        self.debug = value;
        self
    }

    /// Register an XML namespace with prefix
    pub fn with_prefix(mut self, prefix: impl Into<String>, namespace: impl Into<String>) -> Self {
        self.namespaces.insert(prefix.into(), namespace.into());
        self
    }

    /// A prefix to assign when setting annotation IDs, within this string you can use the special variable `{resource}` to use the resource ID.
    pub fn with_id_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.id_prefix = Some(prefix.into());
        self
    }

    /// Inject a DTD (for XML entity resolution)
    pub fn with_inject_dtd(mut self, dtd: impl Into<String>) -> Self {
        self.inject_dtd = Some(dtd.into());
        self
    }

    /// Set default whitespace handling
    pub fn with_whitespace(mut self, handling: XmlWhitespaceHandling) -> Self {
        self.whitespace = handling;
        self
    }

    /// Set an element configuration
    pub fn with_element<F>(mut self, expression: &str, setup: F) -> Self
    where
        F: Fn(XmlElementConfig) -> XmlElementConfig,
    {
        let expression = XPathExpression::new(expression);
        let element = setup(XmlElementConfig::new(expression));
        if self.debug {
            eprintln!("[STAM fromxml] registered {:?}", element);
        }
        self.elements.push(element);
        self
    }

    /// How to handle this element?
    fn element_config(&self, node: Node) -> Option<&XmlElementConfig> {
        let nodepath: NodePath = node.into();
        for elementconfig in self.elements.iter().rev() {
            if elementconfig.path.test(&nodepath, self) {
                return Some(elementconfig);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
/// Determines how to handle whitespace for an XML element
pub enum XmlWhitespaceHandling {
    //Inherit from parent
    Inherit,
    /// Whitespace is kept as is in the XML
    Preserve,
    /// all whitespace becomes space, consecutive whitespace is squashed
    Collapse,
}

impl Default for XmlWhitespaceHandling {
    fn default() -> Self {
        XmlWhitespaceHandling::Inherit
    }
}

impl XmlWhitespaceHandling {
    fn collapse() -> Self {
        XmlWhitespaceHandling::Collapse
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum XmlAnnotationHandling {
    /// No annotation
    None,

    /// Selects the text pertaining to the current element
    TextSelector,

    /// Selects the text pertaining to the current resource
    ResourceSelector,

    /// Selects the text between the current element and the next instance of the same element type
    TextSelectorBetweenMarkers,
}

#[derive(Debug, Clone, Deserialize)]
/// XML Element configuration, determines how to map an XML element (identified by an XPath expression) to STAM
pub struct XmlElementConfig {
    /// This is XPath-like expression (just a small subset of XPath) to identify an element by its path
    path: XPathExpression,

    annotation: XmlAnnotationHandling,

    annotationdata: Vec<XmlAnnotationDataConfig>,

    /// Template or None for no text handling, prefixes are never targeted by annotations
    textprefix: Option<String>,

    /// Template or None for no text handling
    text: Option<String>,

    /// Template or None for no text handling, suffixes are never targeted by annotations
    textsuffix: Option<String>,

    /// Template or None for no ID extraction
    id: Option<String>,

    #[serde(default)]
    /// Descend into children (false) or not? (true)
    stop: bool,

    #[serde(default)]
    /// Whitespace handling for this element
    whitespace: XmlWhitespaceHandling,
}

impl XmlElementConfig {
    fn new(expression: XPathExpression) -> Self {
        Self {
            path: expression,
            stop: false,
            whitespace: XmlWhitespaceHandling::Inherit,
            annotation: XmlAnnotationHandling::None,
            annotationdata: Vec::new(),
            id: None,
            textprefix: None,
            text: None,
            textsuffix: None,
        }
    }

    pub fn compile(&self, engine: &mut Engine<'_>) -> upon::Result<()> {
        if let Some(text) = self.text.as_ref() {
            if engine.get_template(text.as_str()).is_none() {
                engine.add_template(text.clone(), text.clone())?;
            }
        }
        if let Some(id) = self.id.as_ref() {
            if engine.get_template(id.as_str()).is_none() {
                engine.add_template(id.clone(), id.clone())?;
            }
        }
        for annotationdata in self.annotationdata.iter() {
            if let Some(id) = annotationdata.id.as_ref() {
                if engine.get_template(id.as_str()).is_none() {
                    engine.add_template(id.clone(), id.clone())?;
                }
            }
            if let Some(set) = annotationdata.set.as_ref() {
                if engine.get_template(set.as_str()).is_none() {
                    engine.add_template(set.clone(), set.clone())?;
                }
            }
            if let Some(key) = annotationdata.key.as_ref() {
                if engine.get_template(key.as_str()).is_none() {
                    engine.add_template(key.clone(), key.clone())?;
                }
            }
            if let Some(value) = annotationdata.value.as_ref() {
                if engine.get_template(value.as_str()).is_none() {
                    engine.add_template(value.clone(), value.clone())?;
                }
            }
        }
        Ok(())
    }

    /// This sets the mode that determines how the element is handledhttps://www.youtube.com/watch?v=G_BrbhRrP6g
    pub fn with_stop(mut self, stop: bool) -> Self {
        self.stop = stop;
        self
    }

    /// This sets the whitespace handling for this element
    pub fn with_whitespace(mut self, handling: XmlWhitespaceHandling) -> Self {
        self.whitespace = handling;
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn without_text(mut self) -> Self {
        self.text = None;
        self
    }

    pub fn with_annotation(mut self, annotation: XmlAnnotationHandling) -> Self {
        self.annotation = annotation;
        self
    }

    /// Not a very safe hash function (just uses an address uniquely associated with this object) but works for our ends
    fn hash(&self) -> usize {
        self.path.0.as_ptr() as usize
    }
}

impl PartialEq for XmlElementConfig {
    fn eq(&self, other: &Self) -> bool {
        self.hash() == other.hash()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct XmlAnnotationDataConfig {
    /// Template
    id: Option<String>,
    /// Template
    set: Option<String>,
    /// Template
    key: Option<String>,
    /// Template
    value: Option<String>,

    /// Allow value templates that yield an empty string?
    #[serde(default)]
    allow_empty_value: bool,
}

impl XmlAnnotationDataConfig {
    fn new() -> Self {
        Self {
            id: None,
            set: None,
            key: None,
            value: None,
            allow_empty_value: false,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_set(mut self, set: impl Into<String>) -> Self {
        self.set = Some(set.into());
        self
    }

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.key = Some(value.into());
        self
    }
}

/// Not really full XPath, just a very minor subset
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct XPathExpression(String);

impl XPathExpression {
    pub fn new(expression: impl Into<String>) -> Self {
        Self(expression.into())
    }

    pub fn any() -> Self {
        Self("*".into())
    }

    pub fn iter<'a>(
        &'a self,
        config: &'a XmlConversionConfig,
    ) -> impl Iterator<Item = (Option<&'a str>, &'a str)> {
        self.0.trim_start_matches('/').split("/").map(|segment| {
            if let Some((prefix, name)) = segment.split_once(":") {
                if let Some(namespace) = config.namespaces.get(prefix).map(|x| x.as_str()) {
                    (Some(namespace), name)
                } else {
                    panic!(
                        "XML namespace prefix not known in configuration: {}",
                        prefix
                    );
                }
            } else {
                (None, segment)
            }
        })
    }

    /// matches a node path against an XPath-like expression
    fn test<'a, 'b>(&self, path: &NodePath<'a, 'b>, config: &XmlConversionConfig) -> bool {
        let mut pathiter = path.components.iter().rev();
        for (refns, pat) in self.iter(config).collect::<Vec<_>>().into_iter().rev() {
            if let Some((ns, name)) = pathiter.next() {
                if pat != "*" && pat != "" {
                    if refns.is_none() != ns.is_none() || ns != &refns || pat != *name {
                        return false;
                    }
                }
            } else {
                if pat != "" {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NodePath<'a, 'b> {
    components: VecDeque<(Option<&'a str>, &'b str)>,
}

impl<'a, 'b> Display for NodePath<'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (ns, name) in self.components.iter() {
            write!(f, "/")?;
            if let Some(ns) = ns {
                write!(f, "{{{}}}{}", ns, name)?;
            } else {
                write!(f, "{}", name)?;
            }
        }
        Ok(())
    }
}

impl<'a, 'b> From<Node<'a, 'b>> for NodePath<'a, 'b> {
    fn from(node: Node<'a, 'b>) -> Self {
        let mut components = VecDeque::new();
        for ancestor in node.ancestors() {
            if ancestor.tag_name().name() != "" {
                components
                    .push_front((ancestor.tag_name().namespace(), ancestor.tag_name().name()));
            }
        }
        Self { components }
    }
}

/// Translate an XML file to STAM, given a particular configuration
pub fn from_xml<'a>(
    filename: &Path,
    config: &XmlConversionConfig,
    store: &'a mut AnnotationStore,
) -> Result<(), String> {
    if config.debug {
        eprintln!("[STAM fromxml] parsing {}", filename.display());
    }

    // Read the raw XML data
    let mut xmlstring = read_to_string(filename)
        .map_err(|e| format!("Error opening XML file {}: {}", filename.display(), e))?;

    //patchy: remove HTML5 doctype and inject our own
    if xmlstring[..100].find("<!DOCTYPE html>").is_some() && config.inject_dtd.is_some() {
        xmlstring = xmlstring.replacen("<!DOCTYPE html>", "", 1);
    }

    // we can only inject a DTD if there is no doctype
    if xmlstring[..100].find("<!DOCTYPE").is_none() {
        if let Some(dtd) = config.inject_dtd.as_ref() {
            xmlstring = dtd.to_string() + &xmlstring
        };
    } else if config.inject_dtd.is_some() {
        eprintln!("[STAM fromxml] WARNING: Can not inject DTD because file already has a DOCTYPE");
    }

    // parse the raw XML data into a DOM
    let doc = Document::parse_with_options(
        &xmlstring,
        ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        },
    )
    .map_err(|e| format!("Error parsing XML file {}: {}", filename.display(), e))?;

    let mut converter = XmlToStamConverter::new(config);
    converter.compile();

    let textoutfilename = format!(
        "{}.txt",
        filename
            .file_stem()
            .expect("invalid filename")
            .to_str()
            .expect("invalid utf-8 in filename")
    );

    // extract text (first pass)
    converter
        .extract_element_text(doc.root_element(), converter.config.whitespace)
        .map_err(|e| {
            format!(
                "Error extracting element text from {}: {}",
                filename.display(),
                e
            )
        })?;
    if config.debug {
        eprintln!("[STAM fromxml] extracted full text: {}", &converter.text);
    }
    let resource = TextResourceBuilder::new()
        .with_id(textoutfilename.clone())
        .with_text(std::mem::replace(&mut converter.text, String::new()))
        .with_filename(&textoutfilename);

    converter.resource_handle = Some(
        store
            .add_resource(resource)
            .map_err(|e| format!("Failed to add resource {}: {}", &textoutfilename, e))?,
    );

    // extract annotations (second pass)
    converter
        .extract_element_annotation(doc.root_element(), store)
        .map_err(|e| {
            format!(
                "Error extracting element annotation from {}: {}",
                filename.display(),
                e
            )
        })?;

    Ok(())
}

struct XmlToStamConverter<'a> {
    /// The current character position the conversion process is at
    cursor: usize,

    /// The extracted plain-text after/during untangling
    text: String,

    /// The template engine
    template_engine: Engine<'a>,

    /// Keep track of the new positions (unicode offset) where the node starts in the untangled document
    positionmap: HashMap<NodeId, Offset>,

    /// Keep track of the new positions (bytes offset) where the node starts in the untangled document
    bytepositionmap: HashMap<NodeId, (usize, usize)>,

    /// Keep track of markers (XML elements with `XmlAnnotationHandling::TextSelectorBetweenMarkers`), the key in this map is some hash of XmlElementConfig.
    markers: HashMap<usize, Vec<NodeId>>,

    /// The resource
    resource_handle: Option<TextResourceHandle>,

    /// Used to keep track of whether we need to insert a whitespace before actual text
    pending_whitespace: bool,

    /// The configuration
    config: &'a XmlConversionConfig,

    /// Namespace to prefix map
    prefixes: HashMap<String, String>,

    ///  Global context for template
    global_context: BTreeMap<String, upon::Value>,
}

pub enum XmlConversionError {
    StamError(StamError),
    TemplateError(upon::Error),
}

impl From<StamError> for XmlConversionError {
    fn from(error: StamError) -> Self {
        Self::StamError(error)
    }
}

impl From<upon::Error> for XmlConversionError {
    fn from(error: upon::Error) -> Self {
        Self::TemplateError(error)
    }
}

impl Display for XmlConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::StamError(e) => e.fmt(f),
            Self::TemplateError(e) => e.fmt(f),
        }
    }
}

impl<'a> XmlToStamConverter<'a> {
    fn new(config: &'a XmlConversionConfig) -> Self {
        let mut prefixes: HashMap<String, String> = HashMap::new();
        for (prefix, namespace) in config.namespaces.iter() {
            prefixes.insert(namespace.to_string(), prefix.to_string());
        }
        let mut template_engine = Engine::new();
        template_engine.add_filter("capitalize", filter_capitalize);
        template_engine.add_filter("lower", str::to_lowercase);
        template_engine.add_filter("upper", str::to_uppercase);
        template_engine.add_filter("plus", |a: i64, b: i64| a + b);
        template_engine.add_filter("minus", |a: i64, b: i64| a - b);
        template_engine.add_filter("last", |list: &[upon::Value]| list.last().map(Clone::clone));
        template_engine.add_filter("first", |list: &[upon::Value]| {
            list.first().map(Clone::clone)
        });
        let mut converter = Self {
            cursor: 0,
            text: String::new(),
            template_engine: Engine::new(),
            positionmap: HashMap::new(),
            bytepositionmap: HashMap::new(),
            markers: HashMap::new(),
            resource_handle: None,
            pending_whitespace: false,
            global_context: BTreeMap::new(),
            prefixes,
            config,
        };
        converter.set_global_context();
        converter
    }

    /// Compile templates
    fn compile(&mut self) {
        for element in self.config.elements.iter() {
            element.compile(&mut self.template_engine);
        }
    }

    /// untangle text, extract the text (and only the text)
    /// from an XML document, according to the
    /// mapping configuration and creates a STAM TextResource for it.
    /// Records exact offsets per element/node for later use during annotation extraction.
    fn extract_element_text(
        &mut self,
        node: Node,
        whitespace: XmlWhitespaceHandling,
    ) -> Result<(), XmlConversionError> {
        if self.config.debug {
            let path: NodePath = node.into();
            eprintln!("[STAM fromxml] extracting text from {}", path);
        }
        let mut begin = self.cursor; //current character pos marks the begin
        let mut bytebegin = self.text.len(); //current byte pos marks the begin
        let mut end_discount = 0; //the discount may be needed later if textsuffixes are outputted (which we do not want as part of the annotation)
        let mut end_bytediscount = 0;
        let mut firsttext = true; //tracks whether we have already outputted some text, needed for whitespace handling

        // obtain the configuration that applies to this element
        if let Some(element_config) = self.config.element_config(node) {
            if self.config.debug {
                eprintln!("[STAM fromxml]   matching config: {:?}", element_config);
            }

            if !element_config.stop
                && element_config.annotation != XmlAnnotationHandling::TextSelectorBetweenMarkers
                && element_config.text.is_some()
            {
                //do text extraction for this element

                let whitespace = if node.has_attribute((NS_XML, "space")) {
                    // if there is an explicit xml:space attributes, it overrides whatever whitespace handling we have set:
                    match node.attribute((NS_XML, "space")).unwrap() {
                        "preserve" => XmlWhitespaceHandling::Preserve,
                        "collapse" | "replace" => XmlWhitespaceHandling::Collapse,
                        _ => whitespace,
                    }
                } else if element_config.whitespace == XmlWhitespaceHandling::Inherit {
                    whitespace //from parent, i.e. passed to this (recursive) function by caller
                } else {
                    element_config.whitespace //default from the config
                };

                // process the text prefix, a text template to include prior to the actual text
                if let Some(textprefix) = &element_config.textprefix {
                    self.pending_whitespace = false;
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   outputting textprefix: {:?}", textprefix);
                    }
                    let template = self.template_engine.template(textprefix.as_str());
                    let context = self.context_for_node(&node, None, Some(self.cursor), None);
                    let result = template.render(context).to_string()?;
                    let result_charlen = result.chars().count();
                    self.cursor += result_charlen;
                    self.text += &result;

                    // the textprefix will never be part of the annotation's text selection, increment the offsets:
                    begin += result_charlen;
                    bytebegin += result.len();
                }

                // process all child elements
                for child in node.children() {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   child {:?}", child);
                    }
                    if child.is_text() {
                        // extract the actual element text
                        // this may trigger multiple times if the XML element (`node`) has mixed content

                        let mut innertext = child.text().expect("text node must have text");
                        let mut pending_whitespace = false;
                        let mut leading_whitespace = false;
                        if whitespace == XmlWhitespaceHandling::Collapse && !innertext.is_empty() {
                            // analyse what kind of whitespace we are dealing with
                            let mut all_whitespace = true;
                            leading_whitespace = innertext.chars().next().unwrap().is_whitespace();

                            // any pending whitespace after this elements is 'buffered' in this boolean
                            // and only written out depending on the next text's whitespace situation
                            pending_whitespace = innertext
                                .chars()
                                .inspect(|c| {
                                    if !c.is_whitespace() {
                                        all_whitespace = false
                                    }
                                })
                                .last()
                                .unwrap()
                                .is_whitespace();
                            if all_whitespace {
                                self.pending_whitespace = true;
                                if self.config.debug {
                                    eprintln!(
                                        "[STAM fromxml]       all whitespace, flag pending whitespace and skipping...",
                                    );
                                }
                                continue;
                            }
                            innertext = innertext.trim();
                            if self.config.debug {
                                eprintln!(
                                    "[STAM fromxml]       collapsed whitespace: {:?}",
                                    innertext
                                );
                            }
                        }
                        if self.pending_whitespace || leading_whitespace {
                            //output any pending whitespace
                            if !self.text.is_empty()
                                && !self.text.chars().rev().next().unwrap().is_whitespace()
                            {
                                if self.config.debug {
                                    eprintln!("[STAM fromxml]       outputting pending whitespace",);
                                }
                                self.text.push(' ');
                                self.cursor += 1;
                                if firsttext && self.pending_whitespace {
                                    begin += 1;
                                    bytebegin += 1;
                                    firsttext = false;
                                }
                            }
                            self.pending_whitespace = false;
                        }

                        // finally we output the actual text, and advance the cursor
                        if whitespace == XmlWhitespaceHandling::Collapse {
                            let mut prevc = ' ';
                            let mut innertext = innertext.replace(|c: char| c.is_whitespace(), " ");
                            innertext.retain(|c| {
                                let do_retain = c != ' ' || prevc != ' ';
                                prevc = c;
                                do_retain
                            });
                            self.text += &innertext;
                            self.cursor += innertext.chars().count();
                        } else {
                            self.text += &innertext;
                            self.cursor += innertext.chars().count();
                        }
                        self.pending_whitespace = pending_whitespace;
                    } else if child.is_element() {
                        if self.config.debug {
                            eprintln!("[STAM fromxml] <recursion -^>");
                        }
                        // recursion step, process child element, pass our whitespace handling mode since it may inherit it
                        self.extract_element_text(child, whitespace)?;
                        if self.config.debug {
                            eprintln!("[STAM fromxml] </recursion>");
                        }
                    } else {
                        if self.config.debug {
                            eprintln!("[STAM fromxml]   skipping child node");
                        }
                        continue;
                    }
                }

                let textbegin = self.cursor;
                if let Some(template) = &element_config.text {
                    let context = self.context_for_node(
                        &node,
                        Some(&self.text[bytebegin..]),
                        Some(self.cursor),
                        None,
                    );
                    let template = self.template_engine.template(template.as_str());
                    let result = template.render(&context).to_string()?;
                    let result_charlen = result.chars().count();
                    self.cursor += result_charlen;
                    self.text += &result;
                }

                // process the text suffix, a preconfigured string of text to include after to the actual text
                if let Some(textsuffix) = &element_config.textsuffix {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   outputting textsuffix: {:?}", textsuffix);
                    }
                    let context = self.context_for_node(
                        &node,
                        Some(&self.text[bytebegin..]),
                        Some(textbegin),
                        Some(self.cursor),
                    );
                    let template = self.template_engine.template(textsuffix.as_str());
                    let result = template.render(&context).to_string()?;
                    let end_discount_tmp = result.chars().count();
                    let end_bytediscount_tmp = result.len();
                    self.text += &result;

                    // the textsuffix will never be part of the annotation's text selection, we substract a 'discount'
                    self.cursor += end_discount_tmp;
                    self.pending_whitespace = false;
                    end_discount = end_discount_tmp;
                    end_bytediscount = end_bytediscount_tmp;
                }
            } else if element_config.annotation == XmlAnnotationHandling::TextSelectorBetweenMarkers
            {
                // this is a marker, keep track of it so we can extract the span between markers in [`extract_element_annotation()`] later
                if self.config.debug {
                    eprintln!("[STAM fromxml]   adding to markers");
                }
                self.markers
                    .entry(element_config.hash())
                    .and_modify(|v| v.push(node.id()))
                    .or_insert(vec![node.id()]);
            }
        } else if self.config.debug {
            eprintln!(
                "[STAM fromxml]   WARNING: no match, skipping text extraction for element {}",
                NodePath::from(node)
            );
        }

        // Last, we store the new text offsets for this element/node so
        // we can use it in [`extract_element_annotation()`] to associate
        // actual annotations with this span.
        if begin <= (self.cursor - end_discount) {
            let offset = Offset::simple(begin, self.cursor - end_discount);
            self.positionmap.insert(node.id(), offset);
            self.bytepositionmap
                .insert(node.id(), (bytebegin, self.text.len() - end_bytediscount));
            if self.config.debug {
                let path: NodePath = node.into();
                eprintln!(
                    "[STAM fromxml]   extracted text for {}: {:?}",
                    path,
                    &self.text[bytebegin..(self.text.len() - end_bytediscount)]
                );
            }
        }
        Ok(())
    }

    /// extract annotations from the XML document
    /// according to the mapping configuration and creates a STAM TextResource for it.
    /// The text, for the full document, must have already been extracted earlier with [`extract_element_text()`].
    /// This relies on the exact offsets per element/node computed earlier during text extraction (`positionmap`).
    fn extract_element_annotation(
        &mut self,
        node: Node,
        store: &mut AnnotationStore,
    ) -> Result<(), XmlConversionError> {
        if self.config.debug {
            let path: NodePath = node.into();
            eprintln!("[STAM fromxml] extracting annotation from {}", path);
        }
        // obtain the configuration that applies to this element
        if let Some(element_config) = self.config.element_config(node) {
            if self.config.debug {
                eprintln!("[STAM fromxml]   matching config: {:?}", element_config);
            }
            if element_config.annotation != XmlAnnotationHandling::None {
                let mut builder = AnnotationBuilder::new();

                //prepare variables to pass to the template context
                let offset = self.positionmap.get(&node.id());
                let text = if element_config.annotation == XmlAnnotationHandling::TextSelector {
                    if let Some((beginbyte, endbyte)) = self.bytepositionmap.get(&node.id()) {
                        Some(&self.text[*beginbyte..*endbyte])
                    } else {
                        None
                    }
                } else {
                    None
                };
                let begin = if let Some(offset) = offset {
                    if let Cursor::BeginAligned(begin) = offset.begin {
                        Some(begin)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let end = if let Some(offset) = offset {
                    if let Cursor::BeginAligned(end) = offset.end {
                        Some(end)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let context = self.context_for_node(&node, text, begin, end);

                if let Some(template) = &element_config.id {
                    let template = self.template_engine.template(template.as_str());
                    let id = template.render(&context).to_string()?;
                    if !id.is_empty() {
                        builder = builder.with_id(id);
                    }
                }

                for annotationdata in element_config.annotationdata.iter() {
                    let mut databuilder = AnnotationDataBuilder::new();
                    if let Some(template) = &annotationdata.set {
                        let template = self.template_engine.template(template.as_str());
                        let dataset = template.render(&context).to_string()?;
                        if !dataset.is_empty() {
                            databuilder = databuilder.with_dataset(dataset.into());
                        }
                    } else {
                        databuilder =
                            databuilder.with_dataset(self.config.default_set.as_str().into());
                    }
                    if let Some(template) = &annotationdata.key {
                        let template = self.template_engine.template(template.as_str());
                        let key = template.render(&context).to_string()?;
                        if !key.is_empty() {
                            databuilder = databuilder.with_key(key.into())
                        }
                    }
                    if let Some(template) = &annotationdata.value {
                        let template = self.template_engine.template(template.as_str());
                        let value = template.render(&context).to_string()?;
                        if !value.is_empty() || annotationdata.allow_empty_value {
                            databuilder = databuilder.with_value(value.into());
                        }
                    }
                    builder = builder.with_data_builder(databuilder);
                }

                // Finish the builder and add the actual annotation to the store, according to its element handling
                match element_config.annotation {
                    XmlAnnotationHandling::TextSelector => {
                        // Annotation is on text, translates to TextSelector
                        if let Some(selector) = self.textselector(node) {
                            builder = builder.with_target(selector);
                            if self.config.debug {
                                eprintln!("[STAM fromxml]   builder AnnotateText: {:?}", builder);
                            }
                            store.annotate(builder)?;
                        }
                    }
                    XmlAnnotationHandling::ResourceSelector => {
                        // Annotation is metadata, translates to ResourceSelector
                        builder = builder.with_target(SelectorBuilder::ResourceSelector(
                            self.resource_handle.into(),
                        ));
                        if self.config.debug {
                            eprintln!("[STAM fromxml]   builder AnnotateResource: {:?}", builder);
                        }
                        store.annotate(builder)?;
                    }
                    XmlAnnotationHandling::TextSelectorBetweenMarkers => {
                        // Annotation is on a text span *between* two marker elements
                        if let Some(selector) =
                            self.textselector_for_markers(node, store, element_config)
                        {
                            builder = builder.with_target(selector);
                            if self.config.debug {
                                eprintln!(
                                    "[STAM fromxml]   builder TextSelectorBetweenMarkers: {:?}",
                                    builder
                                );
                            }
                            store.annotate(builder)?;
                        }
                    }
                    _ => panic!(
                        "Invalid annotationhandling: {:?}",
                        element_config.annotation
                    ),
                }
            }

            // Recursion step
            if !element_config.stop {
                for child in node.children() {
                    if child.is_element() {
                        self.extract_element_annotation(child, store)?;
                    }
                }
            }
        } else {
            eprintln!(
                "[STAM fromxml]   WARNING: no match, skipping annotation extraction for element {}",
                NodePath::from(node)
            );
        }
        Ok(())
    }

    // translates an XML attribute to a STAM AnnotationData (constructs a builder)
    fn translate_attribute<'b>(
        &self,
        attribute: Attribute<'b, 'b>,
        attrib_config: &'b XmlAnnotationDataConfig,
    ) -> AnnotationDataBuilder<'b> {
        if let Some(namespace) = attribute.namespace() {
            if let Some(set) = attrib_config.set.as_deref() {
                AnnotationDataBuilder::new()
                    .with_dataset(set.into())
                    .with_key(attribute.name().into())
                    .with_value(attribute.value().into())
            } else {
                AnnotationDataBuilder::new()
                    .with_dataset(namespace.into())
                    .with_key(attribute.name().into())
                    .with_value(attribute.value().into())
            }
        } else {
            AnnotationDataBuilder::new()
                .with_dataset(
                    if let Some(set) = attrib_config.set.as_deref() {
                        set
                    } else {
                        "urn:stam-fromxml"
                    }
                    .into(),
                )
                .with_key(attribute.name().into())
                .with_value(attribute.value().into())
        }
    }

    /// Select text corresponding to the element/node
    fn textselector(&self, node: Node) -> Option<SelectorBuilder> {
        let res_handle = self.resource_handle.expect("resource must be associated");
        if let Some(offset) = self.positionmap.get(&node.id()) {
            Some(SelectorBuilder::TextSelector(
                BuildItem::Handle(res_handle),
                offset.clone(),
            ))
        } else {
            None
        }
    }

    /// Select text between this element/node and the next of the same type
    fn textselector_for_markers<'b>(
        &self,
        node: Node,
        store: &AnnotationStore,
        element_config: &'b XmlElementConfig,
    ) -> Option<SelectorBuilder<'b>> {
        let resource = store
            .resource(
                self.resource_handle
                    .expect("resource must have been created"),
            )
            .expect("resource must exist");
        let mut end: Option<usize> = None;
        if let Some(markers) = self.markers.get(&element_config.hash()) {
            let mut grab = false;
            for n_id in markers.iter() {
                if grab {
                    //this marker is the next one, it's begin position is our desired end position
                    end = self.positionmap.get(n_id).map(|offset| {
                        offset
                            .begin
                            .try_into()
                            .expect("begin cursor must be beginaligned")
                    });
                    break;
                }
                if *n_id == node.id() {
                    //current node/marker found, signal grab for the next one
                    grab = true;
                }
            }
        };
        if end.is_none() {
            //no next marker found, use end of document instead
            end = Some(resource.textlen());
        }
        if let (Some(offset), Some(end)) = (self.positionmap.get(&node.id()), end) {
            Some(SelectorBuilder::TextSelector(
                BuildItem::Handle(self.resource_handle.unwrap()),
                Offset::simple(
                    offset
                        .begin
                        .try_into()
                        .expect("begin cursor must be beginaligned"),
                    end,
                ),
            ))
        } else {
            None
        }
    }

    fn set_global_context(&mut self) {
        self.global_context
            .insert("context".into(), self.config.context.clone().into());
        self.global_context
            .insert("namespaces".into(), self.config.namespaces.clone().into());
        self.global_context
            .insert("default_set".into(), self.config.default_set.clone().into());
    }

    fn context_for_node<'input>(
        &self,
        node: &Node<'a, 'input>,
        text: Option<&'input str>,
        begin: Option<usize>,
        end: Option<usize>,
    ) -> upon::Value {
        let mut context = self.global_context.clone();
        let length = if let (Some(begin), Some(end)) = (begin, end) {
            Some(end - begin)
        } else {
            None
        };
        let extended_name = node.tag_name();
        let name = match (extended_name.namespace(), extended_name.name()) {
            (Some(namespace), tagname) => {
                if let Some(prefix) = self.prefixes.get(namespace) {
                    Some(Cow::Owned(format!("{}:{}", prefix, tagname)))
                } else {
                    Some(Cow::Borrowed(tagname))
                }
            }
            (None, tagname) => Some(Cow::Borrowed(tagname)),
        };
        context.insert("localname".into(), node.tag_name().name().into());
        if let Some(name) = name {
            //name with namespace prefix (if any)
            context.insert("name".into(), name.into());
        }
        if let Some(namespace) = node.tag_name().namespace() {
            //the full namespace
            context.insert("namespace".into(), namespace.into());
        }

        // Offset in the untangled plain text
        if let Some(begin) = begin {
            context.insert("begin".into(), upon::Value::Integer(begin as i64));
        }
        if let Some(end) = end {
            context.insert("end".into(), upon::Value::Integer(end as i64));
        }
        if let Some(length) = length {
            context.insert("length".into(), upon::Value::Integer(length as i64));
        }
        if let Some(text) = text {
            context.insert("text".into(), text.into());
        }
        for attrib in node.attributes() {
            if let Some(namespace) = attrib.namespace() {
                if let Some(prefix) = self.prefixes.get(namespace) {
                    context.insert(
                        format!("@{}:{}", prefix, attrib.name()).into(),
                        attrib.value().into(),
                    );
                } else {
                    context.insert(format!("@{}", attrib.name()).into(), attrib.value().into());
                }
            } else {
                context.insert(format!("@{}", attrib.name()).into(), attrib.value().into());
            }
        }

        upon::Value::Map(context)
    }
}

/// Get recursive text without any elements
fn recursive_text(node: Node) -> String {
    let mut s = String::new();
    for child in node.children() {
        if child.is_text() {
            s += child.text().expect("should have text");
        } else if child.is_element() {
            s += &recursive_text(child);
        }
    }
    s
}

// Filters
fn filter_capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        if i == 0 {
            out.push_str(&c.to_uppercase().collect::<String>())
        } else {
            out.push(c);
        }
    }
    out
}
