use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::fs::{read_to_string, File};
use std::io::{BufRead, BufReader};

use roxmltree::{Attribute, Descendants, Document, Node, NodeId};
use stam::*;

pub struct XmlConversionConfig {
    elements: Vec<XmlElementConfig>,
    attributes: Vec<XmlAttributeConfig>,

    prefix_to_namespace: HashMap<String, String>,

    debug: bool,
}

impl XmlConversionConfig {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            attributes: Vec::new(),
            prefix_to_namespace: HashMap::new(),
            debug: true,
        }
    }

    pub fn with_debug(mut self, value: bool) -> Self {
        self.debug = value;
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>, namespace: impl Into<String>) -> Self {
        self.prefix_to_namespace
            .insert(prefix.into(), namespace.into());
        self
    }

    pub fn with_element<F>(mut self, expression: &str, setup: F) -> Result<Self, String>
    where
        F: Fn(XmlElementConfig) -> XmlElementConfig,
    {
        let expression = XPathExpression::new(expression, &self)?;
        let element = setup(XmlElementConfig::new(expression));
        if self.debug {
            eprintln!("[STAM fromxml] registered {:?}", element);
        }
        self.elements.push(element);
        Ok(self)
    }

    pub fn with_attribute<F>(
        mut self,
        expression: &str,
        namespace: &str,
        name: &str,
        setup: F,
    ) -> Result<Self, String>
    where
        F: Fn(XmlAttributeConfig) -> XmlAttributeConfig,
    {
        let expression = XPathExpression::new(expression, &self)?;
        let attribute = setup(XmlAttributeConfig::new(expression, namespace, name));
        if self.debug {
            eprintln!("[STAM fromxml] registered {:?}", attribute);
        }
        self.attributes.push(attribute);
        Ok(self)
    }

    /// How to handle this element?
    fn element_config(&self, node: Node) -> Option<&XmlElementConfig> {
        let nodepath: NodePath = node.into();
        for elementconfig in self.elements.iter().rev() {
            if elementconfig.expression.test(&nodepath) {
                return Some(elementconfig);
            }
        }
        None
    }

    /// How to handle this attribute?
    fn attribute_config(&self, node: Node, attribute: Attribute) -> Option<&XmlAttributeConfig> {
        let nodepath: NodePath = node.into();
        for attributeconfig in self.attributes.iter().rev() {
            if (attributeconfig.name.as_str() == "*"
                || attributeconfig.name.as_str() == attribute.name())
                && attributeconfig.namespace.as_str() == attribute.namespace().unwrap_or("")
                && attributeconfig.scope.test(&nodepath)
            {
                return Some(attributeconfig);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WhitespaceHandling {
    /// Whitespace is kept as is in the XML
    Preserve,
    /// all whitespace becomes space
    Replace,
    /// all whitespace becomes space, consecutive whitespace is squashed
    Collapse,
}

#[derive(Debug, Clone)]
pub struct XmlElementConfig {
    expression: XPathExpression,
    handling: ElementHandling,
    whitespace: WhitespaceHandling,
    textprefix: Option<String>,
    textsuffix: Option<String>,

    set: Option<String>,
    key: Option<String>,
    value: Option<DataValue>,
}

impl XmlElementConfig {
    fn new(expression: XPathExpression) -> Self {
        Self {
            expression,
            handling: ElementHandling::Include,
            whitespace: WhitespaceHandling::Preserve,
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

    pub fn with_whitespace(mut self, handling: WhitespaceHandling) -> Self {
        self.whitespace = handling;
        self
    }

    pub fn with_textprefix(mut self, textprefix: Option<String>) -> Self {
        self.textprefix = textprefix;
        self
    }

    pub fn with_textsuffix(mut self, textsuffix: Option<String>) -> Self {
        self.textsuffix = textsuffix;
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

#[derive(Debug, Clone)]
pub struct XmlAttributeConfig {
    scope: XPathExpression,
    namespace: String,
    name: String,
    handling: AttributeHandling,

    set: Option<String>,
    key: Option<String>,
}

impl XmlAttributeConfig {
    fn new(
        expression: XPathExpression,
        namespace: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            scope: expression,
            handling: AttributeHandling::Include,
            namespace: namespace.into(),
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
}

/// Not really full XPath, just a very minor subset
#[derive(Debug, Clone, PartialEq)]
struct XPathExpression {
    //(namespace,name) tuples
    components: Vec<(String, String)>,
}

impl XPathExpression {
    pub fn new(
        expression: impl Into<String>,
        config: &XmlConversionConfig,
    ) -> Result<Self, String> {
        let expression: String = expression.into();
        let mut components = Vec::new();
        for rawcomponent in expression.split("/") {
            if let Some((prefix, name)) = rawcomponent.split_once(":") {
                if let Some(namespace) = config.prefix_to_namespace.get(prefix) {
                    components.push((namespace.to_string(), name.to_string()));
                } else {
                    return Err(format!(
                        "XML namespace prefix not known in configuration: {}",
                        prefix
                    ));
                }
            } else {
                components.push((String::new(), rawcomponent.to_string()));
            }
        }
        Ok(Self { components })
    }

    /// matches a node path against an XPath-like expression
    fn test<'a, 'b>(&self, path: &NodePath<'a, 'b>) -> bool {
        let mut pathiter = path.components.iter().rev();
        for (refns, pat) in self.components.iter() {
            if let Some((ns, name)) = pathiter.next() {
                if pat != "*" && pat != "" {
                    if refns.is_empty() != ns.is_none()
                        || ns != &Some(refns.as_str())
                        || pat != *name
                    {
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
                write!(f, "{}:{}", ns, name)?;
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

impl Default for XmlConversionConfig {
    fn default() -> Self {
        Self::new()
            .with_prefix("xml", "http://www.w3.org/XML/1998/namespace")
            .with_prefix("xsd", "http://www.w3.org/2001/XMLSchema")
            .with_prefix("xhtml", "http://www.w3.org/1999/xhtml")
            .with_prefix("html", "http://www.w3.org/1999/xhtml")
            .with_prefix("xlink", "http://www.w3.org/1999/xlink")
            .with_prefix("xsi", "http://www.w3.org/2001/XMLSchema-instance")
            .with_prefix("xsl", "http://www.w3.org/1999/XSL/Transform")
            .with_prefix("tei", "http://www.tei-c.org/ns/1.0")
            .with_prefix("folia", "http://ilk.uvt.nl/folia")
            .with_attribute("*", "", "*", |a| a)
            .unwrap()
            .with_attribute("*", "http://www.w3.org/XML/1998/namespace", "id", |a| {
                a.with_handling(AttributeHandling::Identifier)
            })
            .unwrap()
            .with_element("*", |e| e)
            .unwrap()
    }
}

pub fn from_xml<'a>(
    filename: &str,
    config: &XmlConversionConfig,
    store: &'a mut AnnotationStore,
) -> Result<(), String> {
    if config.debug {
        eprintln!("[STAM fromxml] parsing {}", filename);
    }
    let xmlstring = read_to_string(filename)
        .map_err(|e| format!("Error opening XML file {}: {}", filename, e))?;

    let doc = Document::parse(&xmlstring)
        .map_err(|e| format!("Error parsing XML file {}: {}", filename, e))?;

    let mut converter = XmlToStamConverter::new(config);

    let textfilename = if filename.ends_with(".xml") {
        format!("{}.txt", &filename[0..filename.len() - 4])
    } else {
        format!("{}.txt", filename)
    };

    converter.extract_element_text(doc.root_element());
    if config.debug {
        eprintln!("[STAM fromxml] extracted full text: {}", &converter.text);
    }
    let resource: TextResource = TextResourceBuilder::new()
        .with_id(textfilename.clone())
        //.with_config(Config::default().with_use_include(true))
        .with_text(std::mem::replace(&mut converter.text, String::new()))
        //.with_filename(&textfilename)
        .try_into()
        .map_err(|e| format!("Failed to build resource {}: {}", &textfilename, e))?;

    converter.resource_handle = Some(
        store
            .insert(resource)
            .map_err(|e| format!("Failed to add resource {}: {}", &textfilename, e))?,
    );

    converter.extract_element_annotations(doc.root_element(), store);

    Ok(())
}

struct XmlToStamConverter<'a> {
    cursor: usize,
    text: String,

    /// Keep track of the new positions (unicode offset) where the node starts in the untangled document
    positionmap: HashMap<NodeId, Offset>,

    resource_handle: Option<TextResourceHandle>,

    config: &'a XmlConversionConfig,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ElementHandling {
    /// Skip this element and any text in it, but do descend into its child elements and their text
    PassThrough,

    /// Skip this element and all its descendants, it will be converted neither to text nor to annotations
    Exclude,

    /// Include the element text, discard the annotation
    IncludeTextOnly,

    /// Include this element's text and annotation, and descend into the children
    Include,

    /// Include this element, but do no further processing on any child elements (their text/annotation will be lost)
    IncludeWithoutChildren,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttributeHandling {
    /// Skip this attribute
    Exclude,

    /// Include this attribute as data
    Include,

    /// Use this attribute as identifier for the annotation
    Identifier,
}

impl<'a> XmlToStamConverter<'a> {
    fn new(config: &'a XmlConversionConfig) -> Self {
        Self {
            cursor: 0,
            text: String::new(),
            positionmap: HashMap::new(),
            resource_handle: None,
            config,
        }
    }
    /// untangle text
    fn extract_element_text(&mut self, node: Node) {
        if self.config.debug {
            let path: NodePath = node.into();
            eprintln!("[STAM fromxml] extracting text from {}", path);
        }
        let begin = self.cursor;
        let bytebegin = self.text.len();
        if let Some(element_config) = self.config.element_config(node) {
            if element_config.handling != ElementHandling::Exclude {
                if let Some(textprefix) = &element_config.textprefix {
                    self.text += textprefix;
                    self.cursor += textprefix.chars().count();
                }
                for child in node.children() {
                    if self.config.debug {
                        eprintln!("[STAM fromxml]   child {:?}", child);
                    }
                    if child.is_text() && element_config.handling != ElementHandling::PassThrough {
                        let innertext = child.text().expect("text node must have text");
                        self.text += &innertext;
                        self.cursor += innertext.chars().count();
                    } else if child.is_element() {
                        self.extract_element_text(child);
                    } else {
                        if self.config.debug {
                            eprintln!("[STAM fromxml]   skipping child node");
                        }
                        continue;
                    }
                    if let Some(textsuffix) = &element_config.textsuffix {
                        self.text += textsuffix;
                        self.cursor += textsuffix.chars().count();
                    }
                }
            }
        } else if self.config.debug {
            eprintln!(
                "[STAM fromxml]   WARNING: no match, skipping text extraction for element {}",
                NodePath::from(node)
            );
        }
        if begin < self.cursor {
            let offset = Offset::simple(begin, self.cursor);
            self.positionmap.insert(node.id(), offset);
            if self.config.debug {
                let path: NodePath = node.into();
                eprintln!(
                    "[STAM fromxml]   extracted text for {}: {:?}",
                    path,
                    &self.text[bytebegin..]
                );
            }
        }
    }

    fn extract_element_annotations(&mut self, node: Node, store: &mut AnnotationStore) {
        if let Some(selector) = self.selector(node) {
            if let Some(element_config) = self.config.element_config(node) {
                if element_config.handling == ElementHandling::Include {
                    let mut databuilders = Vec::new();

                    databuilders.push(self.translate_elementtype(node, element_config));

                    let mut annotation_id = None;
                    for attribute in node.attributes() {
                        if let Some(attribute_config) =
                            self.config.attribute_config(node, attribute)
                        {
                            if attribute_config.handling != AttributeHandling::Exclude {
                                if attribute_config.handling == AttributeHandling::Include {
                                    databuilders.push(self.translate_attribute(
                                        node,
                                        attribute,
                                        attribute_config,
                                    ));
                                } else if attribute_config.handling == AttributeHandling::Identifier
                                {
                                    annotation_id = Some(attribute.value().to_string())
                                }
                            }
                        } else if self.config.debug {
                            eprintln!(
                                "[STAM fromxml]   WARNING: no match for attribute {:?} (skipped)",
                                attribute
                            );
                        }
                    }

                    let mut builder = AnnotationBuilder::new().with_target(selector);
                    if let Some(id) = annotation_id {
                        builder = builder.with_id(id);
                    }
                    for databuilder in databuilders {
                        builder = builder.with_data_builder(databuilder);
                    }
                    store.annotate(builder).expect("annotation should succeed");
                }

                for child in node.children() {
                    if child.is_element() {
                        self.extract_element_annotations(child, store);
                    }
                }
            } else if self.config.debug {
                eprintln!(
                    "[STAM fromxml]   WARNING: no match, skipping annotation extraction for element {}",
                    NodePath::from(node)
                );
            }
        }

        for child in node.children() {
            if child.is_element() {
                self.extract_element_annotations(child, store);
            }
        }
    }

    fn translate_attribute<'b>(
        &self,
        node: Node,
        attribute: Attribute,
        attrib_config: &'b XmlAttributeConfig,
    ) -> AnnotationDataBuilder<'b> {
        let name = if let Some(namespace) = node.tag_name().namespace() {
            if let Some(prefix) = node.lookup_prefix(namespace) {
                format!("{}:{}", prefix, node.tag_name().name())
            } else if namespace.ends_with(['/', '#', '?', ':']) {
                format!("{}{}", namespace, node.tag_name().name())
            } else {
                format!("{}/{}", namespace, node.tag_name().name())
            }
        } else {
            node.tag_name().name().to_string()
        };
        AnnotationDataBuilder::new()
            .with_dataset(
                if let Some(set) = attrib_config.set.as_deref() {
                    set
                } else {
                    "urn:stam-fromxml"
                }
                .into(),
            )
            .with_key(name.into())
            .with_value(attribute.value().into())
    }

    fn translate_elementtype<'b>(
        &self,
        node: Node,
        element_config: &'b XmlElementConfig,
    ) -> AnnotationDataBuilder<'b> {
        let name = if let Some(namespace) = node.tag_name().namespace() {
            if let Some(prefix) = node.lookup_prefix(namespace) {
                format!("{}:{}", prefix, node.tag_name().name())
            } else if namespace.ends_with(['/', '#', '?', ':']) {
                format!("{}{}", namespace, node.tag_name().name())
            } else {
                format!("{}/{}", namespace, node.tag_name().name())
            }
        } else {
            node.tag_name().name().to_string()
        };
        if let Some(key) = element_config.key.as_deref() {
            AnnotationDataBuilder::new()
                .with_dataset(
                    if let Some(set) = element_config.set.as_deref() {
                        set
                    } else {
                        "urn:stam-fromxml"
                    }
                    .into(),
                )
                .with_key(key.into())
                .with_value(name.into())
        } else {
            //no key, we set the element type as key, and null as value
            AnnotationDataBuilder::new()
                .with_dataset(
                    if let Some(set) = element_config.set.as_deref() {
                        set
                    } else {
                        "urn:stam-fromxml"
                    }
                    .into(),
                )
                .with_key(name.into())
                .with_value(DataValue::Null)
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
