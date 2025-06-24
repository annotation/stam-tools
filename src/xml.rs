use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, BTreeSet, VecDeque};
use std::fmt::Display;
use std::fs::read_to_string;
use std::path::Path;

use roxmltree::{Attribute, Document, Node, NodeId, ParsingOptions};
use serde::Deserialize;
use stam::*;
use toml;
use upon::Engine;
use crate::info::info;

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
    /// Base elements are named templates, other elements can derive from this
    baseelements: HashMap<String, XmlElementConfig>,

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
            baseelements: HashMap::new(),
            namespaces: HashMap::new(),
            context: HashMap::new(),
            whitespace: XmlWhitespaceHandling::Collapse,
            default_set: default_set(),
            inject_dtd: None,
            id_prefix: None,
            debug: false,
        }
    }

    pub fn resolve_baseelements(&mut self) -> Result<(), XmlConversionError> {
        let mut replace: Vec<(usize, XmlElementConfig)> = Vec::new();
        for (i, element) in self.elements.iter().enumerate() {
            let mut newelement = None;
            for basename in element.base.iter().rev() {
                if let Some(baseelement) = self.baseelements.get(basename) {
                    if newelement.is_none() {
                        newelement = Some(element.clone());
                    }
                    newelement
                        .as_mut()
                        .map(|newelement| newelement.update(baseelement));
                } else {
                    return Err(XmlConversionError::ConfigError(format!(
                        "No such base element: {}",
                        basename
                    )));
                }
            }
            if let Some(newelement) = newelement {
                replace.push((i, newelement));
            }
        }
        for (i, element) in replace {
            self.elements[i] = element;
        }
        Ok(())
    }

    /// Parse the configuration from a TOML string (load the data from file yourself).
    pub fn from_toml_str(tomlstr: &str) -> Result<Self, String> {
        let mut config: Self = toml::from_str(tomlstr).map_err(|e| format!("{}", e))?;
        config.resolve_baseelements().map_err(|e| format!("{}", e))?;
        Ok(config)
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
            if elementconfig.path.test(&nodepath, &node, self) {
                return Some(elementconfig);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
/// Determines how to handle whitespace for an XML element
pub enum XmlWhitespaceHandling {
    /// Not specified (used for base templates)
    Unspecified,
    //Inherit from parent
    Inherit,
    /// Whitespace is kept as is in the XML
    Preserve,
    /// all whitespace becomes space, consecutive whitespace is squashed
    Collapse,
}

impl Default for XmlWhitespaceHandling {
    fn default() -> Self {
        XmlWhitespaceHandling::Unspecified
    }
}

impl XmlWhitespaceHandling {
    fn collapse() -> Self {
        XmlWhitespaceHandling::Collapse
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Copy, Default)]
pub enum XmlAnnotationHandling {
    /// No annotation
    #[default]
    Unspecified,

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

    #[serde(default)]
    path: XPathExpression,

    #[serde(default)]
    annotation: XmlAnnotationHandling,

    #[serde(default)]
    annotationdata: Vec<XmlAnnotationDataConfig>,

    /// Template or None for no text handling, prefixes are never targeted by annotations
    #[serde(default)]
    textprefix: Option<String>,

    /// Extract text. None means unspecified and defaults to false.
    #[serde(default)]
    text: Option<bool>,

    /// Template or None for no text handling, suffixes are never targeted by annotations
    #[serde(default)]
    textsuffix: Option<String>,

    /// Base elements to derive from
    #[serde(default)]
    base: Vec<String>,

    /// Template or None for no ID extraction
    #[serde(default)]
    id: Option<String>,

    #[serde(default)]
    /// Descend into children (false) or not? (true). None means unspecified and defaults to false
    stop: Option<bool>,

    #[serde(default)]
    /// Whitespace handling for this element
    whitespace: XmlWhitespaceHandling,
}

impl XmlElementConfig {
    fn new(expression: XPathExpression) -> Self {
        Self {
            path: expression,
            stop: None,
            whitespace: XmlWhitespaceHandling::Unspecified,
            annotation: XmlAnnotationHandling::Unspecified,
            annotationdata: Vec::new(),
            base: Vec::new(),
            id: None,
            textprefix: None,
            text: None,
            textsuffix: None,
        }
    }

    pub fn update(&mut self, base: &XmlElementConfig) {
        if self.whitespace == XmlWhitespaceHandling::Unspecified
            && base.whitespace != XmlWhitespaceHandling::Unspecified
        {
            self.whitespace = base.whitespace;
        }
        if self.annotation == XmlAnnotationHandling::Unspecified
            && base.annotation != XmlAnnotationHandling::Unspecified
        {
            self.annotation = base.annotation;
        }
        if self.textprefix.is_none() && base.textprefix.is_some() {
            self.textprefix = base.textprefix.clone();
        }
        if self.text.is_none() && base.text.is_some() {
            self.text = base.text;
        }
        if self.textsuffix.is_none() && base.textsuffix.is_some() {
            self.textsuffix = base.textsuffix.clone();
        }
        if self.id.is_none() && base.id.is_some() {
            self.id = base.id.clone();
        }
        if self.stop.is_none() && base.stop.is_some() {
            self.stop = base.stop;
        }
        for annotationdata in base.annotationdata.iter() {
            if !self.annotationdata.contains(annotationdata) {
                self.annotationdata.push(annotationdata.clone());
            }
        }
    }


    /// This sets the mode that determines how the element is handledhttps://www.youtube.com/watch?v=G_BrbhRrP6g
    pub fn with_stop(mut self, stop: bool) -> Self {
        self.stop = Some(stop);
        self
    }

    /// This sets the whitespace handling for this element
    pub fn with_whitespace(mut self, handling: XmlWhitespaceHandling) -> Self {
        self.whitespace = handling;
        self
    }

    pub fn with_text(mut self, text: bool) -> Self {
        self.text = Some(text);
        self
    }

    pub fn with_base(mut self, iter: impl Iterator<Item = impl Into<String>>) -> Self {
        self.base = iter.into_iter().map(|s| s.into()).collect();
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

#[derive(Debug, Clone, Deserialize, PartialEq)]
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

    /// Skip this data entirely if any underlying variables in the templates are undefined
    #[serde(default)]
    skip_if_missing: bool,
}

impl XmlAnnotationDataConfig {
    fn new() -> Self {
        Self {
            id: None,
            set: None,
            key: None,
            value: None,
            allow_empty_value: false,
            skip_if_missing: false,
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
        self.main().trim_start_matches('/').split("/").map(|segment| {
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
    fn test<'a, 'b>(&self, path: &NodePath<'a, 'b>, node: &Node<'a,'b>, config: &XmlConversionConfig) -> bool {
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
        //condition parsing (very basic language only)
        if let Some(condition) = self.condition() {
            if let Some(pos) = condition.find("!=") {
                let var = &condition[..pos];
                let right = condition[pos..].trim_matches('"');
                if self.get_var(var, node, config) == Some(right) {
                    return false;
                }
            } else if let Some(pos) = condition.find("=") {
                let var = &condition[..pos];
                let right = condition[pos..].trim_matches('"');
                if self.get_var(var, node, config) != Some(right) {
                    return false;
                }
            } else {
                //whole condition is one variable
                let v = self.get_var(condition, node, config);
                if v.is_none() || v == Some("") {
                    return false;
                }
            }

        }
        true
    }

    /// Resolve a variable from a conditional expression, given a variable name, node and condfig
    fn get_var<'a,'b>(&self, var: &str, node: &Node<'a,'b>, config: &XmlConversionConfig) -> Option<&'a str> { 
        if var.starts_with("@") {
            if let Some(pos) = var.find(":") {
                let prefix = &var[1..pos];
                if let Some(ns) = config.namespaces.get(prefix) {
                    let var = &var[pos+1..];
                    node.attribute((ns.as_str(),var))
                } else {
                    None
                }
            } else {
                node.attribute(&var[1..])
            }
        } else if var == "text()" {
            node.text().map(|s|s.trim())
        } else {
            None
        }
    }



    /// Returns the main part of the expression, without any conditions
    fn main(&self) -> &str {
        if let Some(end) = self.0.find("[") {
            &self.0[..end]
        } else {
            &self.0
        }
    }

    /// Returns the conditional part of the expression
    fn condition(&self) -> Option<&str> {
        if let (Some(begin), Some(end)) = (self.0.find("["), self.0.find("]")) {
            Some(&self.0[begin + 1..end])
        } else {
            None
        }
    }
}

impl Default for XPathExpression {
    fn default() -> Self {
        Self::any()
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
    converter
        .compile()
        .map_err(|e| format!("Error compiling templates: {}", e))?;

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
        .with_text(converter.text.clone())
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

/// Translate an XML file to STAM, given a particular configuration. Not writing output files and keeping all in memory. Does not support DTD injection.
pub fn from_xml_in_memory<'a>(
    resource_id: &str, 
    xmlstring: &str,
    config: &XmlConversionConfig,
    store: &'a mut AnnotationStore,
) -> Result<(), String> {
    if config.debug {
        eprintln!("[STAM fromxml] parsing XML string");
    }

    // parse the raw XML data into a DOM
    let doc = Document::parse_with_options(
        &xmlstring,
        ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        },
    )
    .map_err(|e| format!("Error parsing XML string: {}",  e))?;

    let mut converter = XmlToStamConverter::new(config);
    converter
        .compile()
        .map_err(|e| format!("Error compiling templates: {}", e))?;

    // extract text (first pass)
    converter
        .extract_element_text(doc.root_element(), converter.config.whitespace)
        .map_err(|e| {
            format!(
                "Error extracting element text from {}: {}",
                resource_id,
                e
            )
        })?;
    if config.debug {
        eprintln!("[STAM fromxml] extracted full text: {}", &converter.text);
    }
    let resource = TextResourceBuilder::new()
        .with_id(resource_id)
        .with_text(converter.text.clone());

    converter.resource_handle = Some(
        store
            .add_resource(resource)
            .map_err(|e| format!("Failed to add resource {}: {}", &resource_id, e))?,
    );

    // extract annotations (second pass)
    converter
        .extract_element_annotation(doc.root_element(), store)
        .map_err(|e| {
            format!(
                "Error extracting element annotation from {}: {}",
                resource_id,
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

    /// Variable names per template
    variables: BTreeMap<String, BTreeSet<&'a str>>,
    
    debugindent: String,
}

pub enum XmlConversionError {
    StamError(StamError),
    TemplateError(String, Option<upon::Error>),
    ConfigError(String),
}

impl From<StamError> for XmlConversionError {
    fn from(error: StamError) -> Self {
        Self::StamError(error)
    }
}

impl From<upon::Error> for XmlConversionError {
    fn from(error: upon::Error) -> Self {
        Self::TemplateError("".into(), Some(error))
    }
}

impl Display for XmlConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::StamError(e) => e.fmt(f),
            Self::TemplateError(s, e) => {
                f.write_str(s.as_str());
                f.write_str(": ");
                if let Some(e) = e {
                    e.fmt(f)?;
                }
                f.write_str("")
            }
            Self::ConfigError(e) => e.fmt(f),
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
        template_engine.add_function("capitalize", filter_capitalize);
        template_engine.add_function("lower", str::to_lowercase);
        template_engine.add_function("upper", str::to_uppercase);
        template_engine.add_function("trim", |s: &str| s.trim().to_string() );
        template_engine.add_function("plus", |a: i64, b: i64| a + b);
        template_engine.add_function("minus", |a: i64, b: i64| a - b);
        template_engine.add_function("eq", |a: &upon::Value, b: &upon::Value| a == b);
        template_engine.add_function("last", |list: &[upon::Value]| list.last().map(Clone::clone));
        template_engine.add_function("first", |list: &[upon::Value]| {
            list.first().map(Clone::clone)
        });
        template_engine.add_function("tokenize", |s: &str| {
            upon::Value::List(
                s.split(" \n").filter_map(|x|
                    if !x.is_empty() { 
                        Some(upon::Value::String(x.to_string())) 
                    } else {
                        None
                    }
                )
                .collect::<Vec<upon::Value>>())
        });
        let mut converter = Self {
            cursor: 0,
            text: String::new(),
            template_engine,
            positionmap: HashMap::new(),
            bytepositionmap: HashMap::new(),
            markers: HashMap::new(),
            resource_handle: None,
            pending_whitespace: false,
            global_context: BTreeMap::new(),
            debugindent: String::new(),
            variables: BTreeMap::new(),
            prefixes,
            config,
        };
        converter.set_global_context();
        converter
    }

    /// Compile templates
    fn compile(&mut self) -> Result<(), XmlConversionError> {
        if self.config.debug {
            eprintln!("[STAM fromxml] compiling templates");
        }
        for element in self.config.elements.iter() {
            if let Some(textprefix) = element.textprefix.as_ref() {
                if self.template_engine.get_template(textprefix.as_str()).is_none() {
                    let template = self.precompile(textprefix.as_str());
                    self.template_engine
                        .add_template(textprefix.clone(), template)
                        .map_err(|e| {
                            XmlConversionError::TemplateError(
                                format!("element/textprefix template {}", textprefix.clone()),
                                Some(e),
                            )
                        })?;
                }
            }
            if let Some(textsuffix) = element.textsuffix.as_ref() {
                if self.template_engine.get_template(textsuffix.as_str()).is_none() {
                    let template = self.precompile(textsuffix.as_str());
                    self.template_engine
                        .add_template(textsuffix.clone(), template)
                        .map_err(|e| {
                            XmlConversionError::TemplateError(
                                format!("element/textsuffix template {}", textsuffix.clone()),
                                Some(e),
                            )
                        })?;
                }
            }
            if let Some(id) = element.id.as_ref() {
                if self.template_engine.get_template(id.as_str()).is_none() {
                    let template = self.precompile(id.as_str());
                    self.template_engine.add_template(id.clone(), template).map_err(|e| {
                        XmlConversionError::TemplateError(
                            format!("element/id template {}", id.clone()),
                            Some(e),
                        )
                    })?;
                }
            }
            for annotationdata in element.annotationdata.iter() {
                if let Some(id) = annotationdata.id.as_ref() {
                    if self.template_engine.get_template(id.as_str()).is_none() {
                        let template = self.precompile(id.as_str());
                        self.template_engine.add_template(id.clone(), template).map_err(|e| {
                            XmlConversionError::TemplateError(
                                format!("annotationdata/id template {}", id.clone()),
                                Some(e),
                            )
                        })?;
                    }
                }
                if let Some(set) = annotationdata.set.as_ref() {
                    if self.template_engine.get_template(set.as_str()).is_none() {
                        let template = self.precompile(set.as_str());
                        //eprintln!("------- DEBUG: {} -> {}", set.as_str(), template);
                        self.template_engine.add_template(set.clone(), template).map_err(|e| {
                            XmlConversionError::TemplateError(
                                format!("annotationdata/set template {}", set.clone()),
                                Some(e),
                            )
                        })?;
                    }
                }
                if let Some(key) = annotationdata.key.as_ref() {
                    if self.template_engine.get_template(key.as_str()).is_none() {
                        let template = self.precompile(key.as_str());
                        self.template_engine.add_template(key.clone(), template).map_err(|e| {
                            XmlConversionError::TemplateError(
                                format!("annotationdata/key template {}", key.clone()),
                                Some(e),
                            )
                        })?;
                    }
                }
                if let Some(value) = annotationdata.value.as_ref() {
                    if self.template_engine.get_template(value.as_str()).is_none() {
                        let template = self.precompile(value.as_str());
                        self.template_engine.add_template(value.clone(), template).map_err(|e| {
                            XmlConversionError::TemplateError(
                                format!("annotationdata/value template {}", value.clone()),
                                Some(e),
                            )
                        })?;
                    }
                }
            }
        }
        Ok(())
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
            eprintln!("[STAM fromxml]{} extracting text for element {}", self.debugindent, path);
        }
        let mut begin = self.cursor; //current character pos marks the begin
        let mut bytebegin = self.text.len(); //current byte pos marks the begin
        let mut end_discount = 0; //the discount may be needed later if textsuffixes are outputted (which we do not want as part of the annotation)
        let mut end_bytediscount = 0;
        let mut firsttext = true; //tracks whether we have already outputted some text, needed for whitespace handling

        // obtain the configuration that applies to this element
        if let Some(element_config) = self.config.element_config(node) {
            if self.config.debug {
                eprintln!("[STAM fromxml]{} matching config: {:?}", self.debugindent, element_config);
            }

            if (element_config.stop == Some(false) || element_config.stop.is_none())
                && element_config.annotation != XmlAnnotationHandling::TextSelectorBetweenMarkers
            {
                //do text extraction for this element

                let whitespace = if node.has_attribute((NS_XML, "space")) {
                    // if there is an explicit xml:space attributes, it overrides whatever whitespace handling we have set:
                    match node.attribute((NS_XML, "space")).unwrap() {
                        "preserve" => XmlWhitespaceHandling::Preserve,
                        "collapse" | "replace" => XmlWhitespaceHandling::Collapse,
                        _ => whitespace,
                    }
                } else if element_config.whitespace == XmlWhitespaceHandling::Inherit
                    || element_config.whitespace == XmlWhitespaceHandling::Unspecified
                {
                    whitespace //from parent, i.e. passed to this (recursive) function by caller
                } else {
                    element_config.whitespace //default from the config
                };

                // process the text prefix, a text template to include prior to the actual text
                if let Some(textprefix) = &element_config.textprefix {
                    self.pending_whitespace = false;
                    if self.config.debug {
                        eprintln!("[STAM fromxml]{} outputting textprefix: {:?}", self.debugindent, textprefix);
                    }
                    let result =
                        self.render_template(textprefix, &node, Some(self.cursor), None)
                            .map_err(|e| match e {
                                XmlConversionError::TemplateError(s, e) => {
                                    XmlConversionError::TemplateError(
                                        format!(
                                        "whilst rendering textprefix template '{}' for node '{}': {}",
                                        textprefix, node.tag_name().name(), s
                                    ),
                                        e,
                                    )
                                }
                                e => e,
                            })?;
                    let result_charlen = result.chars().count();
                    self.cursor += result_charlen;
                    self.text += &result;

                    // the textprefix will never be part of the annotation's text selection, increment the offsets:
                    begin += result_charlen;
                    bytebegin += result.len();
                }

                let textbegin = self.cursor;
                // process all child elements
                for child in node.children() {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]{} child {:?}", self.debugindent, child);
                    }
                    if child.is_text() && element_config.text == Some(true) {
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
                                        "[STAM fromxml]{} ^- all whitespace, flag pending whitespace and skipping...",
                                        self.debugindent,
                                    );
                                }
                                continue;
                            }
                            innertext = innertext.trim();
                            if self.config.debug {
                                eprintln!(
                                    "[STAM fromxml]{} ^- collapsed whitespace: {:?}",
                                    self.debugindent,
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
                                    eprintln!("[STAM fromxml]{} ^- outputting pending whitespace",self.debugindent);
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
                            if self.config.debug {
                                eprintln!("[STAM fromxml]{} ^- outputting text child (collapsed whitespace), cursor is now {}: {}",self.debugindent, self.cursor, innertext);
                            }
                        } else {
                            self.text += &innertext;
                            self.cursor += innertext.chars().count();
                            if self.config.debug {
                                eprintln!("[STAM fromxml]{} ^- outputting text child, cursor is now {}: {}",self.debugindent, self.cursor, innertext);
                            }
                        }
                        self.pending_whitespace = pending_whitespace;
                    } else if child.is_element() {
                        if self.config.debug {
                            eprintln!("[STAM fromxml]{} \\- extracting text for this child", self.debugindent);
                        }
                        self.debugindent.push_str("  ");
                        // recursion step, process child element, pass our whitespace handling mode since it may inherit it
                        self.extract_element_text(child, whitespace)?;
                        self.debugindent.pop();
                        self.debugindent.pop();
                    } else {
                        if self.config.debug {
                            eprintln!("[STAM fromxml]{} ^- skipping this child node", self.debugindent);
                        }
                        continue;
                    }
                }


                // process the text suffix, a preconfigured string of text to include after to the actual text
                if let Some(textsuffix) = &element_config.textsuffix {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]{} outputting textsuffix: {:?}", self.debugindent, textsuffix);
                    }
                    let result = self.render_template(
                        textsuffix.as_str(),
                        &node,
                        Some(textbegin),
                        Some(self.cursor),
                    ).map_err(|e| match e {
                            XmlConversionError::TemplateError(s, e) => {
                                XmlConversionError::TemplateError(
                                    format!(
                                        "whilst rendering textsuffix template '{}' for node '{}': {}",
                                        textsuffix,
                                        node.tag_name().name(),
                                        s
                                    ),
                                    e,
                                )
                            }
                            e => e,
                    })?;
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
                    eprintln!("[STAM fromxml]{} adding to markers", self.debugindent);
                }
                self.markers
                    .entry(element_config.hash())
                    .and_modify(|v| v.push(node.id()))
                    .or_insert(vec![node.id()]);
            }
        } else if self.config.debug {
            eprintln!(
                "[STAM fromxml]{} WARNING: no match, skipping text extraction for element {}",
                self.debugindent,
                NodePath::from(node)
            );
        }

        // Last, we store the new text offsets for this element/node so
        // we can use it in [`extract_element_annotation()`] to associate
        // actual annotations with this span.
        if begin <= (self.cursor - end_discount) {
            let offset = Offset::simple(begin, self.cursor - end_discount);
            if self.config.debug {
                let path: NodePath = node.into();
                eprintln!(
                    "[STAM fromxml]{} extracted text for {} @{:?}: {:?}",
                    self.debugindent,
                    path,
                    &offset,
                    &self.text[bytebegin..(self.text.len() - end_bytediscount)]
                );
            }
            self.positionmap.insert(node.id(), offset);
            self.bytepositionmap
                .insert(node.id(), (bytebegin, self.text.len() - end_bytediscount));
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
            eprintln!("[STAM fromxml]{} extracting annotation from {}", self.debugindent, path);
        }
        // obtain the configuration that applies to this element
        if let Some(element_config) = self.config.element_config(node) {
            if self.config.debug {
                eprintln!("[STAM fromxml]{} matching config: {:?}", self.debugindent, element_config);
            }
            if element_config.annotation != XmlAnnotationHandling::None
                && element_config.annotation != XmlAnnotationHandling::Unspecified
            {
                let mut builder = AnnotationBuilder::new();

                //prepare variables to pass to the template context
                let offset = self.positionmap.get(&node.id());
                if element_config.annotation == XmlAnnotationHandling::TextSelector {
                    if self.text.is_empty() {
                        return Err(XmlConversionError::ConfigError("Can't extract annotations on text if no text was extracted!".into()));
                    }
                    if let Some((beginbyte, endbyte)) = self.bytepositionmap.get(&node.id()) {
                        if self.config.debug {
                            eprintln!("[STAM fromxml]{} annotation covers text {:?} (bytes {}-{})", self.debugindent, offset, beginbyte, endbyte);
                        }
                    }
                }
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


                if let Some(template) = &element_config.id {
                    let context = self.context_for_node(&node, begin, end, template.as_str());
                    let compiled_template = self.template_engine.template(template.as_str());
                    let id = compiled_template.render(&context).to_string().map_err(|e| 
                            XmlConversionError::TemplateError(
                                format!(
                                    "whilst rendering id template '{}' for node '{}'",
                                    template,
                                    node.tag_name().name(),
                                ),
                                Some(e),
                            )
                        )?;
                    if !id.is_empty() {
                        builder = builder.with_id(id);
                    }
                }

                for annotationdata in element_config.annotationdata.iter() {
                    let mut databuilder = AnnotationDataBuilder::new();
                    if let Some(template) = &annotationdata.set {
                        let context = self.context_for_node(&node, begin, end, template.as_str());
                        let compiled_template = self.template_engine.template(template.as_str());
                        let dataset = compiled_template.render(&context).to_string().map_err(|e| 
                                XmlConversionError::TemplateError(
                                    format!(
                                        "whilst rendering annotationdata/dataset template '{}' for node '{}'",
                                        template,
                                        node.tag_name().name(),
                                    ),
                                    Some(e),
                                )
                            )?;
                        if !dataset.is_empty() {
                            databuilder = databuilder.with_dataset(dataset.into())
                        }
                    } else {
                        databuilder =
                            databuilder.with_dataset(self.config.default_set.as_str().into());
                    }
                    if let Some(template) = &annotationdata.key {
                        let context = self.context_for_node(&node, begin, end, template.as_str());
                        let compiled_template = self.template_engine.template(template.as_str());
                        match compiled_template.render(&context).to_string().map_err(|e| 
                                XmlConversionError::TemplateError(
                                    format!(
                                        "whilst rendering annotationdata/key template '{}' for node '{}'",
                                        template,
                                        node.tag_name().name(),
                                    ),
                                    Some(e),
                                )
                            )  {
                            Ok(key) if !key.is_empty() =>
                                databuilder = databuilder.with_key(key.into()) ,
                            Ok(_) if !annotationdata.skip_if_missing => {
                                return Err(XmlConversionError::TemplateError(
                                    format!(
                                        "whilst rendering annotationdata/key template '{}' for node '{}'",
                                        template,
                                        node.tag_name().name(),
                                    ),
                                    None
                                ));
                            },
                            Err(e) if !annotationdata.skip_if_missing => {
                                return Err(e)
                            },
                            _ => {
                                //skip if missing, no op
                            }
                        }
                    }
                    if let Some(template) = &annotationdata.value {
                        let context = self.context_for_node(&node, begin, end, template.as_str());
                        let compiled_template = self.template_engine.template(template.as_str());
                        match compiled_template.render(&context).to_string().map_err(|e| 
                                XmlConversionError::TemplateError(
                                    format!(
                                        "whilst rendering annotationdata/value template '{}' for node '{}'",
                                        template,
                                        node.tag_name().name(),
                                    ),
                                    Some(e),
                                )
                            )  {
                            Ok(value) =>
                                if !value.is_empty() || annotationdata.allow_empty_value {
                                    databuilder = databuilder.with_value(value.into());
                                },
                            Err(e) if !annotationdata.skip_if_missing => {
                                return Err(e)
                            },
                            Err(_) if annotationdata.allow_empty_value => {
                                    databuilder = databuilder.with_value("".into());
                                },
                            Err(_) => {
                                //skip if missing, no op
                            }
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
            if element_config.stop == Some(false) || element_config.stop.is_none() {
                for child in node.children() {
                    if child.is_element() {
                        self.debugindent.push_str("  ");
                        self.extract_element_annotation(child, store)?;
                        self.debugindent.pop();
                        self.debugindent.pop();
                    }
                }
            }
        } else {
            eprintln!(
                "[STAM fromxml]{} WARNING: no match, skipping annotation extraction for element {}",
                self.debugindent,
                NodePath::from(node)
            );
        }
        Ok(())
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

    fn render_template<'input, 't>(
        &self,
        template: &'t str,
        node: &Node<'a, 'input>,
        begin: Option<usize>,
        end: Option<usize>,
    ) -> Result<Cow<'t, str>, XmlConversionError> {
        if template.chars().any(|c| c == '{') {
            //value is a template, templating engine probably needed
            let compiled_template = self.template_engine.template(template);
            let context = self.context_for_node(&node, begin, end, template);
            let result = compiled_template.render(context).to_string()?;
            Ok(Cow::Owned(result))
        } else {
            //value is a literal: templating engine not needed
            Ok(Cow::Borrowed(template))
        }
    }

    fn context_for_node<'input>(
        &self,
        node: &Node<'a, 'input>,
        begin: Option<usize>,
        end: Option<usize>,
        template: &str, 
    ) -> upon::Value {
        let mut context = self.global_context.clone();
        let length = if let (Some(begin), Some(end)) = (begin, end) {
            Some(end - begin)
        } else {
            None
        };
        context.insert("localname".into(), node.tag_name().name().into());
        if let Some(name) = self.get_node_name(node) {
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

        if let Some(vars) = self.variables.get(template) {
            for var in vars {
                if let Some((encodedvar, value)) = self.context_for_var(node, var) {
                    context.insert(encodedvar, value);
                }
            }
        }
        upon::Value::Map(context)
    }

    /// Returns a variable to be set for the context, from the XML DOM
    // returns the *encoded* variable name (safe to pass to template), and the value
    fn context_for_var<'input>(
        &self,
        mut node: &Node<'a, 'input>,
        var: &str, 
    ) -> Option<(String, upon::Value)> {
        let mut path = String::new();
        let var = 
        if var.starts_with("?.$") {
            path.push_str("?.ELEMENT_");
            &var[3..]
        } else if var.starts_with("$") {
            path.push_str("ELEMENT_");
            &var[1..]
        } else if var.starts_with("?.@") {
            path.push_str("?.");
            &var[2..]
        } else {
            var
        };
        for (i, component) in var.split("/").enumerate() {
            if component.starts_with("@"){
                if let Some(pos) = component.find(":") {
                    let prefix = &component[1..pos];
                    if let Some(ns) = self.config.namespaces.get(prefix) {
                        let var = &component[pos+1..];
                        return Some((
                            format!("{}ATTRIB_{}__{}", path, prefix, var).into(),
                            node.attribute((ns.as_str(),var)).into()
                        ));
                    } else {
                        return None;
                    }
                } else {
                    return Some((
                        format!("{}ATTRIB_{}",  path, component).into(),
                        node.attribute(component).into()
                    ));
                }
            } else if component == ".." {
                if let Some(parentnode) = node.parent_element().as_ref() {
                    let mut truncvar = String::new();
                    for (j, component2) in var.split("/").enumerate() {
                        if j>i {
                            if truncvar.is_empty() {
                                truncvar.push('$');
                            } else {
                                truncvar.push('/');
                            }
                            truncvar.push_str(component2);
                        }
                    }
                    //recursion needed because of lifetime issues
                    if let Some((newpath, value)) = self.context_for_var(parentnode, truncvar.as_str()) {
                        if newpath.starts_with("ELEMENT_") && !path.is_empty() {
                            path.push_str(&newpath[8..])
                        } else {
                            path.push_str(&newpath);
                        }
                        return Some((path, value));
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else if component == "." {
                path += "THIS";
            } else {
                let mut newnode: Option<_> = None;
                let (prefix, localname)  = if let Some(pos) = component.find(":") {
                    (Some(&component[pos+1..]),  &component[1..pos])
                } else {
                    (None, component)
                };
                for child in node.children() {
                    if child.is_element() {
                        let namedata = child.tag_name();
                        let child_matches = if let Some(namespace) = namedata.namespace() {
                            if let Some(foundprefix) = self.prefixes.get(namespace) {
                                Some(foundprefix.as_str()) == prefix && localname == namedata.name()
                            } else {
                                false
                            }
                        } else {
                            namedata.name() == localname
                        };
                        if child_matches {
                            newnode = Some(node);
                            if i > 0 {
                                path.push_str("_IN_");
                            }
                            if let Some(prefix) = prefix {
                                path.push_str(prefix);
                                path.push_str("__");
                            } else {
                            }
                            path.push_str(localname);
                            break;
                        }
                    }
                }
                if let Some(newnode) = newnode {
                    node = newnode;
                }
            }
        }
        return Some((path, recursive_text(node).into()));
    }

    fn get_node_name<'b>(&self, node: &'b Node) -> Option<Cow<'b,str>> {
        let extended_name = node.tag_name();
        match (extended_name.namespace(), extended_name.name()) {
            (Some(namespace), tagname) => {
                if let Some(prefix) = self.prefixes.get(namespace) {
                    Some(Cow::Owned(format!("{}__{}", prefix, tagname)))
                } else {
                    Some(Cow::Borrowed(tagname))
                }
            }
            (None, tagname) => Some(Cow::Borrowed(tagname)),
        }
    }

    fn precompile(&mut self, template: &'a str) -> Cow<'a,str> {
        let mut replacement = String::new();
        let mut variables: BTreeSet<&'a str> = BTreeSet::new();
        let mut begin = 0;
        let mut end = 0;
        for i  in 0..template.len() {
            let slice = &template[i..];
            if slice.starts_with("{{") || slice.starts_with("{%") {
                begin = i;
            } else if slice.starts_with("}}") || slice.starts_with("%}") {
                if end < begin+2 {
                    replacement.push_str(&template[end..begin+2]);
                }
                let inner = &template[begin+2..i]; //the part without the {{  }}
                replacement.push_str(&self.precompile_inblock(inner, &mut variables));
                end = i;
            }
        }
        if end > 0 {
            replacement.push_str(&template[end..]);
        }
        self.variables.insert(template.into(), variables);

        if !replacement.is_empty() {
            Cow::Owned(replacement)
        } else {
            Cow::Borrowed(template)
        }
    }

    fn precompile_inblock<'s>(&self, s: &'s str, vars: &mut BTreeSet<&'s str>) -> Cow<'s,str> {
        let mut quoted = false;
        let mut var = false;
        let mut begin = 0;
        let mut end = 0;
        let mut replacement = String::new();
        for (i,c) in s.char_indices() {
            if c == '"' {
                quoted = !quoted;
            } else if !quoted {
                if !var && (c == '@' || c == '$') {
                    //token is an XML variable name, its syntax needs some changes before it can be used in the templating engine
                    var = true;
                    begin = i;
                } else if var && !c.is_alphanumeric() && c != '.' && c != '/' && c != '_' && c != ':' && c != '@' {
                    //end of variable
                    if end < begin {
                        replacement.push_str(&s[end..begin]);
                    }
                    let varname = &s[begin..i];
                    vars.insert(varname);
                    let replacement_var = self.precompile_name(varname);
                    replacement += &replacement_var;
                    var = false;
                    end = i;
                }
            }
        }
        if end > 0 {
            replacement.push_str(&s[end..]);
        }
        if var {
            //don't forget last one
            let varname = &s[begin..];
            vars.insert(varname);
            let replacement_var = self.precompile_name(varname);
            replacement += &replacement_var;
        }
        if !replacement.is_empty() {
            Cow::Owned(replacement)
        } else {
            Cow::Borrowed(s)
        }
    }

    /// upon's templating syntax doesn't support some of the characters we use in names, this function substitutes them for more verbose equivalents
    fn precompile_name(&self, s: &str) -> String {
        let mut replacement = String::new();
        let mut skip = 0;
        for (i,c) in s.char_indices() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            if c == '$' {
                let slice = &s[i..];
                if slice.starts_with("$..") {
                    replacement.push_str("ELEMENT_PARENT");
                    skip = 2;
                } else if slice.starts_with("$.") {
                    replacement.push_str("ELEMENT_THIS");
                    skip = 1;
                } else {
                    replacement.push_str("ELEMENT_");
                }
            } else if c == '@' {
                replacement.push_str("ATTRIB_");
            } else if c == '/' {
                replacement.push_str("_IN_");
            } else if c == ':' {
                replacement.push_str("__");
            } else {
                replacement.push(c);
            }
        }
        //eprintln!("DEBUG: precompile_name({}) -> {}", s, replacement);
        replacement
    }
}



/// Get recursive text without any elements
fn recursive_text(node: &Node) -> String {
    let mut s = String::new();
    for child in node.children() {
        if child.is_text() {
            s += child.text().expect("should have text");
        } else if child.is_element() {
            s += &recursive_text(&child);
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


#[cfg(test)]
mod tests {
    use super::*;

    const XMLSMALLEXAMPLE: &'static str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body><head><title>test</title></head><h1>TEST</h1><p xml:id="p1">This  is a <em xml:id="emphasis">test</em>.</p></body></html>"#;

    const XMLEXAMPLE: &'static str = r#"<!DOCTYPE entities[<!ENTITY nbsp "&#xA0;">]>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:my="http://example.com">
<head>
    <title>Test</title>
    <meta name="author" content="proycon" />
</head>
<body>
    <h1>Header</h1>

    <p xml:id="par1">
        <span xml:id="sen1">This is a sentence.</span>
        <span xml:id="sen2">This is the second&nbsp;sentence.</span>
    </p>
    <p xml:id="par2">
        <strong>This</strong> is    the <em>second</em> paragraph.
            It has a <strong>bold</strong> word and one in <em>italics</em>.<br/>
        Let's highlight stress in the following word: <span my:stress="secondary">re</span>pu<span my:stress="primary">ta</span>tion.
    </p>
    <p xml:space="preserve"><![CDATA[This    third
paragraph consists
of CDATA and is configured to preserve whitespace, and weird &entities; ]]></p>

    <h2>Subsection</h2>

    <p>
    Have some fruits:<br/>
    <ul xml:id="list1">
        <li xml:id="fruit1">apple</li>
        <li xml:id="fruit2">banana</li>
        <li xml:id="fruit3">melon</li>
    </ul>
    </p>

    Some lingering text outside of any confines...
</body>
</html>"#;

    const CONF: &'static str = r#"#default whitespace handling (Collapse or Preserve)
whitespace = "Collapse"
default_set = "urn:stam-fromhtml" 

[namespaces]
#this defines the namespace prefixes you can use in this configuration
xml = "http://www.w3.org/XML/1998/namespace"
html = "http://www.w3.org/1999/xhtml"
xsd =  "http://www.w3.org/2001/XMLSchema"
xlink = "http://www.w3.org/1999/xlink"

# elements and attributes are matched in reverse-order, so put more generic statements before more specific ones

#Define some base elements that we reuse later for actual elements (prevents unnecessary repetition)
[baseelements.common]
id = "{% if ?.@xml:id %}{{ @xml:id }}{% endif %}"

    [[baseelements.common.annotationdata]]
    key = "type"
    value = "{{ localname }}"

    [[baseelements.common.annotationdata]]
    key = "lang"
    value = "{{ @xml:lang }}"
    skip_if_missing = true

    [[baseelements.common.annotationdata]]
    key = "n"
    value = "{{ @n }}"
    skip_if_missing = true

    [[baseelements.common.annotationdata]]
    key = "style"
    value = "{{ @style }}"
    skip_if_missing = true

    [[baseelements.common.annotationdata]]
    key = "class"
    value = "{{ @class }}"
    skip_if_missing = true

    [[baseelements.common.annotationdata]]
    key = "src"
    value = "{{ @src }}"
    skip_if_missing = true

[baseelements.text]
text = true


[[elements]]
base = [ "text", "common" ]
path = "*"
text = true
annotation = "TextSelector"

# Pass through the following elements without mapping to text
[[elements]]
base = [ "common" ]
path = "//html:head"

[[elements]]
base = [ "common" ]
path = "//html:head//*"

# Map metadata like <meta name="key" content="value"> to annotations with key->value data selecting the resource (ResourceSelector)
[[elements]]
base = [ "common" ]
path = "//html:head//html:meta"

[[elements.annotationdata]]
key = "{% if ?.@name %}{{ name }}{% endif %}"
value = "{% if ?.@content %}{{ @content }}{% endif %}"
skip_if_missing = true

# By default, ignore any tags in the head (unless they're mentioned specifically later in the config)
[[elements]]
path = "//html:head/html:title"
annotation = "ResourceSelector"

[[elements.annotationdata]]
key = "title"
value = "{{ $. | trim }}"


# Determine how various structural elements are converted to text

[[elements]]
base = [ "common" ]
path = "//html:br"
textsuffix = "\n"

[[elements]]
base = [ "common", "text" ]
path = "//html:p"
textprefix = "\n"
textsuffix = "\n"

# Let's do headers and bulleted lists like markdown
[[elements]]
base = [ "common", "text" ]
path = "//html:h1"
textsuffix = "\n"

[[elements]]
base = [ "common", "text" ]
path = "//html:h2"
textsuffix = "\n"

[[elements]]
base = [ "common", "text" ]
path = "//html:li"
textprefix = "* "
textsuffix = "\n"

[[elements]]
base = [ "common", "text" ]
path = "//html:li/html:li"
textprefix = "  * "
textsuffix = "\n"

[[elements]]
base = [ "common", "text" ]
path = "//html:li/html:li/html:li"
textprefix = "    * "
textsuffix = "\n"
"#;

    #[test]
    fn test_precompile_template_nochange() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ foo }}";
        let template_out = conv.precompile(template_in);
        assert_eq!( template_out, template_in);
        //foo is not a special variable
        assert!(!conv.variables.get(template_in).as_ref().unwrap().contains("foo"));
        Ok(())
    }

    #[test]
    fn test_precompile_template_attrib() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ @foo }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ATTRIB_foo }}");
        //foo is an attribute so is returned 
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("@foo"));
        Ok(())
    }

    #[test]
    fn test_precompile_template_attrib_ns() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ @bar:foo }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ATTRIB_bar__foo }}");
        //foo is an attribute so is returned 
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("@bar:foo"));
        Ok(())
    }

    #[test]
    fn test_precompile_template_element() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ $foo }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ELEMENT_foo }}");
        //foo is an element so is returned 
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("$foo"));
        Ok(())
    }

    #[test]
    fn test_precompile_template_element_ns() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ $bar:foo }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ELEMENT_bar__foo }}");
        //foo is an element so is returned 
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("$bar:foo"));
        Ok(())
    }

    #[test]
    fn test_precompile_template_this_text() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ $. }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ELEMENT_THIS }}");
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("$."));
        Ok(())
    }

    #[test]
    fn test_precompile_template_parent_text() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ $.. }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ELEMENT_PARENT }}");
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("$.."));
        Ok(())
    }


    #[test]
    fn test_precompile_template_attrib2() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{% for x in @foo %}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{% for x in ATTRIB_foo %}");
        //foo is an attribute so is returned 
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("@foo"));
        Ok(())
    }

    #[test]
    fn test_precompile_template_attrib3() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ ?.@foo }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ?.ATTRIB_foo }}");
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("@foo"));
        Ok(())
    }

    #[test]
    fn test_precompile_template_path() -> Result<(), String> {
        let config = XmlConversionConfig::new();
        let mut conv = XmlToStamConverter::new(&config);
        let template_in = "{{ $x/y/z/@a }}";
        let template_out = conv.precompile(template_in);
        assert_eq!(template_out, "{{ ELEMENT_x_IN_y_IN_z_IN_ATTRIB_a }}");
        assert!(conv.variables.get(template_in).as_ref().unwrap().contains("$x/y/z/@a"));
        Ok(())
    }

    #[test]
    fn test_loadconfig() -> Result<(), String> {
        let config = XmlConversionConfig::from_toml_str(CONF)?;
        let mut conv = XmlToStamConverter::new(&config);
        conv.compile().map_err(|e| format!("{}",e))?;
        assert_eq!(conv.config.namespaces.len(),4 , "number of namespaces");
        assert_eq!(conv.config.elements.len(), 12, "number of elements");
        assert_eq!(conv.config.baseelements.len(), 2, "number of baseelements");
        assert_eq!(conv.config.elements.get(0).unwrap().annotationdata.len(), 6,"number of annotationdata under first element");
        assert_eq!(conv.config.baseelements.get("common").unwrap().annotationdata.len(), 6,"number of annotationdata under baseelement common");
        Ok(())
    }

    #[test]
    fn test_small() -> Result<(), String> {
        let config = XmlConversionConfig::from_toml_str(CONF)?;
        let mut store = stam::AnnotationStore::new(stam::Config::new());
        from_xml_in_memory("test", XMLSMALLEXAMPLE, &config, &mut store)?;
        let res = store.resource("test").expect("resource must have been created at this point");
        assert_eq!(res.text(), "TEST\n\nThis is a test.\n", "resource text");
        assert_eq!(store.annotations_len(), 4, "number of annotations");
        info(&store, true);
        let annotation = store.annotation("emphasis").expect("annotation must have been created at this point");
        assert_eq!(annotation.text_simple(), Some("test"));
        let key = store.key("urn:stam-fromhtml", "title").expect("key must exist");
        let annotation = res.annotations_as_metadata().next().expect("annotation");
        assert_eq!(annotation.data().filter_key(&key).value_as_str(), Some("test"));
        Ok(())
    }

    #[test]
    fn test_full() -> Result<(), String> {
        let config = XmlConversionConfig::from_toml_str(CONF)?;
        let mut store = stam::AnnotationStore::new(stam::Config::new());
        from_xml_in_memory("test", XMLEXAMPLE, &config, &mut store)?;
        Ok(())
    }

}
