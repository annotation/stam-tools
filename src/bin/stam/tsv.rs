use clap::Arg;
use stam::{
    Annotation, AnnotationData, AnnotationDataSet, AnnotationStore, AnyId, DataKey, DataValue,
    Storable, TextResource, TextResourceHandle, TextSelection,
};
use std::fmt;
use std::process::exit;

pub fn tsv_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("null")
            .long("null")
            .help("Text to use for NULL values")
            .takes_value(true)
            .default_value("-"),
    );
    args.push(
        Arg::with_name("delimiter")
            .long("delimiter")
            .help("Delimiter for multiple values in a single column")
            .takes_value(true)
            .default_value("|"),
    );
    args.push(
        Arg::with_name("columns")
            .long("columns")
            .short('C')
            .help("Column Format, comma separated list of column names to output")
            .long_help(
                "Choose from the following known columns names (case insensitive):

* Type                 - Outputs the type of the row (Annotation,AnnotationData), useful in Nested mode.
* Id                   - Outputs the ID of the row item
* Annotation           - Outputs the ID of the associated Annotation
* AnnotationData       - Outputs the ID of the associated AnnotationData
* AnnotationDataSet    - Outputs the ID of the associated AnnotationDataSet
* TextResource         - Output the associated resource identifier
* DataKey              - Outputs the ID of the associated DataKey
* DataValue            - Outputs the data value 
* TextSelection        - Outputs any associated text selection(s) as a combination of resource identifier(s) with an offset
* Text                 - Outputs the associated text
* Offset               - Outputs offset pair in unicode character points (0-indexed, end is non-inclusive)
* BeginOffset          - Outputs begin offset in unicode character points
* EndOffset            - Outputs end offset in unicode character points
* Utf8Offset           - Outputs offset pair in UTF-8 bytes  (0-indexed, end is non inclusive)
* BeginUtf8Offset      - Outputs begin offset in UTF-8 bytes
* EndUtf8Offset        - Outputs end offset in UTF8-bytes
* Ignore               - Always outputs the NULL value
",
            )
            .takes_value(true)
            .default_value("Type,Id,AnnotationDataSet,DataKey,DataValue,Text,TextSelection"),
    );
    args.push(
        Arg::with_name("type")
            .long("type")
            .help("Select the data type to focus on for the TSV output.")
            .long_help(
                "Choose one from the following types (case insensitive):

* Annotation
* AnnotationData
* AnnotationDataSet
* DataKey
* TextResource
* TextSelection
",
            )
            .takes_value(true)
            .default_value("Annotation"),
    );
    args.push(
        Arg::with_name("no-header")
            .long("no-header")
            .short('H')
            .help("Do not output a header on the first line")
            .takes_value(false),
    );
    args
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Column {
    Type,
    Id,
    Annotation,
    TextResource,
    AnnotationData,
    AnnotationDataSet,
    Offset,
    BeginOffset,
    EndOffset,
    Utf8Offset,
    BeginUtf8Offset,
    EndUtf8Offset,
    DataKey,
    DataValue,
    Text,
    TextSelection,
    Ignore,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Type {
    Annotation,
    AnnotationDataSet,
    AnnotationData,
    DataKey,
    TextResource,
    TextSelection,
}

impl TryFrom<&str> for Type {
    type Error = String;
    fn try_from(val: &str) -> Result<Self, Self::Error> {
        let val_lower = val.to_lowercase();
        match val_lower.as_str() {
            "annotation" | "annotations" => Ok(Self::Annotation),
            "annotationdataset" | "dataset" | "annotationset" | "annotationdatasets"
            | "datasets" | "annotationsets" => Ok(Self::AnnotationDataSet),
            "data" | "annotationdata" | "datavalue" | "datavalues" => Ok(Self::AnnotationData),
            "datakey" | "datakeys" | "key" | "keys" => Ok(Self::DataKey),
            "resource" | "textresource" | "resources" | "textresources" => Ok(Self::TextResource),
            "textselection" | "textselections" => Ok(Self::TextSelection),
            _ => Err(format!(
                "Unknown type: {}, see --help for allowed values",
                val
            )),
        }
    }
}

impl Type {
    fn as_str(&self) -> &str {
        match self {
            Self::Annotation => "Annotation",
            Self::AnnotationData => "AnnotationData",
            Self::AnnotationDataSet => "AnnotationDataSet",
            Self::DataKey => "DataKey",
            Self::TextResource => "TextResource",
            Self::TextSelection => "TextSelection",
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<&str> for Column {
    type Error = String;
    fn try_from(val: &str) -> Result<Self, Self::Error> {
        let val_lower = val.to_lowercase();
        match val_lower.as_str() {
            "type" => Ok(Self::Type),
            "id" => Ok(Self::Id),
            "annotationid" | "annotation" => Ok(Self::Annotation),
            "annotationdatasetid"
            | "annotationdataset"
            | "set"
            | "setid"
            | "datasetid"
            | "dataset" => Ok(Self::AnnotationDataSet),
            "resource" | "resourceid" | "textresource" | "textresources" => Ok(Self::TextResource),
            "annotationdataid" | "dataid" => Ok(Self::AnnotationData),
            "offset" => Ok(Self::Offset),
            "beginoffset" | "begin" | "start" | "startoffset" => Ok(Self::BeginOffset),
            "endoffset" | "end" => Ok(Self::EndOffset),
            "utf8offset" => Ok(Self::Utf8Offset),
            "beginutf8offset" | "beginutf8" | "beginbyte" | "startbyte" | "startutf8"
            | "startutf8offset" => Ok(Self::BeginUtf8Offset),
            "endutf8offset" | "endutf8" | "endbyte" => Ok(Self::EndUtf8Offset),
            "datakey" | "key" | "datakeyid" | "keyid" => Ok(Self::DataKey),
            "datavalue" | "value" => Ok(Self::DataValue),
            "text" => Ok(Self::Text),
            "textselections" | "textselection" => Ok(Self::TextSelection),
            "ignore" => Ok(Self::Ignore),
            _ => Err(format!(
                "Unknown column: {}, see --help for allowed values",
                val
            )),
        }
    }
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone)]
struct Context<'a> {
    id: Option<&'a str>,
    textselections: Option<&'a Vec<(TextResourceHandle, TextSelection)>>,
    textselections_text: Option<&'a Vec<&'a str>>,
    textselection: Option<&'a TextSelection>,
    text: Option<&'a str>,
    annotation: Option<&'a Annotation>,
    data: Option<&'a AnnotationData>,
    resource: Option<&'a TextResource>,
    set: Option<&'a AnnotationDataSet>,
    key: Option<&'a DataKey>,
    value: Option<&'a DataValue>,
}

impl<'a> Default for Context<'a> {
    fn default() -> Self {
        Context {
            id: None,
            textselections: None,      //multiple
            textselections_text: None, //text for possibly multiple textselections
            textselection: None,       //single one
            text: None,                //single text reference
            annotation: None,
            data: None,
            resource: None,
            set: None,
            key: None,
            value: None,
        }
    }
}

impl Column {
    fn as_str(&self) -> &str {
        match self {
            Self::Type => "Type",
            Self::Id => "Id",
            Self::Annotation => "Annotation",
            Self::TextResource => "TextResource",
            Self::AnnotationData => "AnnotationData",
            Self::AnnotationDataSet => "AnnotationDataSet",
            Self::Offset => "Offset",
            Self::BeginOffset => "BeginOffset",
            Self::EndOffset => "EndOffset",
            Self::Utf8Offset => "Utf8Offset",
            Self::BeginUtf8Offset => "BeginUtf8Offset",
            Self::EndUtf8Offset => "EndUtf8Offset",
            Self::DataKey => "DataKey",
            Self::DataValue => "DataValue",
            Self::Text => "Text",
            Self::TextSelection => "TextSelection",
            Self::Ignore => "Ignore",
        }
    }

    fn print(
        &self,
        store: &AnnotationStore,
        tp: Type,
        colnr: usize,
        col_len: usize,
        context: &Context,
        delimiter: &str,
        null: &str,
    ) {
        if colnr > 0 {
            print!("\t");
        }
        match self {
            Column::Type => print!("{}", tp.as_str()),
            Column::Id => print!("{}", context.id.unwrap_or(null)),
            Column::TextSelection => {
                if context.textselection.is_some() && context.resource.is_some() {
                    print!(
                        "{}#{}-{}",
                        context.resource.unwrap().id().unwrap_or(""),
                        context.textselection.unwrap().begin(),
                        context.textselection.unwrap().end(),
                    );
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|(reshandle, t)| {
                                let resource = store
                                    .resource(&AnyId::from(*reshandle))
                                    .expect("resource must exist");
                                format!("{}#{}-{}", resource.id().unwrap_or(""), t.begin(), t.end())
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::Offset => {
                if let Some(textselection) = context.textselection {
                    print!("{}-{}", textselection.begin(), textselection.end(),);
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|(_reshandle, t)| { format!("{}-{}", t.begin(), t.end()) })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::BeginOffset => {
                if let Some(textselection) = context.textselection {
                    print!("{}", textselection.begin());
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|(_reshandle, t)| { format!("{}", t.begin()) })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::EndOffset => {
                if let Some(textselection) = context.textselection {
                    print!("{}", textselection.end());
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|(_reshandle, t)| { format!("{}", t.end()) })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::Utf8Offset => {
                if let (Some(textselection), Some(resource)) =
                    (context.textselection, context.resource)
                {
                    print!(
                        "{}-{}",
                        resource
                            .utf8byte(textselection.begin())
                            .expect("Offset must be valid"),
                        resource
                            .utf8byte(textselection.end())
                            .expect("Offset must be valid"),
                    );
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|(reshandle, t)| {
                                let resource = store
                                    .resource(&AnyId::from(*reshandle))
                                    .expect("resource must exist");
                                format!(
                                    "{}-{}",
                                    resource.utf8byte(t.begin()).expect("offset must be valid"),
                                    resource.utf8byte(t.end()).expect("offset must be valid"),
                                )
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::BeginUtf8Offset => {
                if let (Some(textselection), Some(resource)) =
                    (context.textselection, context.resource)
                {
                    print!(
                        "{}",
                        resource
                            .utf8byte(textselection.begin())
                            .expect("Offset must be valid"),
                    );
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|(reshandle, t)| {
                                let resource = store
                                    .resource(&AnyId::from(*reshandle))
                                    .expect("resource must exist");
                                format!(
                                    "{}",
                                    resource.utf8byte(t.begin()).expect("offset must be valid"),
                                )
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::EndUtf8Offset => {
                if let (Some(textselection), Some(resource)) =
                    (context.textselection, context.resource)
                {
                    print!(
                        "{}",
                        resource
                            .utf8byte(textselection.end())
                            .expect("Offset must be valid"),
                    );
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|(reshandle, t)| {
                                let resource = store
                                    .resource(&AnyId::from(*reshandle))
                                    .expect("resource must exist");
                                format!(
                                    "{}",
                                    resource.utf8byte(t.end()).expect("offset must be valid"),
                                )
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::Text => {
                if let Some(text) = context.text {
                    print!("{}", text)
                } else if context.textselections_text.is_some() {
                    print!(
                        "{}",
                        context
                            .textselections_text
                            .map(|text| text.join(delimiter).replace("\n", " "))
                            .unwrap_or(null.to_string()),
                    )
                } else {
                    print!("{}", null)
                }
            }
            Column::Annotation => print!(
                "{}",
                context
                    .annotation
                    .map(|annotation| annotation.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::AnnotationData => print!(
                "{}",
                context
                    .data
                    .map(|data| data.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::AnnotationDataSet => print!(
                "{}",
                context
                    .set
                    .map(|set| set.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::TextResource => print!(
                "{}",
                context
                    .resource
                    .map(|resource| resource.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::DataKey => print!(
                "{}",
                context
                    .key
                    .map(|key| key.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::DataValue => print!(
                "{}",
                context
                    .value
                    .map(|value| value.to_string())
                    .unwrap_or(null.to_string())
            ),
            _ => print!("{}", null),
        }
        if colnr == col_len - 1 {
            print!("\n");
        }
    }
}

struct Columns(Vec<Column>);

impl Columns {
    fn printrow(
        &self,
        store: &AnnotationStore,
        tp: Type,
        context: &Context,
        delimiter: &str,
        null: &str,
    ) {
        for (i, column) in self.0.iter().enumerate() {
            column.print(&store, tp, i, self.0.len(), context, delimiter, null);
        }
    }

    fn printheader(&self) {
        for (i, column) in self.0.iter().enumerate() {
            if i > 0 {
                print!("\t")
            } else {
                print!("#")
            }
            print!("{}", column);
            if i == self.0.len() - 1 {
                print!("\n")
            }
        }
    }
}

pub fn to_tsv(
    store: &AnnotationStore,
    columnconfig: &[&str],
    tp: Type,
    flatten: bool,
    delimiter: &str,
    null: &str,
    header: bool,
) {
    let columns = Columns(
        columnconfig
            .iter()
            .map(|col| {
                Column::try_from(*col)
                    .map_err(|err| {
                        eprintln!("{}", err);
                        exit(1);
                    })
                    .unwrap()
            })
            .collect(),
    );

    if header {
        columns.printheader();
    }

    match tp {
        Type::Annotation => {
            let want_text = columns.0.contains(&Column::Text);
            let want_textselections = columns.0.contains(&Column::TextSelection);
            for annotation in store.annotations() {
                let text: Option<Vec<&str>> = if want_text {
                    Some(store.text_by_annotation(annotation).collect())
                } else {
                    None
                };
                let textselections: Option<Vec<(TextResourceHandle, TextSelection)>> =
                    if want_textselections {
                        Some(store.textselections_by_annotation(annotation).collect())
                    } else {
                        None
                    };
                if !flatten {
                    let context = Context {
                        id: annotation.id(),
                        annotation: Some(annotation),
                        textselections: textselections.as_ref(),
                        textselections_text: text.as_ref(),
                        ..Context::default()
                    };
                    columns.printrow(&store, Type::Annotation, &context, delimiter, null);
                }
                for (key, data, dataset) in store.data_by_annotation(annotation) {
                    let context = Context {
                        id: if flatten { annotation.id() } else { data.id() },
                        annotation: Some(annotation),
                        textselections: if flatten {
                            textselections.as_ref()
                        } else {
                            None
                        },
                        textselections_text: if flatten { text.as_ref() } else { None },
                        key: Some(key),
                        data: Some(data),
                        set: Some(dataset),
                        value: Some(data.value()),
                        ..Context::default()
                    };
                    columns.printrow(
                        &store,
                        if flatten {
                            Type::Annotation
                        } else {
                            Type::AnnotationData
                        },
                        &context,
                        delimiter,
                        null,
                    );
                }
            }
        }
        Type::AnnotationDataSet | Type::AnnotationData | Type::DataKey => {
            for set in store.annotationsets() {
                if !flatten || tp == Type::AnnotationDataSet {
                    let context = Context {
                        id: set.id(),
                        set: Some(set),
                        ..Context::default()
                    };
                    columns.printrow(&store, Type::AnnotationDataSet, &context, delimiter, null);
                }
                if tp == Type::AnnotationData {
                    for data in set.data() {
                        let key = set
                            .key(&AnyId::from(data.key()))
                            .expect("Referenced key from data must exist");
                        let context = Context {
                            id: data.id(),
                            set: Some(set),
                            key: Some(key),
                            value: Some(data.value()),
                            ..Context::default()
                        };
                        columns.printrow(&store, Type::AnnotationData, &context, delimiter, null);
                    }
                } else if tp == Type::DataKey {
                    for key in set.keys() {
                        let context = Context {
                            id: key.id(),
                            set: Some(set),
                            key: Some(key),
                            ..Context::default()
                        };
                        columns.printrow(&store, Type::DataKey, &context, delimiter, null);
                    }
                }
            }
        }
        Type::TextResource | Type::TextSelection => {
            for res in store.resources() {
                if !flatten || tp == Type::TextResource {
                    let context = Context {
                        id: res.id(),
                        resource: Some(res),
                        text: if res.text().len() > 1024 || res.text().find("\n").is_some() {
                            Some("[too long to display]")
                        } else {
                            Some(res.text())
                        },
                        ..Context::default()
                    };
                    columns.printrow(&store, Type::TextResource, &context, delimiter, null);
                }
                if tp == Type::TextSelection {
                    for textselection in res.textselections() {
                        let textselections = vec![(res.handle().unwrap(), textselection.clone())];
                        let id = format!(
                            "{}#{}-{}",
                            res.id().unwrap_or(""),
                            textselection.begin(),
                            textselection.end()
                        );
                        let context = Context {
                            id: Some(id.as_str()),
                            resource: Some(res),
                            textselections: Some(&textselections),
                            text: res.text_by_textselection(textselection).ok(),
                            ..Context::default()
                        };
                        columns.printrow(&store, Type::TextSelection, &context, delimiter, null);
                    }
                }
            }
        }
    }
}
