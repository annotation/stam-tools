# stam fromxml: mapping configuration 

The `stam fromxml` tool allows to map XML files with *inline annotations* to
STAM. It will effectively *untangle* the inline annotations and produce plain
text on the one hand, and stand-off STAM annotations on that plain text on the
other hand. From STAM, you can easily carry on with conversion to W3C Web Annotations.

The mapping is provided in an external configuration file which is passed to
`stam fromxml` via the `--config` parameter. The basic syntax for this configuration
file is [toml](https://toml.io/en/). Basic familiarity with toml is assumed,
the details of the actual mapping language are explained in this
document.

## Modelling choices

A mapping between XML and STAM depends on a variety of choices you as a modeller need to make:

* What parts of the XML go to the plain text and what parts go to annotations? For instance, is a `<title>` field something
  you want to explicitly render in the text or is it considered metadata (an annotation) on the text as a whole?
* How do you want to deal with the whitespace present in the XML? Preserve it or collapse it?
* How do you want to map XML elements and attributes to annotations and annotation data (i.e. set, keys and values)?
* If you intend to to export to W3C Web Annotations later, ensure you use full URIs for sets that correspond to the
RDF vocabularies you intend to use.

Because of all these choices and the flexible nature of STAM which allows you
to model things as you want, there is no "one single way" to map XML to STAM.

## Global configuration 

### inject_dtd

This key is used to inject a DTD (Document Type Definition) to the underlying XML processor. For the mapping, this is intended to specify a conversion from XML entities to their unicode values. Example:

```toml
inject_dtd = """<!DOCTYPE entities[<!ENTITY Tab "&#x9;"><!ENTITY NewLine "&#xA;"><!ENTITY excl "&#x21;"><!ENTITY quot "&#x22;">]>"""
```

If your XML document has any unknown entities that do not appear in the DTD, then an error will be raised and conversion will fail. 

### whitespace

Determines how to handle whitespace in XML. At the global level, this specifies
the default for the entire document, it can be overridden at element level.
Values are:

* `Preserve` - Whitespace is kept exactly as it is in the XML.
* `Collapse` - All whitespace is converted to a single space, and consecutive whitespace is squashed. You usually want to set this for the global option.

Example:

```toml
whitespace = "Collapse"
```

### default_set

This is the default STAM set to fall back to if no set is provided at a higher
level. It prevents unnecessary duplication and verbosity in your configuration
if you rely on the same set. If you need compatibility with W3C Web Annotation
export, this needs to be a URI.

Example:

```toml
default_set = "https://ns.tt.di.huc.knaw.nl/tei" 
```

If you don't set this nor specify it on deeper levels, you'll get a set named `urn:stam-fromxml`.

### namespaces

This is a toml table mapping namespace prefixes to XML namespaces. Example:

```toml
xml = "http://www.w3.org/XML/1998/namespace"
xsd = "http://www.w3.org/2001/XMLSchema"
xlink = "http://www.w3.org/1999/xlink"
tei = "http://www.tei-c.org/ns/1.0"
```

These prefixes can subsequently be used in path selectors and templates.

### id_strip_suffix

This is a list of suffixes to strip when forming identifiers for resources.
A common practise is to add the extension of your text files here:

```toml
id_strip_suffix = [ ".txt" ]
```

### elements

Elements is a list of tables, each entry defines how to map a certain XML
element, identified by an XPath-like path expression, to plain text and
annotations. This is the backbone of the conversion process. Elements should be
defined in order from generic to specific. So elements that are defined later
take precedence over those defined earlier (and therefore should be more
specific). A particular XML node is only matched once with its most appropriate configuration. 

The syntax for element configuration is explain in [its own section](#Element_configuration)

### context

Makes extra variables available globally for the templating engine.

Example:

```toml
[context]
foo = "bar"
x = 42
```

Additionally, you can load global variables from a file at run run time via
``--context-file``, this is either a toml or json file, the contents of the
former is the same as in the above example, but without `[context]`.

## Element configuration

### path

The path is an XPath-like expression that identifies what to match in the input document.
We say XPath-like, because only a relatively small subset of XPath is implemented.

Use `/` to match an absolute path. Namespace prefixes are allowed in expressions, as long as you defined them in the `namespaces` section. Example, this matches to root of a TEI document:

```toml
[[elements]]
path = "/tei:TEI"
```

A deeper path can look like this:

```toml
[[elements]]
path = "/tei:TEI/tei:teiHeader/tei:fileDesc/tei:titleStmt/tei:title"
```
Use `//` to match anywhere in the document hierarchy. Example, this matches all TEI paragraphs regardless where they occur:

```toml
[[elements]]
path = "//tei:p"
```

You can also use this mid-path, this is a more generic form of the expression we saw before:

```toml
[[elements]]
path = "/tei:TEI/tei:teiHeader//tei:title"
```

A wildcard is available to match all nodes:

```toml
[[elements]]
path = "*"
```

Or all nodes under a particular element:

```toml
[[elements]]
path = "//tei:note/*"
```

Limited conditional syntax is supported between square brackets. It can match an attribute:

```toml
[[elements]]
path = "//tei:item[@n=1]"
```

... or the immediate XML text that is directly under the element (no mixed content nodes):

```toml
[[elements]]
path = """//tei:item[text()="foobar"]"""
```

### text

Once an attribute is matched, this boolean specifies whether the text under it
should be extracted to the plain text representation. This defaults to `false`, 
effectively skipping the text directly under a node.

For example, the following defines an element match that will extract the text of all elements:

```toml
[[elements]]
path = "*"
text = true
```

### textprefix  & textsuffix

Whereas the `text` boolean extracts text from the XML text, the `textprefix` and `textsuffix` fields are used
to insert text *before* respectively *after* the extracted text. Consider the following example:

```toml
[[elements]]
path = "//tei:list/tei:item"
textprefix = "* "
text = true
textsuffix = "\n"
```

This rule extracts bulleted lists and inserts a bullet marker (`*` in this case) in the plain text variant. It also
ensures the extracted text ends with a newline. The latter is especially relevant if your whitespace handling is set to collapse. You'll often find yourself setting `textsuffix` to one or more newlines for different elements.

`textprefix` and `textsuffix` take not just string literals, but take *templates*. See [the templating section](#Templating_language). This allows for complex expressions like:

```toml
[[elements]]
path = "//tei:list/tei:item"
textprefix = "{% if ?.@n %}{{ @n }}.{% else %}*{% endif %} "
text = true
textsuffix = "\n"
```

### stop

This boolean determines whether child elements of this node will be processed or not. The default value
is `true`. By setting this to `false`, you can effectively ignore large parts from the XML input document.

For example, this ignores the entire header in a TEI document:

```toml
[[elements]]
path = "//tei:teiHeader"
stop = true
```

When you set `text = true` and `stop = true`, only the immediate text under the element will be extracted (no mixed content).

### whitespace

This determines the whitespace handling for this match, permitted values are:

* `Preserve` - Whitespace is kept exactly as it is in the XML.
* `Inherit` - Use the same setting as the parent node (this is the default)
* `Collapse` - All whitespace is converted to a single space, and consecutive whitespace is squashed. 


```toml
[[elements]]
path = "//html:pre"
whitespace = "Preserve"
```

### annotation

This field specifies whether you want to create an annotation and if so, of what type. It specifies the *target* of the annotation. The following values are implemented:

* `None` - No annotation (this is the default). 
* `TextSelector` - Create an annotation that points to the text that was extracted (this assumes `text = true`), using a `TextSelector`.
* `ResourceSelector` - Create an annotation that points to the text resource as a whole, as opposed to a specific offset. This is  often used to associate metadata on a document-level.
* `TextSelectorBetweenMarkers` - See [the section on Markers](#Markers).

### id 

This determines the ID of the annotation. This field supports [templating](#Templating_language) and you in fact rarely want to set it to a static value unless you are really sure it is a unique match in the input.

A common situation is to take the existing `xml:id` attribute and set it as the identifier for the annotation:

```toml
[[elements]]
path = "*"
id = "{{ ?.@xml:id }}"
```

Empty IDs will be automatically discarded. The `?.` prefix is used to not raise
an error on elements that have no `xml:id` attribute.

### annotationdata

This is a list of tables that specifies the actual data or body for the annotation. These are the key/value pairs of which we can have an arbitrary number per annotation. We assume you're familiar with the STAM's concept of *AnnotationData*  and
how it relates to annotations. If not, read up on it [here](https://github.com/annotation/stam#class-annotationdata).

`annotationdata` takes the following fields, most of them are [templates](#Templating_language) and allow you to refer to XML attributes and other nodes in the XML input document:

* `set` (template) - The STAM dataset for the annotationdata. Use a URI if exporting to W3C Web Annotations later. If not set, the global `default_set` will be used.
* `key` (template) - The STAM key, e.g. some property name as you want it to appear in the output.
* `value` (any type) - The value for the annotationdata. You can use any type here, including tables/maps/lists, note that all strings (at any depth) are interpreted as templates. 
* `skip_if_missing` (boolean) - If undefined variables are used in the template, silently skip this annotation data. Do not raise an error.
* `allow_empty_value` (boolean) - Even if the value is an empty string, allow that as a valid value.

A common situation is to copy from an XML attribute to STAM annotationdata, for example:

```toml
[[elements.annotationdata]]
key = "n"
value = "{{ @n }}"
skip_if_missing = true
```

Another common situation is to convert what was text in the XML to an annotation value on the resource (e.g. metadata). The variable ``$.`` can be used for this inside a template:

```toml
[[elements]]
path = "//tei:fileDesc/tei:titleStmt/tei:title"
annotation = "ResourceSelector"

[[elements.annotationdata]]
set = "http://www.tei-c.org/ns/1.0"
key = "title"
value = "{{$.}}"
skip_if_missing = true
```

Likewise, `$..` can be used to get the text of the parent element, and any other text path can be specified with `$node/child` syntax.

Another common use is to express the XML element type in annotations. The variable `localname` holds the tagname (stripped of any XML namespaces or prefixes):


```toml
[[elements]]
path = "*"

[[elements.annotationdata]]
key = "type"
value = "{{ localname }}"
```

Multiple `annotationdata` elements can be specified per element.

### base

Derives this element definition from a base element defined earlier. This prevents unnecessary repetition. See [base elements](#Base_elements).

Multiple are allowed.

## Templating language

The underlying templating syntax we use is as implemented in
[upon](https://docs.rs/upon/latest/upon/syntax/index.html). The syntax
shares many similarities with well-known templating systems such as [jinja2](https://jinja.palletsprojects.com/en/stable/):

* Value lookup and output is done using an expression in a double set of curly braces: `{{ x }}`.
* Blocks are available wrapped in `{%` and `%}`. For example: `{% if expression %}{% else %}{% endif %}` and
`{% for value in expression %}{% endfor %}`.

Peculiarities in our implementation:

* Variables for XML attributes start with `@`, for example: `{{ @n }}`
    * XML namespaces are supported: `{{ @xml:id }}`
* Variables for XML elements start with `$` and return the immediate text by default, for example: `{{ $child }}`
    * XML namespaces are supported: `{{ $prefix:child }}`
    * Only immediate text of the first match, no mixed content.
* The text of the current element is returned with: `{{ $. }}`.
    * It will contain the text of this node and all nodes under it recursively.
* If you want to return the attribute of a child element instead, combine `$` with `@`: 
    * Refer to the immediate text of a child element: `{{ $child@attrib }}` or `{{ $prefix:child@attrib }}`.
* Parent elements are denoted with `$..` and return the text:
    * It will contain the text of the parent node and all nodes under it recursively
    * Refer to an attribute: `{{ $../@attrib }}`.
* Use the `?.` prefix before a variable if you want to return an empty value if it does not exist, rather than raise an error, which would be the default: `{{ ?.@xml:id }}` or `{{ ?.$child }}`. If you set `skip_if_missing = true` then this is already implied.
* The following variables are available as well:
    * ``{{ resource }}`` -  the ID of the associated resource
    * ``{{ inputfile }}`` -  the path + filename of the input file (exactly as passed) 
    * ``{{ localname }}`` -  the tag name of the current node (without namespace) 
    * ``{{ namespace }}`` -  the namespace name of the current node (without tag)
    * ``{{ doc_num }}`` -  the document number (integer, 0-indexed), will only have a non-zero value in case of concatenating multiple XML input files to a single output text
    * ``{{ begin }}`` -  the begin offset (integer, 0-indexed) of the referenced text (integer)
    * ``{{ end }}`` -  the end offset (integer, non-inclusive) of the referenced text (integer)
    * ``{{ length }}`` -  the length (integer) of the referenced text (in unicode points)

### Filters

The following filters are implemented in the templating engine:

* `s | capitalize` - Converts first letter to uppercase
* `s | upper` - Converts everything to uppercase 
* `s | lower` - Converts everything to lowercase 
* `s | trim` - Strips whitespace (includes newlines)
* `x | first` - Returns the first element 
* `x | last` - Returns the last element 
* `s | tokenize` - Split on whitespace and newlines (consecutive whitespace is squashed)
* `x | eq: y` - equality testing
* `x | ne: y` - inequality testing
* `x | gt: y` - greater than for integers
* `x | lt:  y` - less than for integers
* `x | gte: y` - greater than or equal for integers
* `x | lte: y` - less than or equal for integers
* `x | int` - Converts a value (string, float) to an integer
* `x | as_range` - Converts an integer to a range of integers (starting with 1, ending with the number)
* `x | add: y` - addition (integers)
* `x | sub: y` - subtraction (integers)
* `x | mul: y` - multiplication (integers)
* `x | div: y` - division (integers)
* `s | replace: from, to` - replace a substring
* `s | basename` - Returns the base name of a file path
* `s | noext` - Strips the extension off a filename
* `s | starts_with: prefix` - Tests if a string starts with a given prefix (string)
* `s | ends_with: prefix` - Tests if a string ends with a given prefix (string)

Usage example:

```
{{ my_variable | trim }}
```

## Markers

The natural way in XML to mark a span of text is using some element that scopes over the text, i.e.:

```xml
<span>This is my text</span>
```

However, some XML formats use what we call *markers* or *milestones* to
indirectly demarcate spans. These are empty-elements that are repeated to mark
a span. Consider the `<tei:pb/>` to mark page breaks in TEI or `<br/>` to mark
line breaks in HTML. What if you want to annotate the text *between two
instances of the same marker element*? What if you want to mark *pages* or
*lines* respectively with such a XML input?

This conversion can be specified as follows:

```toml
[[elements]]
path = "//tei:pb"
annotation = "TextSelectorBetweenMarkers"
text = true
```

Now the annotation will not be made on the text underlying the element directly (because there isn't any),
but on the text that follows it until the next marker is encountered.

## Base elements

Remember that an input node only matches one elements rule. Often though, you have lots
of rules that may share a lot of aspects, such as common annotationdata specifications.

In order to prevent unnecessary duplication, you can specify *base elements* at
the global configuration level. These are effectively abstract elements or
templates (not to be confused with the templating syntax) upon which you can
base your element rules. By itself they do nothing, but you can use `base` on the
field in your element configuration to specify what base elements to derive
from.

Base elements are maps, here we define a base element called `withtype` that sets a type key corresponding to the element type and also converts some common XML attributes like `xml:id` and `xml:lang`:

```toml
[baseelements.withtype]
id = "{% if ?.@xml:id %}{{ @xml:id }}{% endif %}"

[[baseelements.withtype.annotationdata]]
key = "type"
value = "{{ localname }}"

[[baseelement.withtype.annotationdata]]
key = "lang"
value = "{{ @xml:lang }}"
skip_if_missing = true
```

You can now re-use this for your actual elements definitions as follows:


```toml
[[elements]]
path = "*" 
base = [ "withtype" ]
```

```toml
[[elements]]
path = "//html:p" 
base = [ "withtype" ]
```

You can derive from multiple base elements so you can mix and match, just take
note that fields are assigned on a first-come-first-serve basis in case there are
conflicting definitions.


## Metadata

Sometimes you want to place extra metadata annotations that are not directly
informed by the underlying XML, but are, for example, using information
injected via context variables (either from the configuration or using
`--context-file`).

This can be done in the `[[metadata]]` block. Underneath you can express
`[[metadata.annotationdata]]` which works similar to the `annotationdata` block
we have already seen.  You can also express `id` and `annotation` similar to
how they are used in `[[elements]]`.

Metadata annotations will by default be represented using a `ResourceSelector` on
the entire text resource that is produced, but you can also opt for a
`TextSelector` on the entire resource's text by setting `annotation =
TextSelector`.




