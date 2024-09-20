use std::collections::{HashMap, VecDeque};
use std::fmt::Display;
use std::fs::read_to_string;
use std::path::Path;

use roxmltree::{Attribute, Document, Node, NodeId, ParsingOptions};
use serde::Deserialize;
use stam::*;
use toml;

const NS_XML: &str = "http://www.w3.org/XML/1998/namespace";

#[derive(Deserialize)]
/// Holds the configuration for mapping a specific XML format to STAM
pub struct XmlConversionConfig {
    #[serde(default)]
    /// Holds configurations for mapping specific XML elements to STAM, evaluated in reverse-order, so put more generic rules before specific ones
    elements: Vec<XmlElementConfig>,

    #[serde(default)]
    /// Holds configurations for mapping specific XML attributes to STAM, evaluated in reverse-order, so put more generic rules before specific ones
    attributes: Vec<XmlAttributeConfig>,

    #[serde(default)]
    /// Maps XML prefixes to namespace
    namespaces: HashMap<String, String>,

    #[serde(default = "Whitespace::collapse")]
    /// Default whitespace handling
    whitespace: Whitespace,

    #[serde(default)]
    /// Inject a DTD (for XML entity resolution)
    inject_dtd: Option<String>,

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
            attributes: Vec::new(),
            namespaces: HashMap::new(),
            whitespace: Whitespace::Collapse,
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
    pub fn with_whitespace(mut self, handling: Whitespace) -> Self {
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

    /// Set an attribute configuration
    pub fn with_attribute<F>(mut self, scope_expression: &str, name: &str, setup: F) -> Self
    where
        F: Fn(XmlAttributeConfig) -> XmlAttributeConfig,
    {
        let expression = XPathExpression::new(scope_expression);
        let attribute = setup(XmlAttributeConfig::new(expression, name));
        if self.debug {
            eprintln!("[STAM fromxml] registered {:?}", attribute);
        }
        self.attributes.push(attribute);
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

    /// How to handle this attribute?
    fn attribute_config(&self, element: Node, attribute: Attribute) -> Option<&XmlAttributeConfig> {
        let nodepath: NodePath = element.into();
        for attributeconfig in self.attributes.iter().rev() {
            let (namespace, name) = attributeconfig.resolve(self);
            if (name == "*" || name == attribute.name())
                && (namespace == attribute.namespace() || name == "*")
                && attributeconfig.scope.test(&nodepath, self)
            {
                return Some(attributeconfig);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
/// Determines how to handle whitespace for an XML element
pub enum Whitespace {
    //Inherit from parent
    Inherit,
    /// Whitespace is kept as is in the XML
    Preserve,
    /// all whitespace becomes space, consecutive whitespace is squashed
    Collapse,
}

impl Default for Whitespace {
    fn default() -> Self {
        Whitespace::Inherit
    }
}

impl Whitespace {
    fn collapse() -> Self {
        Whitespace::Collapse
    }
}

#[derive(Debug, Clone, Deserialize)]
/// XML Element configuration, determines how to map an XML element (identified by an XPath expression) to STAM
pub struct XmlElementConfig {
    /// This is XPath-like expression (just a small subset of XPath) to identify an element by its path
    path: XPathExpression,

    #[serde(default)]
    /// This is the mode that determines how the element is handled
    handling: ElementHandling,

    #[serde(default)]
    /// Whitespace handling for this element
    whitespace: Whitespace,

    /// When extracting text, insert this text *before* the actual text (if any)
    /// It is often used for delimiters/whitspace/newlines.
    textprefix: Option<String>,

    /// When extracting text, insert this text *after* the actual text (if any)
    /// It is often used for delimiters/whitspace/newlines.
    textsuffix: Option<String>,

    /// The STAM set to translate this element to
    set: Option<String>,

    /// The STAM key to translate this element to
    key: Option<String>,

    /// The value to translate this element
    value: Option<String>,
}

impl XmlElementConfig {
    fn new(expression: XPathExpression) -> Self {
        Self {
            path: expression,
            handling: ElementHandling::AnnotateText,
            whitespace: Whitespace::Inherit,
            textprefix: None,
            textsuffix: None,

            set: None,
            key: None,
            value: None,
        }
    }

    /// This sets the mode that determines how the element is handled
    pub fn with_handling(mut self, handling: ElementHandling) -> Self {
        self.handling = handling;
        self
    }

    /// This sets the whitespace handling for this element
    pub fn with_whitespace(mut self, handling: Whitespace) -> Self {
        self.whitespace = handling;
        self
    }

    pub fn with_textprefix(mut self, textprefix: impl Into<String>) -> Self {
        self.textprefix = Some(textprefix.into());
        self
    }

    pub fn with_textsuffix(mut self, textsuffix: impl Into<String>) -> Self {
        self.textsuffix = Some(textsuffix.into());
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
        self.value = Some(value.into());
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
pub struct XmlAttributeConfig {
    #[serde(default = "XPathExpression::any")]
    scope: XPathExpression,

    name: String,

    #[serde(default)]
    handling: AttributeHandling,

    set: Option<String>,
    key: Option<String>,
}

impl XmlAttributeConfig {
    fn new(expression: XPathExpression, name: impl Into<String>) -> Self {
        Self {
            scope: expression,
            handling: AttributeHandling::KeyValue,
            name: name.into(),

            set: None,
            key: None,
        }
    }

    pub fn with_handling(mut self, handling: AttributeHandling) -> Self {
        self.handling = handling;
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

    /// get namespace and name
    pub fn resolve<'a>(&'a self, config: &'a XmlConversionConfig) -> (Option<&'a str>, &'a str) {
        if let Some((prefix, name)) = self.name.split_once(":") {
            if let Some(namespace) = config.namespaces.get(prefix).map(|x| x.as_str()) {
                (Some(namespace), name)
            } else {
                panic!(
                    "XML namespace prefix not known in configuration: {}",
                    prefix
                );
            }
        } else {
            (None, self.name.as_str())
        }
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
    standoff_textfiles: bool,
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
    let resource: TextResource = TextResourceBuilder::new()
        .with_id(textoutfilename.clone())
        .with_config(store.new_config().with_use_include(standoff_textfiles))
        .with_text(std::mem::replace(&mut converter.text, String::new()))
        .with_filename(&textoutfilename)
        .try_into()
        .map_err(|e| format!("Failed to build resource {}: {}", &textoutfilename, e))?;

    converter.resource_handle = Some(
        store
            .insert(resource)
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

    /// Keep track of the new positions (unicode offset) where the node starts in the untangled document
    positionmap: HashMap<NodeId, Offset>,

    /// Keep track of markers (XML elements with `ElementHandling::MarkersToTextSpan`), the key in this map is some hash of XmlElementConfig.
    markers: HashMap<usize, Vec<NodeId>>,

    /// The resource
    resource_handle: Option<TextResourceHandle>,

    /// Used to keep track of whether we need to insert a whitespace before actual text
    pending_whitespace: bool,

    /// The configuration
    config: &'a XmlConversionConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
/// This determines how an XML element translates to STAM.
pub enum ElementHandling {
    /// Skip this element and any text in it, do not create any annotations, but do descend into its child elements and their text
    PassThrough,

    /// Skip this element and all its descendants, it will be converted neither to text nor to annotations
    Exclude,

    /// Include the element text, discard the annotation, do descend into children
    ExtractTextOnly,

    /// Include this element's text and annotation, and descend into the children
    AnnotateText,

    /// Ignore any text, associate metadata with the resource
    AnnotateResource,

    /// Associate metadata with the resource, take the text of the element as data value
    /// This is useful for mapping XML metadata like `<author>John Doe</author>` to an annotation with datakey 'author' and value 'John Doe'.
    AnnotateResourceWithTextAsData,

    /// Annotate the text span from element until the next occurence of the same element.
    /// This can be used for example to convert <br/> elements annotations spanning whole lines.
    AnnotateBetweenMarkers,
}

impl Default for ElementHandling {
    fn default() -> Self {
        Self::AnnotateText
    }
}

impl ElementHandling {
    fn extract_text(&self) -> bool {
        match self {
            Self::AnnotateText | Self::ExtractTextOnly => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
/// This determines how an XML attribute translates to STAM.
pub enum AttributeHandling {
    /// Skip this attribute
    Exclude,

    /// Include this attribute as data ([`stam::AnnotationData`]) with corresponding key and value.
    KeyValue,

    /// Use this attribute as identifier for the annotation
    Identifier,

    /// Use this attribute to extract the text from (prior to any text in the element, but after the element's textprefix)
    ExtractTextFirst,

    /// Use this attribute to extract the text from (after any text in the element, but before the elements's textpostfix)
    ExtractTextAfter,

    /// Use this attribute value as key.
    /// This implies that there must be a single other attribute with [`AttributeHandling::Value`]
    Key,

    /// Use this attribute value as value.
    /// If there is another attribute with [`AttributeHandling::Key`], then that will be used as key,
    /// otherwise the element name itself will be used as key.
    Value,
}

impl Default for AttributeHandling {
    fn default() -> Self {
        Self::KeyValue
    }
}

impl<'a> XmlToStamConverter<'a> {
    fn new(config: &'a XmlConversionConfig) -> Self {
        Self {
            cursor: 0,
            text: String::new(),
            positionmap: HashMap::new(),
            markers: HashMap::new(),
            resource_handle: None,
            pending_whitespace: false,
            config,
        }
    }

    /// untangle text, extract the text (and only the text)
    /// from an XML document, according to the
    /// mapping configuration and creates a STAM TextResource for it.
    /// Records exact offsets per element/node for later use during annotation extraction.
    fn extract_element_text(
        &mut self,
        node: Node,
        whitespace: Whitespace,
    ) -> Result<(), StamError> {
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

            if element_config.handling != ElementHandling::Exclude
                && element_config.handling != ElementHandling::AnnotateBetweenMarkers
                && element_config.handling.extract_text()
            {
                //do text extraction for this element

                let whitespace = if node.has_attribute((NS_XML, "space")) {
                    // if there is an explicit xml:space attributes, it overrides whatever whitespace handling we have set:
                    match node.attribute((NS_XML, "space")).unwrap() {
                        "preserve" => Whitespace::Preserve,
                        "collapse" | "replace" => Whitespace::Collapse,
                        _ => whitespace,
                    }
                } else if element_config.whitespace == Whitespace::Inherit {
                    whitespace //from parent, i.e. passed to this (recursive) function by caller
                } else {
                    element_config.whitespace //default from the config
                };

                // process the text prefix, a preconfigured string of text to include prior to the actual text
                if let Some(textprefix) = &element_config.textprefix {
                    self.pending_whitespace = false;
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   outputting textprefix: {:?}", textprefix);
                    }
                    let (textprefix_len, textprefix_bytelen) = if has_variables(textprefix) {
                        let s = resolve_variables(textprefix, node);
                        self.text += &s;
                        (s.chars().count(), s.len())
                    } else {
                        self.text += textprefix;
                        (textprefix.chars().count(), textprefix.len())
                    };
                    self.cursor += textprefix_len;
                    // the textprefix will never be part of the annotation's text selection, increment the offsets:
                    begin += textprefix_len;
                    bytebegin += textprefix_bytelen;
                }

                // test if this element is configured to grab text from attributes
                let mut textfromattrib_after = None;
                for attribute in node.attributes() {
                    if let Some(attribute_config) = self.config.attribute_config(node, attribute) {
                        if attribute_config.handling == AttributeHandling::ExtractTextFirst {
                            if self.pending_whitespace {
                                self.text.push(' ');
                                self.pending_whitespace = false;
                                begin += 1;
                                bytebegin += 1;
                            }
                            firsttext = false;
                            self.text += attribute.value();
                        } else if attribute_config.handling == AttributeHandling::ExtractTextAfter {
                            //store for later output
                            textfromattrib_after = Some(attribute.value());
                        }
                    }
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
                        if whitespace == Whitespace::Collapse && !innertext.is_empty() {
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
                        if whitespace == Whitespace::Collapse {
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
                // was there text from an attribute we need to output still? then do so
                if let Some(textfromattrib) = textfromattrib_after {
                    self.text += textfromattrib;
                    self.cursor += textfromattrib.chars().count();
                }

                // process the text suffix, a preconfigured string of text to include after to the actual text
                if let Some(textsuffix) = &element_config.textsuffix {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   outputing textsuffix: {:?}", textsuffix);
                    }
                    let (end_discount_tmp, end_bytediscount_tmp) = if has_variables(textsuffix) {
                        let s = resolve_variables(textsuffix, node);
                        self.text += &s;
                        (s.chars().count(), s.len())
                    } else {
                        self.text += textsuffix;
                        (textsuffix.chars().count(), textsuffix.len())
                    };
                    // the textsuffix will never be part of the annotation's text selection, we substract a 'discount'
                    self.cursor += end_discount_tmp;
                    self.pending_whitespace = false;
                    end_discount = end_discount_tmp;
                    end_bytediscount = end_bytediscount_tmp;
                }
            } else if element_config.handling == ElementHandling::AnnotateBetweenMarkers {
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
    ) -> Result<(), StamError> {
        if self.config.debug {
            let path: NodePath = node.into();
            eprintln!("[STAM fromxml] extracting annotation from {}", path);
        }

        // obtain the configuration that applies to this element
        if let Some(element_config) = self.config.element_config(node) {
            if self.config.debug {
                eprintln!("[STAM fromxml]   matching config: {:?}", element_config);
            }
            if element_config.handling != ElementHandling::Exclude
                && element_config.handling != ElementHandling::ExtractTextOnly
                && element_config.handling != ElementHandling::PassThrough
            {
                let mut builder = AnnotationBuilder::new();

                if element_config.handling != ElementHandling::AnnotateResourceWithTextAsData {
                    // add annotation data that corresponds to the type of the element
                    builder =
                        builder.with_data_builder(self.translate_elementtype(node, element_config));
                }

                // these are needed for certain types of attribute handling where processing is deferred until are attributes are checked
                let mut set = None;
                let mut key = None;
                let mut value = None;

                for attribute in node.attributes() {
                    // obtain the configuration that applies to this attribute
                    if let Some(attribute_config) = self.config.attribute_config(node, attribute) {
                        if self.config.debug {
                            eprintln!(
                                "[STAM fromxml]   Extracting {:?} with config {:?}",
                                attribute, attribute_config
                            );
                        }
                        if attribute_config.handling == AttributeHandling::KeyValue {
                            // XML attribute name maps to a STAM key, XML attribute value to STAM datavalue
                            // (simplest one-to-one mapping)
                            builder = builder.with_data_builder(
                                self.translate_attribute(attribute, attribute_config),
                            );
                        } else if attribute_config.handling == AttributeHandling::Key {
                            // This attribute determines the STAM key, *another* attribute determines the value
                            // (so building data is deferred until both are found)
                            if set.is_none() && attribute_config.set.is_some() {
                                set = attribute_config.set.as_deref();
                            }
                            if key.is_some() {
                                eprintln!("[STAM fromxml] WARNING: A key attribute was already assigned, this one is ignored: {:?}", attribute);
                                continue;
                            }
                            key = Some(attribute);
                        } else if attribute_config.handling == AttributeHandling::Value {
                            // This attribute determines the STAM value, *another* attribute determines the key
                            // or if no such key element exists, the element type is used as key
                            // (so building data is deferred until both are found)
                            if set.is_none() && attribute_config.set.is_some() {
                                set = attribute_config.set.as_deref();
                            }
                            if value.is_some() {
                                eprintln!("[STAM fromxml] WARNING: A value attribute was already assigned, this one is ignored: {:?}", attribute);
                                continue;
                            }
                            value = Some(attribute);
                        } else if attribute_config.handling == AttributeHandling::Identifier {
                            // This attribute determines the ID of the annotation as a whole
                            // (note: there is no way to set the ID of AnnotationData in this converter)
                            if let Some(id_prefix) = &self.config.id_prefix {
                                if id_prefix.find("{resource}").is_some() {
                                    let resource_id = store
                                        .resource(
                                            self.resource_handle
                                                .expect("resource must have been created"),
                                        )
                                        .or_fail()?
                                        .id()
                                        .expect("resource must have ID");
                                    builder = builder.with_id(format!(
                                        "{}{}",
                                        &id_prefix.replace("{resource}", resource_id),
                                        attribute.value()
                                    ));
                                } else {
                                    builder = builder.with_id(attribute.value());
                                }
                            } else {
                                builder = builder.with_id(attribute.value());
                            }
                        }
                    } else {
                        eprintln!(
                            "[STAM fromxml]   WARNING: no match for attribute {} (skipped)",
                            attribute.name()
                        );
                    }
                }

                // add data in cases where processing was deferred due to use of AttributeHandling::Key and/or AttributeHandling::Value
                if let (Some(key), Some(value)) = (key, value) {
                    builder = builder.with_data_builder(self.translate_combine_attributes(
                        set,
                        key,
                        value,
                        element_config,
                    ))
                } else if let Some(value) = value {
                    builder = builder.with_data_builder(self.translate_combine_element_attribute(
                        set,
                        node,
                        value,
                        element_config,
                    ))
                }

                // Finish the builder and add the actual annotation to the store, according to its element handling
                match element_config.handling {
                    ElementHandling::AnnotateText => {
                        // Annotation is on text, translates to TextSelector
                        if let Some(selector) = self.textselector(node) {
                            builder = builder.with_target(selector);
                            if self.config.debug {
                                eprintln!("[STAM fromxml]   builder AnnotateText: {:?}", builder);
                            }
                            store.annotate(builder)?;
                        }
                    }
                    ElementHandling::AnnotateResource => {
                        // Annotation is metadata, translates to ResourceSelector
                        builder = builder.with_target(SelectorBuilder::ResourceSelector(
                            self.resource_handle.into(),
                        ));
                        if self.config.debug {
                            eprintln!("[STAM fromxml]   builder AnnotateResource: {:?}", builder);
                        }
                        store.annotate(builder)?;
                    }
                    ElementHandling::AnnotateResourceWithTextAsData => {
                        // Annotation is metadata, translates to ResourceSelector
                        if node.text().is_some() {
                            builder = builder.with_target(SelectorBuilder::ResourceSelector(
                                self.resource_handle.into(),
                            ));
                            builder = builder.with_data_builder(
                                self.translate_text_as_data(node, element_config),
                            );
                            if self.config.debug {
                                eprintln!(
                                    "[STAM fromxml]   builder AnnotateResourceWithTextAsData: {:?}",
                                    builder
                                );
                            }
                            store.annotate(builder).expect("annotation should succeed");
                        }
                    }
                    ElementHandling::AnnotateBetweenMarkers => {
                        // Annotation is on a text span *between* two marker elements
                        if let Some(selector) =
                            self.textselector_for_markers(node, store, element_config)
                        {
                            builder = builder.with_target(selector);
                            if self.config.debug {
                                eprintln!(
                                    "[STAM fromxml]   builder AnnotateBetweenMarkers: {:?}",
                                    builder
                                );
                            }
                            store.annotate(builder)?;
                        }
                    }
                    _ => panic!("Invalid elementhandling: {:?}", element_config.handling),
                }
            }

            // Recursion step
            if element_config.handling != ElementHandling::Exclude {
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
        attrib_config: &'b XmlAttributeConfig,
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

    /// translates an XML attribute like key=value  to an equivalent STAM annotationdata pair
    fn translate_elementtype<'b>(
        &self,
        node: Node<'b, 'b>,
        element_config: &'b XmlElementConfig,
    ) -> AnnotationDataBuilder<'b> {
        if let (Some(set), Some(key)) =
            (element_config.set.as_deref(), element_config.key.as_deref())
        {
            let builder = AnnotationDataBuilder::new()
                .with_dataset(set.into())
                .with_key(key.into());
            self.translate_element_value(builder, node, element_config)
        } else if let Some(namespace) = node.tag_name().namespace() {
            let builder = if let Some(set) = element_config.set.as_deref() {
                AnnotationDataBuilder::new()
                    .with_dataset(set.into())
                    .with_key(node.tag_name().name().into())
            } else {
                AnnotationDataBuilder::new()
                    .with_dataset(namespace.into())
                    .with_key(node.tag_name().name().into())
            };
            self.translate_element_value(builder, node, element_config)
        } else {
            let builder = AnnotationDataBuilder::new()
                .with_dataset("urn:stam-fromxml".into())
                .with_key(node.tag_name().name().into());
            self.translate_element_value(builder, node, element_config)
        }
    }

    fn translate_element_value<'b>(
        &self,
        builder: AnnotationDataBuilder<'b>,
        node: Node<'b, 'b>,
        element_config: &'b XmlElementConfig,
    ) -> AnnotationDataBuilder<'b> {
        if let Some(value) = &element_config.value {
            //string value may have a template we need to resolve
            builder.with_value(resolve_variables(value.as_str(), node).into())
        } else {
            builder.with_value(DataValue::Null)
        }
    }

    /// Can convert something like `<author value="John Doe">` to STAM annotationdata with key "author" and value "John Doe".
    fn translate_combine_element_attribute<'b>(
        &self,
        set: Option<&'b str>,
        node: Node<'b, 'b>,
        value: Attribute<'b, 'b>,
        element_config: &'b XmlElementConfig,
    ) -> AnnotationDataBuilder<'b> {
        if let Some(set) = set {
            AnnotationDataBuilder::new()
                .with_dataset(set.into())
                .with_key(node.tag_name().name().into())
                .with_value(value.value().into())
        } else if let Some(set) = element_config.set.as_deref() {
            AnnotationDataBuilder::new()
                .with_dataset(set.into())
                .with_key(node.tag_name().name().into())
                .with_value(value.value().into())
        } else {
            AnnotationDataBuilder::new()
                .with_dataset("urn:stam-fromxml".into())
                .with_key(node.tag_name().name().into())
                .with_value(value.value().into())
        }
    }

    /// Can convert something like `<meta name="author" content="John Doe">` to STAM annotationdata with key "author" and value "John Doe".
    fn translate_combine_attributes<'b>(
        &self,
        set: Option<&'b str>,
        key: Attribute<'b, 'b>,
        value: Attribute<'b, 'b>,
        element_config: &'b XmlElementConfig,
    ) -> AnnotationDataBuilder<'b> {
        if let Some(set) = set {
            AnnotationDataBuilder::new()
                .with_dataset(set.into())
                .with_key(key.value().into())
                .with_value(value.value().into())
        } else if let Some(set) = element_config.set.as_deref() {
            AnnotationDataBuilder::new()
                .with_dataset(set.into())
                .with_key(key.value().into())
                .with_value(value.value().into())
        } else {
            AnnotationDataBuilder::new()
                .with_dataset("urn:stam-fromxml".into())
                .with_key(key.value().into())
                .with_value(value.value().into())
        }
    }

    /// Maps XML text to STAM AnnotationData, i.e. the text is NOT used in actual text extraction but turns into data
    /// Useful for metadata elements like HTML <title>
    fn translate_text_as_data<'b>(
        &self,
        node: Node<'b, 'b>,
        element_config: &'b XmlElementConfig,
    ) -> AnnotationDataBuilder<'b> {
        let text = recursive_text(node);
        if let (Some(set), Some(key)) =
            (element_config.set.as_deref(), element_config.set.as_deref())
        {
            AnnotationDataBuilder::new()
                .with_dataset(set.into())
                .with_key(key.into())
                .with_value(text.into())
        } else if let Some(namespace) = node.tag_name().namespace() {
            if let Some(set) = element_config.set.as_deref() {
                AnnotationDataBuilder::new()
                    .with_dataset(set.into())
                    .with_key(node.tag_name().name().into())
                    .with_value(text.into())
            } else {
                AnnotationDataBuilder::new()
                    .with_dataset(namespace.into())
                    .with_key(node.tag_name().name().into())
                    .with_value(text.into())
            }
        } else {
            AnnotationDataBuilder::new()
                .with_dataset("urn:stam-fromxml".into())
                .with_key(node.tag_name().name().into())
                .with_value(text.into())
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

/// Tests if this string is a template that has variables that reference attributes using the `{@attrib}` syntax.
fn has_variables(s: &str) -> bool {
    while let Some(pos) = s.find("{@") {
        for c in s[pos..].chars() {
            if c.is_whitespace() {
                break;
            } else if c == '}' {
                return true;
            }
        }
    }
    false
}

/// resolve attribute variables in a string
fn resolve_variables(s: &str, node: Node) -> String {
    let mut out = String::new();
    let mut begin = None;
    for (bytepos, c) in s.char_indices() {
        if begin.is_some() {
            if c == '}' {
                let varname = &s[begin.unwrap() + 1..bytepos];
                if varname.starts_with('@') {
                    let varname = &varname[1..];
                    //TODO: handle namespaces prefixes
                    if let Some(value) = node.attribute(varname) {
                        out += value;
                    }
                    //(note: if not found it resolve to an empty string)
                    begin = None;
                }
            } else if c.is_whitespace() {
                //not a variable, flush buffer
                out += &s[begin.unwrap()..bytepos];
                begin = None;
            }
        } else if c == '{' {
            begin = Some(bytepos)
        } else {
            out.push(c);
        }
    }
    if let Some(begin) = begin {
        //flush remainder of buffer
        out += &s[begin..];
    }
    out
}
