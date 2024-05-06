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
pub struct XmlConversionConfig {
    #[serde(default)]
    elements: Vec<XmlElementConfig>,

    #[serde(default)]
    attributes: Vec<XmlAttributeConfig>,

    #[serde(default)]
    /// Maps prefixes to namespace
    namespaces: HashMap<String, String>,

    #[serde(default = "Whitespace::collapse")]
    whitespace: Whitespace,

    #[serde(default)]
    inject_dtd: Option<String>,

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
            debug: false,
        }
    }

    pub fn from_toml_str(tomlstr: &str) -> Result<Self, String> {
        toml::from_str(tomlstr).map_err(|e| format!("{}", e))
    }

    pub fn with_debug(mut self, value: bool) -> Self {
        self.debug = value;
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>, namespace: impl Into<String>) -> Self {
        self.namespaces.insert(prefix.into(), namespace.into());
        self
    }

    pub fn with_inject_dtd(mut self, dtd: impl Into<String>) -> Self {
        self.inject_dtd = Some(dtd.into());
        self
    }

    /// Set default whitespace handling
    pub fn with_whitespace(mut self, handling: Whitespace) -> Self {
        self.whitespace = handling;
        self
    }

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

    pub fn with_attribute<F>(mut self, expression: &str, name: &str, setup: F) -> Self
    where
        F: Fn(XmlAttributeConfig) -> XmlAttributeConfig,
    {
        let expression = XPathExpression::new(expression);
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
pub struct XmlElementConfig {
    path: XPathExpression,

    #[serde(default)]
    handling: ElementHandling,

    #[serde(default)]
    whitespace: Whitespace,
    textprefix: Option<String>,
    textsuffix: Option<String>,

    set: Option<String>,
    key: Option<String>,
    value: Option<DataValue>,
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

    pub fn with_handling(mut self, handling: ElementHandling) -> Self {
        self.handling = handling;
        self
    }

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

    pub fn with_value(mut self, value: impl Into<DataValue>) -> Self {
        self.value = Some(value.into());
        self
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

pub fn from_xml<'a>(
    filename: &Path,
    config: &XmlConversionConfig,
    store: &'a mut AnnotationStore,
    standoff_textfiles: bool,
) -> Result<(), String> {
    if config.debug {
        eprintln!("[STAM fromxml] parsing {}", filename.display());
    }

    let mut xmlstring = read_to_string(filename)
        .map_err(|e| format!("Error opening XML file {}: {}", filename.display(), e))?;

    //patchy: remove HTML5 doctype and inject our own
    if xmlstring[..100].find("<!DOCTYPE html>").is_some() && config.inject_dtd.is_some() {
        xmlstring = xmlstring.replacen("<!DOCTYPE html>", "", 1);
    }

    if xmlstring[..100].find("<!DOCTYPE").is_none() {
        if let Some(dtd) = config.inject_dtd.as_ref() {
            xmlstring = dtd.to_string() + &xmlstring
        };
    }

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

    converter.extract_element_text(doc.root_element(), converter.config.whitespace);
    if config.debug {
        eprintln!("[STAM fromxml] extracted full text: {}", &converter.text);
    }
    let resource: TextResource = TextResourceBuilder::new()
        .with_id(textoutfilename.clone())
        .with_config(Config::default().with_use_include(standoff_textfiles))
        .with_text(std::mem::replace(&mut converter.text, String::new()))
        .with_filename(&textoutfilename)
        .try_into()
        .map_err(|e| format!("Failed to build resource {}: {}", &textoutfilename, e))?;

    converter.resource_handle = Some(
        store
            .insert(resource)
            .map_err(|e| format!("Failed to add resource {}: {}", &textoutfilename, e))?,
    );

    converter.extract_element_annotation(doc.root_element(), store);

    Ok(())
}

struct XmlToStamConverter<'a> {
    cursor: usize,
    text: String,

    /// Keep track of the new positions (unicode offset) where the node starts in the untangled document
    positionmap: HashMap<NodeId, Offset>,

    resource_handle: Option<TextResourceHandle>,

    pending_whitespace: bool,

    config: &'a XmlConversionConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
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
pub enum AttributeHandling {
    /// Skip this attribute
    Exclude,

    /// Include this attribute as data with corresponding key and value.
    KeyValue,

    /// Use this attribute as identifier for the annotation
    Identifier,

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
            resource_handle: None,
            pending_whitespace: false,
            config,
        }
    }

    /// untangle text
    fn extract_element_text(&mut self, node: Node, whitespace: Whitespace) {
        if self.config.debug {
            let path: NodePath = node.into();
            eprintln!("[STAM fromxml] extracting text from {}", path);
        }
        let mut begin = self.cursor;
        let mut bytebegin = self.text.len();
        let mut end_discount = 0;
        let mut end_bytediscount = 0;
        let mut firsttext = true;
        if let Some(element_config) = self.config.element_config(node) {
            if self.config.debug {
                eprintln!("[STAM fromxml]   matching config: {:?}", element_config);
            }
            if element_config.handling != ElementHandling::Exclude
                && element_config.handling.extract_text()
            {
                let whitespace = if node.has_attribute((NS_XML, "space")) {
                    match node.attribute((NS_XML, "space")).unwrap() {
                        "preserve" => Whitespace::Preserve,
                        "collapse" | "replace" => Whitespace::Collapse,
                        _ => whitespace,
                    }
                } else if element_config.whitespace == Whitespace::Inherit {
                    whitespace
                } else {
                    element_config.whitespace
                };
                if let Some(textprefix) = &element_config.textprefix {
                    self.pending_whitespace = false;
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   outputting textprefix: {:?}", textprefix);
                    }
                    self.text += textprefix;
                    let textprefix_len = textprefix.chars().count();
                    self.cursor += textprefix_len;
                    // the textprefix will never be part of the annotation's text selection:
                    begin += textprefix_len;
                    bytebegin += textprefix.len();
                }
                for child in node.children() {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   child {:?}", child);
                    }
                    if child.is_text() {
                        let mut innertext = child.text().expect("text node must have text");
                        let mut pending_whitespace = false;
                        let mut leading_whitespace = false;
                        if whitespace == Whitespace::Collapse && !innertext.is_empty() {
                            let mut all_whitespace = true;
                            leading_whitespace = innertext.chars().next().unwrap().is_whitespace();
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
                            //output pending whitespace
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
                        self.extract_element_text(child, whitespace);
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
                if let Some(textsuffix) = &element_config.textsuffix {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   outputing textsuffix: {:?}", textsuffix);
                    }
                    self.text += textsuffix;
                    // the textsuffix will never be part of the annotation's text selection:
                    end_discount = textsuffix.chars().count();
                    self.cursor += end_discount;
                    self.pending_whitespace = false;
                    end_bytediscount = textsuffix.len();
                }
            }
        } else if self.config.debug {
            eprintln!(
                "[STAM fromxml]   WARNING: no match, skipping text extraction for element {}",
                NodePath::from(node)
            );
        }
        if begin < (self.cursor - end_discount) {
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
    }

    fn extract_element_annotation(&mut self, node: Node, store: &mut AnnotationStore) {
        if self.config.debug {
            let path: NodePath = node.into();
            eprintln!("[STAM fromxml] extracting annotation from {}", path);
        }
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
                    builder =
                        builder.with_data_builder(self.translate_elementtype(node, element_config));
                }

                let mut set = None;
                let mut key = None;
                let mut value = None;
                for attribute in node.attributes() {
                    if let Some(attribute_config) = self.config.attribute_config(node, attribute) {
                        if self.config.debug {
                            eprintln!(
                                "[STAM fromxml]   Extracting {:?} with config {:?}",
                                attribute, attribute_config
                            );
                        }
                        if attribute_config.handling == AttributeHandling::KeyValue {
                            builder = builder.with_data_builder(
                                self.translate_attribute(attribute, attribute_config),
                            );
                        } else if attribute_config.handling == AttributeHandling::Key {
                            if set.is_none() && attribute_config.set.is_some() {
                                set = attribute_config.set.as_deref();
                            }
                            if key.is_some() {
                                eprintln!("[STAM fromxml] WARNING: A key attribute was already assigned, this one is ignored: {:?}", attribute);
                                continue;
                            }
                            key = Some(attribute);
                        } else if attribute_config.handling == AttributeHandling::Value {
                            if set.is_none() && attribute_config.set.is_some() {
                                set = attribute_config.set.as_deref();
                            }
                            if value.is_some() {
                                eprintln!("[STAM fromxml] WARNING: A value attribute was already assigned, this one is ignored: {:?}", attribute);
                                continue;
                            }
                            value = Some(attribute);
                        } else if attribute_config.handling == AttributeHandling::Identifier {
                            builder = builder.with_id(attribute.value());
                        }
                    } else {
                        eprintln!(
                            "[STAM fromxml]   WARNING: no match for attribute {} (skipped)",
                            attribute.name()
                        );
                    }
                }

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

                match element_config.handling {
                    ElementHandling::AnnotateText => {
                        if let Some(selector) = self.selector(node) {
                            builder = builder.with_target(selector);
                            if self.config.debug {
                                eprintln!("[STAM fromxml]   builder AnnotateText: {:?}", builder);
                            }
                            store.annotate(builder).expect("annotation should succeed");
                        }
                    }
                    ElementHandling::AnnotateResource => {
                        builder = builder.with_target(SelectorBuilder::ResourceSelector(
                            self.resource_handle.into(),
                        ));
                        if self.config.debug {
                            eprintln!("[STAM fromxml]   builder AnnotateResource: {:?}", builder);
                        }
                        store.annotate(builder).expect("annotation should succeed");
                    }
                    ElementHandling::AnnotateResourceWithTextAsData => {
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
                    _ => panic!("Invalid elementhandling: {:?}", element_config.handling),
                }
            }

            if element_config.handling != ElementHandling::Exclude {
                for child in node.children() {
                    if child.is_element() {
                        self.extract_element_annotation(child, store);
                    }
                }
            }
        } else {
            eprintln!(
                "[STAM fromxml]   WARNING: no match, skipping annotation extraction for element {}",
                NodePath::from(node)
            );
        }
    }

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
            AnnotationDataBuilder::new()
                .with_dataset(set.into())
                .with_key(key.into())
                .with_value(node.tag_name().name().into())
        } else if let Some(namespace) = node.tag_name().namespace() {
            if let Some(set) = element_config.set.as_deref() {
                AnnotationDataBuilder::new()
                    .with_dataset(set.into())
                    .with_key(node.tag_name().name().into())
                    .with_value(DataValue::Null)
            } else {
                AnnotationDataBuilder::new()
                    .with_dataset(namespace.into())
                    .with_key(node.tag_name().name().into())
                    .with_value(DataValue::Null)
            }
        } else {
            AnnotationDataBuilder::new()
                .with_dataset("urn:stam-fromxml".into())
                .with_key(node.tag_name().name().into())
                .with_value(DataValue::Null)
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

    fn selector(&self, node: Node) -> Option<SelectorBuilder> {
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
