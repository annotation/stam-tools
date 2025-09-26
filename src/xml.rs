use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, BTreeSet};
use std::fmt::Display;
use std::fs::read_to_string;
use std::path::Path;
use std::hash::{Hash,DefaultHasher,Hasher};

use roxmltree::{Document, Node, NodeId, ParsingOptions};
use serde::Deserialize;
use stam::*;
use toml;
use upon::Engine;

const NS_XML: &str = "http://www.w3.org/XML/1998/namespace";
const CONTEXT_ANNO: &str = "http://www.w3.org/ns/anno.jsonld";


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
    /// Sets additional context variables that can be used in templates
    metadata: Vec<MetadataConfig>,

    #[serde(default)]
    /// Inject a DTD (for XML entity resolution)
    inject_dtd: Option<String>,

    #[serde(default = "default_set")]
    default_set: String,

    #[serde(default)]
    /// A prefix to assign when setting annotation IDs, within this string you can use the special variable `{resource}` to use the resource ID.
    id_prefix: Option<String>,

    #[serde(default)]
    /// Add provenance information pointing each annotation to the appropriate node in the XML source files where it came from (translates into XPathSelector in Web Annotation output)
    provenance: bool,

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
            metadata: Vec::new(),
            whitespace: XmlWhitespaceHandling::Collapse,
            default_set: default_set(),
            inject_dtd: None,
            id_prefix: None,
            provenance: false,
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

    /// Add provenance information pointing each annotation to the appropriate node in the XML source files where it came from (translates into XPathSelector in Web Annotation output)
    pub fn with_provenance(mut self, value: bool) -> Self {
        self.provenance = value;
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
    fn element_config(&self, node: Node, path: &NodePath) -> Option<&XmlElementConfig> {
        for elementconfig in self.elements.iter().rev() {
            if elementconfig.path.test(path, &node, self) {
                return Some(elementconfig);
            }
        }
        None
    }

    pub fn add_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.context.insert(key.into(), value.into());
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
    /// Any string values are interpreted as templates
    value: Option<toml::Value>,

    /// Allow value templates that yield an empty string?
    #[serde(default)]
    allow_empty_value: bool,

    /// Skip this data entirely if any underlying variables in the templates are undefined
    #[serde(default)]
    skip_if_missing: bool,
}

impl XmlAnnotationDataConfig {
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

    pub fn with_value(mut self, value: impl Into<toml::Value>) -> Self {
        self.value = Some(value.into());
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
            if let Some(component) = pathiter.next() {
                if pat != "*" && pat != "" {
                    if refns.is_none() != component.namespace.is_none() || component.namespace != refns || pat != component.tagname {
                        return false;
                    }
                }
            } else {
                if pat != "" {
                    return false;
                }
            }
        }
        //condition parsing (very basic language only), only supported on the leaf node
        if let Some(condition) = self.condition() {
            for condition in condition.split(" and ") { //MAYBE TODO: doesn't take quotes into account yet!
                let condition = condition.trim();
                if let Some(pos) = condition.find("!=") {
                    let var = &condition[..pos];
                    let right = condition[pos+2..].trim_matches('"');
                    if self.get_var(var, node, config) == Some(right) {
                        return false;
                    }
                } else if let Some(pos) = condition.find("=") {
                    let var = &condition[..pos];
                    let right = condition[pos+1..].trim_matches('"');
                    let value = self.get_var(var, node, config);
                    if value != Some(right) {
                        return false;
                    }
                } else {
                    //condition is one variable and merely needs to exist
                    let v = self.get_var(condition, node, config);
                    if v.is_none() || v == Some("") {
                        return false;
                    }
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

    /// Returns the conditional part of the expression, only supported on the leaf node
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
struct NodePathComponent<'a,'b> {
    namespace: Option<&'a str>,
    tagname: &'b str,
    /// Index sequence number, 1-indexed (as specified by XPath)
    index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Default)]
struct NodePath<'a, 'b> {
    components: Vec<NodePathComponent<'a,'b>>,
}

impl<'a, 'b> Display for NodePath<'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for component in self.components.iter() {
            write!(f, "/")?;
            if let Some(ns) = component.namespace {
                if let Some(index) = component.index {
                    write!(f, "{{{}}}{}[{}]", ns, component.tagname, index)?;
                } else {
                    write!(f, "{{{}}}{}", ns, component.tagname)?;
                }
            } else {
                if let Some(index) = component.index {
                    write!(f, "{}[{}]", component.tagname, index)?;
                } else {
                    write!(f, "{}", component.tagname)?;
                }
            }
        }
        Ok(())
    }
}

impl<'a,'b> NodePath<'a,'b> {
    fn add(&mut self, node: &Node<'a,'b>, index: Option<usize>) {
        if node.tag_name().name() != "" {
            self.components.push(
                NodePathComponent {
                    namespace: node.tag_name().namespace(),
                    tagname: node.tag_name().name(),
                    index,
                }
            )
        }
    }

    fn format_as_xpath(&self, prefixes: &HashMap<String, String>) -> String {
        let mut out = String::new();
        for component in self.components.iter() {
            out.push('/');
            if let Some(ns) = component.namespace {
                if let Some(prefix) = prefixes.get(ns) {
                    if let Some(index) = component.index {
                        out += &format!("{}:{}[{}]", prefix, component.tagname, index);
                    } else {
                        out += &format!("{}:{}", prefix, component.tagname);
                    }
                } else {
                    eprintln!("STAM fromxml WARNING: format_as_xpath: namespace {} not defined, no prefix found!", ns);
                    if let Some(index) = component.index {
                        out += &format!("{}[{}]", component.tagname, index);
                    } else {
                        out += &format!("{}", component.tagname);
                    }
                }
            } else {
                if let Some(index) = component.index {
                    out += &format!("{}[{}]", component.tagname, index);
                } else {
                    out += &format!("{}", component.tagname);
                }
            }
        }
        out
    }
}


/// Counts elder siblings, used to determine index values
#[derive(Default,Debug)]
struct SiblingCounter {
    map: HashMap<String,usize>,
}

impl SiblingCounter {
    fn count<'a,'b>(&mut self, node: &Node<'a,'b>) -> usize {
        let s = format!("{:?}", node.tag_name());
        *self.map.entry(s).and_modify(|c| {*c += 1;}).or_insert(1)
    }
}


#[derive(Debug, Clone, Deserialize)]
/// XML Element configuration, determines how to map an XML element (identified by an XPath expression) to STAM
pub struct MetadataConfig {
    /// This is XPath-like expression (just a small subset of XPath) to identify an element by its path
    #[serde(default)]
    annotation: XmlAnnotationHandling,

    #[serde(default)]
    annotationdata: Vec<XmlAnnotationDataConfig>,

    /// Template or None for no ID extraction
    #[serde(default)]
    id: Option<String>,
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
    let mut path = NodePath::default();
    path.add(&doc.root_element(), None);
    converter
        .extract_element_text(doc.root_element(), &path, converter.config.whitespace, Some(textoutfilename.as_str()), Some(&filename.to_string_lossy()), 0)
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

    converter.add_metadata(store).map_err(|e| format!("Failed to add metadata {}: {}", &textoutfilename, e))?;

    // extract annotations (second pass)
    converter
        .extract_element_annotation(doc.root_element(), &path,  Some(&filename.to_string_lossy()),0,  store)
        .map_err(|e| {
            format!(
                "Error extracting element annotation from {}: {}",
                filename.display(),
                e
            )
        })?;

    Ok(())
}

/// Translate an XML file to STAM, given a particular configuration. This translates multiple XML files to a single output file.
pub fn from_multi_xml<'a>(
    filenames: &Vec<&Path>,
    config: &XmlConversionConfig,
    store: &'a mut AnnotationStore,
) -> Result<(), String> {

    let textoutfilename = format!(
        "{}.txt",
            filenames.iter().next().expect("1 or more filename need to be provided")
            .file_stem()
            .expect("invalid filename")
            .to_str()
            .expect("invalid utf-8 in filename")
    );

    // Read the raw XML data
    let mut xmlstrings: Vec<String> = Vec::new();
    let mut docs: Vec<Document> = Vec::new();
    for filename in filenames.iter() {
        if config.debug {
            eprintln!("[STAM fromxml] parsing {} (one of multiple)", filename.display());
        }
        //patchy: remove HTML5 doctype and inject our own
        let mut xmlstring = read_to_string(filename).map_err(|e| format!("Error opening XML file {}: {}", filename.display(), e))?;
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
        xmlstrings.push(xmlstring);
    }

    for (filename, xmlstring) in filenames.iter().zip(xmlstrings.iter()) {
        // parse the raw XML data into a DOM
        let doc = Document::parse_with_options(
            xmlstring,
            ParsingOptions {
                allow_dtd: true,
                ..ParsingOptions::default()
            },
        )
        .map_err(|e| format!("Error parsing XML file {}: {}", filename.display(), e))?;
        docs.push(doc);
    }

    let mut converter = XmlToStamConverter::new(config);
    converter
        .compile()
        .map_err(|e| format!("Error compiling templates: {}", e))?;

    for (i, (doc, filename)) in docs.iter().zip(filenames.iter()).enumerate() {
        let mut path = NodePath::default();
        path.add(&doc.root_element(), None);
        // extract text (first pass)
        converter
            .extract_element_text(doc.root_element(), &path, converter.config.whitespace, Some(textoutfilename.as_str()), Some(&filename.to_string_lossy()), i)
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

    converter.add_metadata(store).map_err(|e| format!("Failed to add metadata {}: {}", &textoutfilename, e))?;

    // extract annotations (second pass)
    for (i,(doc, filename)) in docs.iter().zip(filenames.iter()).enumerate() {
        let mut path = NodePath::default();
        path.add(&doc.root_element(), None);
        converter
            .extract_element_annotation(doc.root_element(), &path, Some(&filename.to_string_lossy()),i,  store)
            .map_err(|e| {
                format!(
                    "Error extracting element annotation from {}: {}",
                    filename.display(),
                    e
                )
            })?;
    }

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

    let mut path = NodePath::default();
    path.add(&doc.root_element(), None);
    // extract text (first pass)
    converter
        .extract_element_text(doc.root_element(), &path, converter.config.whitespace, Some(resource_id), Some(resource_id), 0)
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

    converter.add_metadata(store).map_err(|e| format!("Failed to add metadata for {}: {}", &resource_id, e))?;

    // extract annotations (second pass)
    converter
        .extract_element_annotation(doc.root_element(), &path, Some(resource_id), 0, store)
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

    /// Keep track of the new positions (unicode offset) where the node starts in the untangled document. The key consist of a document sequence number and a node ID.
    positionmap: HashMap<(usize,NodeId), Offset>,

    /// Keep track of the new positions (bytes offset) where the node starts in the untangled document. The key consist of a document sequence number and a node ID.
    bytepositionmap: HashMap<(usize,NodeId), (usize, usize)>,

    /// Keep track of markers (XML elements with `XmlAnnotationHandling::TextSelectorBetweenMarkers`), the key in this map is some hash of XmlElementConfig.
    markers: HashMap<usize, Vec<(usize,NodeId)>>,

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
                f.write_str(s.as_str())?;
                f.write_str(": ")?;
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
        template_engine.add_function("add", |a: i64, b: i64| a + b);
        template_engine.add_function("sub", |a: i64, b: i64| a - b);
        template_engine.add_function("mul", |a: i64, b: i64| a * b);
        template_engine.add_function("div", |a: i64, b: i64| a / b);
        template_engine.add_function("eq", |a: &upon::Value, b: &upon::Value| a == b);
        template_engine.add_function("ne", |a: &upon::Value, b: &upon::Value| a != b);
        template_engine.add_function("gt", |a: i64, b: i64| a > b);
        template_engine.add_function("lt", |a: i64, b: i64| a < b);
        template_engine.add_function("gte", |a: i64, b: i64| a >= b);
        template_engine.add_function("lte", |a: i64, b: i64| a <= b);
        template_engine.add_function("int", |a: &upon::Value| match a {
            upon::Value::Integer(x) => upon::Value::Integer(*x), 
            upon::Value::Float(x) => upon::Value::Integer(*x as i64), 
            upon::Value::String(s) => upon::Value::Integer(s.parse().expect("int filter expects an integer value")),
            _ => panic!("int filter expects an integer value"), //<< --^  TODO: PANIC IS WAY TO STRICT
        });
        template_engine.add_function("as_range", |a: i64| upon::Value::List(std::ops::Range { start: 0, end: a }.into_iter().map(|x| upon::Value::Integer(x+1)).collect::<Vec<_>>()) );
        template_engine.add_function("last", |list: &[upon::Value]| list.last().map(Clone::clone));
        template_engine.add_function("first", |list: &[upon::Value]| {
            list.first().map(Clone::clone)
        });
        template_engine.add_function("tokenize", |s: &str| {
            upon::Value::List(
                s.split(|c| c == ' ' || c == '\n').filter_map(|x|
                    if !x.is_empty() { 
                        Some(upon::Value::String(x.to_string())) 
                    } else {
                        None
                    }
                )
                .collect::<Vec<upon::Value>>())
        });
        template_engine.add_function("replace", |s: &str, from: &str, to: &str| { 
            upon::Value::String(s.replace(from,to))
        });
        template_engine.add_function("basename", |a: &upon::Value| match a {
            upon::Value::String(s) => upon::Value::String(s.split(|c| c == '/' || c == '\\').last().expect("splitting must work").to_string()),
            _ => panic!("basename filter expects a string value"), //<< --^  TODO: PANIC IS WAY TO STRICT
        });
        template_engine.add_function("noext", |a: &upon::Value| match a {
            upon::Value::String(s) => if let Some(pos) = s.rfind('.') {
                s[..pos].to_string()
            } else {
                s.to_string()
            },
            _ => panic!("basename filter expects a string value"), //<< --^  TODO: PANIC IS WAY TO STRICT
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
                    self.compile_value(value)?;
                }
            }
        }
        for metadata in self.config.metadata.iter() {
            if let Some(id) = metadata.id.as_ref() {
                if self.template_engine.get_template(id.as_str()).is_none() {
                    let template = self.precompile(id.as_str());
                    self.template_engine.add_template(id.clone(), template).map_err(|e| {
                        XmlConversionError::TemplateError(
                            format!("metadata/id template {}", id.clone()),
                            Some(e),
                        )
                    })?;
                }
            }
            for annotationdata in metadata.annotationdata.iter() {
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
                    self.compile_value(value)?;
                }
            }
        }
        Ok(())
    }

    /// Compile templates from a value, all strings are considered templates
    fn compile_value(&mut self, value: &'a toml::Value) -> Result<(), XmlConversionError> {
        match value {
            toml::Value::String(value) => {
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
            toml::Value::Table(map) => {
                for (_key, value) in map.iter() {
                    self.compile_value(value)?;
                }
            },
            toml::Value::Array(list) => {
                for value in list.iter() {
                    self.compile_value(value)?;
                }
            }
            _ => {} //no templates in other types
        }
        Ok(())
    }

    /// untangle text, extract the text (and only the text)
    /// from an XML document, according to the
    /// mapping configuration and creates a STAM TextResource for it.
    /// Records exact offsets per element/node for later use during annotation extraction.
    fn extract_element_text<'b>(
        &mut self,
        node: Node<'a,'b>,
        path: &NodePath<'a,'b>,
        whitespace: XmlWhitespaceHandling,
        resource_id: Option<&str>,
        inputfile: Option<&str>,
        doc_num: usize,
    ) -> Result<(), XmlConversionError> {
        if self.config.debug {
            eprintln!("[STAM fromxml]{} extracting text for element {}", self.debugindent, path);
        }
        let mut begin = self.cursor; //current character pos marks the begin
        let mut bytebegin = self.text.len(); //current byte pos marks the begin
        let mut end_discount = 0; //the discount may be needed later if textsuffixes are outputted (which we do not want as part of the annotation)
        let mut end_bytediscount = 0;
        let mut firsttext = true; //tracks whether we have already outputted some text, needed for whitespace handling

        let mut elder_siblings = SiblingCounter::default();

        // obtain the configuration that applies to this element
        if let Some(element_config) = self.config.element_config(node, path) {
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
                        self.render_template(textprefix, &node, Some(self.cursor), None, resource_id, inputfile, doc_num)
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
                        let mut path = path.clone();
                        let count = elder_siblings.count(&child);
                        path.add(&child, Some(count));
                        self.extract_element_text(child, &path, whitespace, resource_id, inputfile, doc_num)?;
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
                        resource_id,
                        inputfile,
                        doc_num
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
                    .and_modify(|v| v.push((doc_num, node.id())))
                    .or_insert(vec![(doc_num, node.id())]);
            }
        } else if self.config.debug {
            eprintln!(
                "[STAM fromxml]{} WARNING: no match, skipping text extraction for element {}",
                self.debugindent,
                path
            );
        }

        // Last, we store the new text offsets for this element/node so
        // we can use it in [`extract_element_annotation()`] to associate
        // actual annotations with this span.
        if begin <= (self.cursor - end_discount) {
            let offset = Offset::simple(begin, self.cursor - end_discount);
            if self.config.debug {
                eprintln!(
                    "[STAM fromxml]{} extracted text for {} @{:?}: {:?}",
                    self.debugindent,
                    path,
                    &offset,
                    &self.text[bytebegin..(self.text.len() - end_bytediscount)]
                );
            }
            self.positionmap.insert((doc_num, node.id()), offset);
            self.bytepositionmap
                .insert((doc_num, node.id()), (bytebegin, self.text.len() - end_bytediscount));
        }
        Ok(())
    }

    /// extract annotations from the XML document
    /// according to the mapping configuration and creates a STAM TextResource for it.
    /// The text, for the full document, must have already been extracted earlier with [`extract_element_text()`].
    /// This relies on the exact offsets per element/node computed earlier during text extraction (`positionmap`).
    fn extract_element_annotation<'b>(
        &mut self,
        node: Node<'a,'b>,
        path: &NodePath<'a,'b>,
        inputfile: Option<&str>,
        doc_num: usize,
        store: &mut AnnotationStore,
    ) -> Result<(), XmlConversionError> {
        if self.config.debug {
            eprintln!("[STAM fromxml]{} extracting annotation from {}", self.debugindent, path);
        }

        let mut elder_siblings = SiblingCounter::default();

        // obtain the configuration that applies to this element
        if let Some(element_config) = self.config.element_config(node, &path) {
            if self.config.debug {
                eprintln!("[STAM fromxml]{} matching config: {:?}", self.debugindent, element_config);
            }
            if element_config.annotation != XmlAnnotationHandling::None
                && element_config.annotation != XmlAnnotationHandling::Unspecified
            {
                let mut builder = AnnotationBuilder::new();

                //prepare variables to pass to the template context
                let offset = self.positionmap.get(&(doc_num, node.id()));
                if element_config.annotation == XmlAnnotationHandling::TextSelector {
                    if let Some((beginbyte, endbyte)) = self.bytepositionmap.get(&(doc_num, node.id())) {
                        if self.config.debug {
                            eprintln!("[STAM fromxml]{} annotation covers text {:?} (bytes {}-{})", self.debugindent, offset, beginbyte, endbyte);
                        }
                    }  else if self.text.is_empty() {
                        return Err(XmlConversionError::ConfigError("Can't extract annotations on text if no text was extracted!".into()));
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

                let resource_id = if let Some(resource_handle) = self.resource_handle {
                    store.resource(resource_handle).unwrap().id()
                } else {
                    None
                };

                if let Some(template) = &element_config.id {
                    let context = self.context_for_node(&node, begin, end, template.as_str(), resource_id, inputfile, doc_num);
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
                        let context = self.context_for_node(&node, begin, end, template.as_str(), resource_id, inputfile, doc_num);
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
                        let context = self.context_for_node(&node, begin, end, template.as_str(), resource_id, inputfile, doc_num);
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
                                //skip whole databuilder if missing
                                continue
                            }
                        }
                    }
                    if let Some(value) = &annotationdata.value {
                        match self.extract_value(value,  node, annotationdata.allow_empty_value, annotationdata.skip_if_missing, begin, end, resource_id, inputfile, doc_num)? {
                            Some(value) => {
                                databuilder = databuilder.with_value(value);
                            },
                            None =>  {
                                //skip whole databuilder if missing
                                continue
                            }
                        }
                    }
                    builder = builder.with_data_builder(databuilder);
                }

                if self.config.provenance  && inputfile.is_some() {
                    let path_string = if let Some(id) = node.attribute((NS_XML,"id")) {
                        //node has an ID, use that
                        format!("//{}[@xml:id=\"{}\"]", self.get_node_name_for_xpath(&node), id)
                    } else {
                        //no ID, use full XPath expression
                        path.format_as_xpath(&self.prefixes)
                    };
                    let databuilder = AnnotationDataBuilder::new().with_dataset(CONTEXT_ANNO.into()).with_key("target".into()).with_value(
                        BTreeMap::from([
                            ("source".to_string(),inputfile.unwrap().into()),
                            ("selector".to_string(), 
                                    BTreeMap::from([
                                        ("type".to_string(),"XPathSelector".into()),
                                        ("value".to_string(),path_string.into())
                                    ]).into()
                            )
                        ]).into()
                    );
                    builder = builder.with_data_builder(databuilder);
                }


                // Finish the builder and add the actual annotation to the store, according to its element handling
                match element_config.annotation {
                    XmlAnnotationHandling::TextSelector => {
                        // Annotation is on text, translates to TextSelector
                        if let Some(selector) = self.textselector(node, doc_num) {
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
                            self.textselector_for_markers(node, doc_num, store, element_config)
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
                        let mut path = path.clone();
                        let count = elder_siblings.count(&child);
                        path.add(&child, Some(count));
                        //eprintln!("DEBUG: count={}, child={:?}, parent={:?}, elder_siblings={:?}", count, child.tag_name(), node.tag_name(), elder_siblings);
                        self.extract_element_annotation(child, &path, inputfile, doc_num, store)?;
                        self.debugindent.pop();
                        self.debugindent.pop();
                    }
                }
            }
        } else {
            eprintln!(
                "[STAM fromxml]{} WARNING: no match, skipping annotation extraction for element {}",
                self.debugindent,
                path
            );
        }
        Ok(())
    }

    /// Extract values, running the templating engine in case of string values
    fn extract_value<'b>(&self, value: &'a toml::Value, node: Node<'a,'b>, allow_empty_value: bool, skip_if_missing: bool, begin: Option<usize>, end: Option<usize>, resource_id: Option<&str>, inputfile: Option<&str>, doc_num: usize) -> Result<Option<DataValue>, XmlConversionError>{
        match value {
            toml::Value::String(template) => {  
                let context = self.context_for_node(&node, begin, end, template.as_str(), resource_id, inputfile, doc_num);
                let compiled_template = self.template_engine.template(template.as_str()); //panics if doesn't exist, but that can't happen
                match compiled_template.render(&context).to_string().map_err(|e| 
                        XmlConversionError::TemplateError(
                            format!(
                                "whilst rendering annotationdata/map template '{}' for node '{}'",
                                template,
                                node.tag_name().name(),
                            ),
                            Some(e),
                        )
                    )  {
                    Ok(value) => {
                        if !value.is_empty() || allow_empty_value {
                            Ok(Some(value.into()))
                        } else {
                            //skip
                            Ok(None)
                        }
                    },
                    Err(e) if !skip_if_missing => {
                        Err(e)
                    },
                    Err(_) if allow_empty_value => {
                        Ok(Some("".into()))
                    },
                    Err(_) => {
                        //skip whole databuilder if missing
                        Ok(None)
                    }
                }
            },
            toml::Value::Table(map) => {  
                let mut resultmap: BTreeMap<String,DataValue> = BTreeMap::new();
                for (key, value) in map.iter() {
                    if let Some(value) = self.extract_value(value,  node, false, true, begin, end, resource_id, inputfile, doc_num)? {
                        resultmap.insert(key.clone(), value);
                    }
                }
                Ok(Some(resultmap.into()))
            },
            toml::Value::Array(list) => {  
                let mut resultlist: Vec<DataValue> = Vec::new();
                for value in list.iter() {
                    if let Some(value) = self.extract_value(value, node, false, true, begin, end, resource_id, inputfile, doc_num)? {
                        resultlist.push(value);
                    }
                }
                Ok(Some(resultlist.into()))
            }
            toml::Value::Boolean(v) => Ok(Some(DataValue::Bool(*v))),
            toml::Value::Float(v) => Ok(Some(DataValue::Float(*v))),
            toml::Value::Integer(v) => Ok(Some(DataValue::Int(*v as isize))),
            toml::Value::Datetime(_v) => {
                todo!("fromxml: Datetime conversion not implemented yet");
            }
        }
    }

    /// Extract values for metadata (no associated node), running the templating engine in case of string values
    fn extract_value_metadata<'b>(&self, value: &'a toml::Value, context: &upon::Value, allow_empty_value: bool, skip_if_missing: bool, resource_id: Option<&str>) -> Result<Option<DataValue>, XmlConversionError>{
        match value {
            toml::Value::String(template) => {  
                let compiled_template = self.template_engine.template(template.as_str()); //panics if doesn't exist, but that can't happen
                match compiled_template.render(&context).to_string().map_err(|e| 
                        XmlConversionError::TemplateError(
                            format!(
                                "whilst rendering annotationdata/metadata template '{}' for metadata",
                                template,
                            ),
                            Some(e),
                        )
                    )  {
                    Ok(value) => {
                        if !value.is_empty() || allow_empty_value {
                            Ok(Some(value.into()))
                        } else {
                            //skip
                            Ok(None)
                        }
                    },
                    Err(e) if !skip_if_missing => {
                        Err(e)
                    },
                    Err(_) if allow_empty_value => {
                        Ok(Some("".into()))
                    },
                    Err(_) => {
                        //skip whole databuilder if missing
                        Ok(None)
                    }
                }
            },
            toml::Value::Table(map) => {  
                let mut resultmap: BTreeMap<String,DataValue> = BTreeMap::new();
                for (key, value) in map.iter() {
                    if let Some(value) = self.extract_value_metadata(value, context, false, true,  resource_id)? {
                        resultmap.insert(key.clone(), value);
                    }
                }
                Ok(Some(resultmap.into()))
            },
            toml::Value::Array(list) => {  
                let mut resultlist: Vec<DataValue> = Vec::new();
                for value in list.iter() {
                    if let Some(value) = self.extract_value_metadata(value, context, false, true, resource_id)? {
                        resultlist.push(value);
                    }
                }
                Ok(Some(resultlist.into()))
            }
            toml::Value::Boolean(v) => Ok(Some(DataValue::Bool(*v))),
            toml::Value::Float(v) => Ok(Some(DataValue::Float(*v))),
            toml::Value::Integer(v) => Ok(Some(DataValue::Int(*v as isize))),
            toml::Value::Datetime(_v) => {
                todo!("fromxml: Datetime conversion not implemented yet");
            }
        }
    }

    /// Select text corresponding to the element/node and document number
    fn textselector<'s>(&'s self, node: Node, doc_num: usize) -> Option<SelectorBuilder<'s>> {
        let res_handle = self.resource_handle.expect("resource must be associated");
        if let Some(offset) = self.positionmap.get(&(doc_num, node.id())) {
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
        doc_num: usize,
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
            for (d_num, n_id) in markers.iter() {
                if grab {
                    //this marker is the next one, it's begin position is our desired end position
                    end = self.positionmap.get(&(*d_num, *n_id)).map(|offset| {
                        offset
                            .begin
                            .try_into()
                            .expect("begin cursor must be beginaligned")
                    });
                    break;
                }
                if doc_num == *d_num && *n_id == node.id() {
                    //current node/marker found, signal grab for the next one
                    grab = true;
                }
            }
        };
        if end.is_none() {
            //no next marker found, use end of document instead
            end = Some(resource.textlen());
        }
        if let (Some(offset), Some(end)) = (self.positionmap.get(&(doc_num, node.id())), end) {
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
        resource: Option<&str>,
        inputfile: Option<&str>,
        doc_num: usize,
    ) -> Result<Cow<'t, str>, XmlConversionError> {
        if template.chars().any(|c| c == '{') {
            //value is a template, templating engine probably needed
            let compiled_template = self.template_engine.template(template);
            let context = self.context_for_node(&node, begin, end, template, resource, inputfile, doc_num);
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
        resource: Option<&str>,
        inputfile: Option<&str>,
        doc_num: usize,
    ) -> upon::Value {
        let mut context = self.global_context.clone();
        let length = if let (Some(begin), Some(end)) = (begin, end) {
            Some(end - begin)
        } else {
            None
        };
        context.insert("localname".into(), node.tag_name().name().into());
        //name with name prefix (if any)
        context.insert("name".into(), self.get_node_name_for_template(node).into());
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
        if let Some(resource) = resource {
            //the resource ID
            context.insert("resource".into(), resource.into());
        }
        if let Some(inputfile) = inputfile {
            //the input file
            context.insert("inputfile".into(), inputfile.into());
        }
        //document number (0-indexed), useful in case multiple input documents are cast to a single output text
        context.insert("doc_num".into(), upon::Value::Integer(doc_num as i64));

        if let Some(vars) = self.variables.get(template) {
            for var in vars {
                let mut encodedvar = String::new();
                if let Some(value) = self.context_for_var(node, var, &mut encodedvar) {
                    if value != upon::Value::None {
                        context.insert(encodedvar, value);
                    }
                }
            }
        }
        upon::Value::Map(context)
    }

    /// Looks up a variable value (from the DOM XML) to be used in for template context
    // returns value and stores full the *encoded* variable name in path (this is safe to pass to template)
    fn context_for_var<'input>(
        &self,
        node: &Node<'a, 'input>,
        var: &str, 
        path: &mut String,
    ) -> Option<upon::Value> {

        let first = path.is_empty();
        let var = 
        if var.starts_with("?.$") {
            if first {
                path.push_str("?.ELEMENT_");
            };
            &var[3..]
        } else if var.starts_with("$") {
            if first {
                path.push_str("ELEMENT_");
            };
            &var[1..]
        } else if var.starts_with("?.@") {
            if first {
                path.push_str("?.");
            };
            &var[2..]
        } else {
            var
        };

        if !first && !var.is_empty() {
            path.push_str("_IN_");
        }

        //get the first component of the variable
        let (component, remainder) = var.split_once("/").unwrap_or((var,""));
        //eprintln!("DEBUG: component={}", component);
        if component.is_empty() {
            //an empty component is the stop condition , this function is called recursively, stripping one
            //component at a time until nothing is left, we then take the text of that final node:
            Some(recursive_text(node).into())
        } else if component.starts_with("@"){
            if let Some(pos) = component.find(":") {
                let prefix = &component[1..pos];
                if let Some(ns) = self.config.namespaces.get(prefix) {
                    let var = &component[pos+1..];
                    path.push_str("ATTRIB_");
                    path.push_str(prefix);
                    path.push_str("__");
                    path.push_str(var);
                    Some(
                        node.attribute((ns.as_str(),var)).into()
                    )
                } else {
                    None
                }
            } else {
                let var = &component[1..];
                path.push_str("ATTRIB_");
                path.push_str(var);
                Some(
                    node.attribute(var).into()
                )
            }
        } else if component == ".." {
            if let Some(parentnode) = node.parent_element().as_ref() {
                //recurse with parent node
                path.push_str("PARENT");
                self.context_for_var(parentnode, remainder, path)
            } else {
                None
            }
        } else if component == "." {
            path.push_str("THIS");
            if !remainder.is_empty() {
                //a . is meaningless if not the final component
                self.context_for_var(node, remainder, path)
            } else {
                Some(recursive_text(node).into())
            }
        } else {
            let (prefix, localname)  = if let Some(pos) = component.find(":") {
                (Some(&component[0..pos]),  &component[pos+1..])
            } else {
                (None, component)
            };
            let localname_with_condition = localname;
            let (localname, condition_str, condition) = self.extract_condition(localname_with_condition); //extract X-Path like conditions [@attrib="value"]  (very limited!)
            //eprintln!("DEBUG: looking for {} (localname={}, condition={:?}) in {:?}", localname_with_condition,  localname, condition, node.tag_name());
            for child in node.children() {
                if child.is_element() {
                    let namedata = child.tag_name();
                    let mut child_matches = if let Some(namespace) = namedata.namespace() {
                        if let Some(foundprefix) = self.prefixes.get(namespace) {
                            Some(foundprefix.as_str()) == prefix && localname == namedata.name()
                        } else {
                            false
                        }
                    } else {
                        namedata.name() == localname
                    };
                    if child_matches {
                        //MAYBE TODO: move to separate funtion
                        if let Some((attribname, negate, attribvalue)) = condition {
                            //test condition: falsify child_matches
                            if let Some(pos) = attribname.find(":") {
                                let prefix = &attribname[0..pos];
                                if let Some(ns) = self.config.namespaces.get(prefix) {
                                    let attribname = &attribname[pos+1..];
                                    if let Some(value) = child.attribute((ns.as_str(),attribname)) {
                                        if !negate && attribvalue != Some(value) {
                                            child_matches = false;
                                        } else if negate && attribvalue == Some(value) {
                                            child_matches = false;
                                        }
                                    } else {
                                        child_matches = false;
                                    }
                                } else {
                                    child_matches = false;
                                }
                            } else {
                                if let Some(value) = child.attribute(attribname) {
                                    if !negate && attribvalue != Some(value) {
                                        child_matches = false;
                                    } else if negate && attribvalue == Some(value) {
                                        child_matches = false;
                                    }
                                } else {
                                    child_matches = false;
                                }
                            }
                        }
                        if !child_matches && self.config.debug {
                            eprintln!("[STAM fromxml] candidate node does not meet condition: {}", localname_with_condition);
                        }
                        //end condition test
                    }
                    if child_matches {
                        if let Some(prefix) = prefix {
                            path.push_str(prefix);
                            path.push_str("__");
                        }
                        path.push_str(localname);
                        if condition.is_some() {
                            //simply encode the condition as a hash (non-decodable but that's okay)
                            let mut hasher = DefaultHasher::new();
                            condition_str.hash(&mut hasher);
                            let h = hasher.finish();
                            path.push_str(&format!("_COND{}_", h));
                        }
                        return self.context_for_var(&child, remainder, path);
                    }
                }
            }
            //no match found for this variable
            None
        }
    }

    fn extract_condition<'b>(&self, localname: &'b str) -> (&'b str, &'b str, Option<(&'b str, bool, Option<&'b str>)>) { //(localname, condition, Option<(attrib, negation, attribvalue)>)
        //simple conditional statement
        if localname.ends_with("]") {
            if let Some(pos) = localname.find("[") {
                let condition = &localname[pos+1..localname.len()-1];
                let (mut attrib, negation, attribvalue) = if let Some(pos) = condition.find("=") {
                     let attrib = condition[0..pos].trim();
                     let value = condition[pos+1..].trim();
                     let value = &value[1..value.len() - 1]; //strips the literal quotes (") for the value
                     if attrib.ends_with('!') {
                        //negation (!= operator)
                        (attrib[..attrib.len() - 1].trim(), true, Some(value))
                     } else {
                        (attrib.trim(), false, Some(value))
                     }
                } else {
                    (condition, false, None)
                };
                if attrib.starts_with('@') {
                    //this should actually be mandatory and already checked during template precompilation
                    attrib = &attrib[1..];
                }
                return (&localname[..pos], condition, Some((attrib,  negation,attribvalue )) );
            }
        }
        (localname, "", None)
    }


    fn get_node_name_for_template<'b>(&self, node: &'b Node) -> Cow<'b,str> {
        let extended_name = node.tag_name();
        match (extended_name.namespace(), extended_name.name()) {
            (Some(namespace), tagname) => {
                if let Some(prefix) = self.prefixes.get(namespace) {
                    Cow::Owned(format!("{}__{}", prefix, tagname))
                } else {
                    Cow::Borrowed(tagname)
                }
            }
            (None, tagname) => Cow::Borrowed(tagname),
        }
    }

    fn get_node_name_for_xpath<'b>(&self, node: &'b Node) -> Cow<'b,str> {
        let extended_name = node.tag_name();
        match (extended_name.namespace(), extended_name.name()) {
            (Some(namespace), tagname) => {
                if let Some(prefix) = self.prefixes.get(namespace) {
                    Cow::Owned(format!("{}:{}", prefix, tagname))
                } else {
                    Cow::Borrowed(tagname)
                }
            }
            (None, tagname) => Cow::Borrowed(tagname),
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
        let mut in_condition = false;
        for (i,c) in s.char_indices() {
            if in_condition && c != ']' {
                continue;
            }
            if c == '"' {
                quoted = !quoted;
            } else if !quoted {
                if !var && (c == '@' || c == '$') {
                    //token is an XML variable name, its syntax needs some changes before it can be used in the templating engine
                    var = true;
                    begin = i;
                } else if var && c == '[' {
                    in_condition = true;
                } else if var && ((!c.is_alphanumeric() && c != '.' && c != '/' && c != '_' && c != ':' && c != '@') || (c == ']' && in_condition)) {
                    //end of variable (including condition if applicable)
                    if end < begin {
                        replacement.push_str(&s[end..begin]);
                    }
                    let varname = if c == ']' {
                        &s[begin..i+1]
                    } else {
                        &s[begin..i]
                    };
                    vars.insert(varname);
                    let replacement_var = self.precompile_name(varname);
                    replacement += &replacement_var;
                    var = false;
                    end = if c == ']' {
                        i + 1
                    } else {
                        i
                    };
                    in_condition = false;
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
        let mut begincondition = None;
        let mut skip = 0;
        for (i,c) in s.char_indices() {
            if begincondition.is_some() && c != ']' {
                continue;
            } else if skip > 0 {
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
            } else if c == '[' {
                begincondition = Some(i+1);
            } else if c == ']' {
                //conditions are just stored as hashes
                if let Some(begin) = begincondition {
                    let mut hasher = DefaultHasher::new();
                    let _ = &s[begin..i].hash(&mut hasher);
                    let h = hasher.finish();
                    replacement.push_str(&format!("_COND{}_", h));
                }
                begincondition = None;
            } else {
                replacement.push(c);
            }
        }
        //eprintln!("DEBUG: precompile_name({}) -> {}", s, replacement);
        replacement
    }

    fn add_metadata(&self, store: &mut AnnotationStore) -> Result<(), XmlConversionError> {
        for metadata in self.config.metadata.iter() {
            let mut builder = AnnotationBuilder::new();

            //prepare variables to pass to the template context
            if metadata.annotation == XmlAnnotationHandling::TextSelector {

            }
            let resource_id = if let Some(resource_handle) = self.resource_handle {
                store.resource(resource_handle).unwrap().id()
            } else {
                None
            };

            let mut context = self.global_context.clone();
            if let Some(resource_id) = resource_id {
                context.insert("resource".into(), resource_id.into());
            }

            if let Some(template) = &metadata.id {
                let compiled_template = self.template_engine.template(template.as_str());
                let id = compiled_template.render(&context).to_string().map_err(|e| 
                        XmlConversionError::TemplateError(
                            format!(
                                "whilst rendering metadata id template '{}'",
                                template,
                            ),
                            Some(e),
                        )
                    )?;
                if !id.is_empty() {
                    builder = builder.with_id(id);
                }
            }

            for annotationdata in metadata.annotationdata.iter() {
                let mut databuilder = AnnotationDataBuilder::new();
                if let Some(template) = &annotationdata.set {
                    let compiled_template = self.template_engine.template(template.as_str());
                    let dataset = compiled_template.render(&context).to_string().map_err(|e| 
                            XmlConversionError::TemplateError(
                                format!(
                                    "whilst rendering annotationdata/dataset template '{}' for metadata",
                                    template,
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
                    let compiled_template = self.template_engine.template(template.as_str());
                    match compiled_template.render(&context).to_string().map_err(|e| 
                            XmlConversionError::TemplateError(
                                format!(
                                    "whilst rendering annotationdata/key template '{}' for metadata",
                                    template,
                                ),
                                Some(e),
                            )
                        )  {
                        Ok(key) if !key.is_empty() =>
                            databuilder = databuilder.with_key(key.into()) ,
                        Ok(_) if !annotationdata.skip_if_missing => {
                            return Err(XmlConversionError::TemplateError(
                                format!(
                                    "whilst rendering annotationdata/key template '{}' metadata",
                                    template,
                                ),
                                None
                            ));
                        },
                        Err(e) if !annotationdata.skip_if_missing => {
                            return Err(e)
                        },
                        _ => {
                            //skip whole databuilder if missing
                            continue
                        }
                    }
                }
                if let Some(value) = &annotationdata.value {
                    match self.extract_value_metadata(value, &upon::Value::Map(context.clone()), annotationdata.allow_empty_value, annotationdata.skip_if_missing,  resource_id.as_deref())? {
                        Some(value) => {
                            databuilder = databuilder.with_value(value);
                        },
                        None =>  {
                            //skip whole databuilder if missing
                            continue
                        }
                    }
                }
                builder = builder.with_data_builder(databuilder);
            }



            // Finish the builder and add the actual annotation to the store, according to its element handling
            match metadata.annotation {
                XmlAnnotationHandling::TextSelector => {
                    // Annotation is on text, translates to TextSelector
                    builder = builder.with_target(SelectorBuilder::TextSelector(BuildItem::Handle(self.resource_handle.expect("resource must have handle")), Offset::whole()));
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   builder AnnotateText: {:?}", builder);
                    }
                    store.annotate(builder)?;
                }
                XmlAnnotationHandling::ResourceSelector  | XmlAnnotationHandling::None | XmlAnnotationHandling::Unspecified => {
                    // Annotation is metadata (default), translates to ResourceSelector
                    builder = builder.with_target(SelectorBuilder::ResourceSelector(
                        self.resource_handle.into(),
                    ));
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   builder AnnotateResource: {:?}", builder);
                    }
                    store.annotate(builder)?;
                }
                _ => panic!(
                    "Invalid annotationhandling for metadata: {:?}",
                    metadata.annotation
                ),
            }
        }
        Ok(())
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
    //use crate::info::info;

    const XMLSMALLEXAMPLE: &'static str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body><head><title>test</title></head><h1>TEST</h1><p xml:id="p1">This  is a <em xml:id="emphasis" style="color:green">test</em>.</p></body></html>"#;

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

    
    //fake example (not real HTML, testing TEI-like space attribute with complex template)
    const XMLTEISPACE: &'static str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body><space dim="vertical" unit="lines" quantity="3" /></body></html>"#;

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

#Not real HTML, test-case modelled after TEI space
[[elements]]
base = [ "common" ]
path = """//html:space[@dim="vertical" and @unit="lines"]"""
text = true
textsuffix = """\n{% for x in @quantity | int | as_range %}\n{% endfor %}"""

[[elements]]
base = [ "common", "text" ]
path = "//html:example"
annotation = "TextSelector"

[[elements.annotationdata]]
key = "requiredattrib"
value = "{{ @requiredattrib }}"

[[elements.annotationdata]]
key = "optattrib"
value = "{{ ?.@optattrib }}"

[[elements]]
base = [ "common","text" ]
path = "//html:marquee"
annotation = "TextSelector"

#map value, some bogus data to test parsing
[[elements.annotationdata]]
key = "map"

[elements.annotationdata.value]
text = "{{ $. }}"
number = 42
bogus = true
"#;

    const XMLREQATTRIBEXAMPLE: &'static str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body><example xml:id="ann1" requiredattrib="blah">test</example></body></html>"#;

    const XMLREQATTRIBEXAMPLE2: &'static str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body><example xml:id="ann1">test</example></body></html>"#;

    const XMLREQATTRIBEXAMPLE3: &'static str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body><example xml:id="ann1" requiredattrib="blah" optattrib="blah">test</example></body></html>"#;

    const XMLMAPEXAMPLE: &'static str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body><marquee xml:id="ann1">test</marquee></body></html>"#;

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
        assert_eq!(conv.config.elements.len(), 15, "number of elements");
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
        let annotation = store.annotation("emphasis").expect("annotation must have been created at this point");
        assert_eq!(annotation.text_simple(), Some("test"));
        //eprintln!("DEBUG: {:?}",annotation.data().collect::<Vec<_>>());
        let key = store.key("urn:stam-fromhtml", "style").expect("key must exist");
        assert_eq!(annotation.data().filter_key(&key).value_as_str(), Some("color:green"));
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

    #[test]
    fn test_teispace() -> Result<(), String> {
        let config = XmlConversionConfig::from_toml_str(CONF)?;
        let mut store = stam::AnnotationStore::new(stam::Config::new());
        from_xml_in_memory("test", XMLTEISPACE, &config, &mut store)?;
        let res = store.resource("test").expect("resource must have been created at this point");
        assert_eq!(res.text(), "\n\n\n\n", "resource text");
        Ok(())
    }


    #[test]
    fn test_reqattrib() -> Result<(), String> {
        let config = XmlConversionConfig::from_toml_str(CONF)?;
        let mut store = stam::AnnotationStore::new(stam::Config::new());
        from_xml_in_memory("test", XMLREQATTRIBEXAMPLE, &config, &mut store)?;
        let res = store.resource("test").expect("resource must have been created at this point");
        assert_eq!(res.text(), "test", "resource text");
        let key = store.key("urn:stam-fromhtml", "requiredattrib").expect("key must exist");
        let annotation = store.annotation("ann1").expect("annotation");
        assert_eq!(annotation.data().filter_key(&key).value_as_str(), Some("blah"));
        assert!(store.key("urn:stam-fromhtml", "optattrib").is_none(), "optional attrib is unused");
        Ok(())
    }

    #[test]
    fn test_reqattrib2() -> Result<(), String> {
        let mut config = XmlConversionConfig::from_toml_str(CONF)?;
        config = config.with_debug(true);
        let mut store = stam::AnnotationStore::new(stam::Config::new());
        assert!(from_xml_in_memory("test", XMLREQATTRIBEXAMPLE2, &config, &mut store).is_err(), "checking if error is returned");
        Ok(())
    }

    #[test]
    fn test_reqattrib3() -> Result<(), String> {
        let config = XmlConversionConfig::from_toml_str(CONF)?;
        let mut store = stam::AnnotationStore::new(stam::Config::new());
        from_xml_in_memory("test", XMLREQATTRIBEXAMPLE3, &config, &mut store)?;
        let res = store.resource("test").expect("resource must have been created at this point");
        assert_eq!(res.text(), "test", "resource text");
        let reqkey = store.key("urn:stam-fromhtml", "requiredattrib").expect("key must exist");
        let optkey = store.key("urn:stam-fromhtml", "optattrib").expect("key optattrib must exist");
        let annotation = store.annotation("ann1").expect("annotation");
        assert_eq!(annotation.data().filter_key(&reqkey).value_as_str(), Some("blah"));
        assert_eq!(annotation.data().filter_key(&optkey).value_as_str(), Some("blah"));
        Ok(())
    }

    #[test]
    fn test_map() -> Result<(), String> {
        let config = XmlConversionConfig::from_toml_str(CONF)?;
        let mut store = stam::AnnotationStore::new(stam::Config::new());
        from_xml_in_memory("test", XMLMAPEXAMPLE, &config, &mut store)?;
        let res = store.resource("test").expect("resource must have been created at this point");
        assert_eq!(res.text(), "test", "resource text");
        let key = store.key("urn:stam-fromhtml", "map").expect("key must exist");
        let annotation = store.annotation("ann1").expect("annotation");
        let data = annotation.data().filter_key(&key).value().expect("data must exist");
        if let DataValue::Map(data) = data {
            assert_eq!(data.get("text"), Some(&DataValue::String("test".into())));
            assert_eq!(data.get("number"), Some(&DataValue::Int(42)));
            assert_eq!(data.get("bogus"), Some(&DataValue::Bool(true)));
            assert_eq!(data.len(), 3);
        } else {
            assert!(false, "Data is supposed to be a map");
        }
        Ok(())
    }
}
