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
* AnnotationDataSet    - Outputs the ID of the associated AnnotationDataSet
* TextSelections       - Outputs any associated text selection as a combination of resource identifier(s) with an offset
* Text                 - Outputs the associated text
* Ignore               - Always outputs the NULL value
",
            )
            .takes_value(true)
            .default_value("Type,Id,AnnotationDataSet,DataKey,DataValue,Text,TextSelections"),
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
    Resource,
    Resources,
    AnnotationData,
    AnnotationDataSet,
    Offset,
    BeginOffset,
    EndOffset,
    DataKey,
    DataValue,
    Text,
    TextSelections,
    Ignore,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Type {
    Annotation,
    AnnotationDataSet,
    AnnotationData,
    DataKey,
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
            "resources" => Ok(Self::Resources),
            "resource" | "resourceid" => Ok(Self::Resource),
            "annotationdataid" | "dataid" => Ok(Self::AnnotationData),
            "offset" => Ok(Self::Offset),
            "beginoffset" | "begin" | "start" | "startoffset" => Ok(Self::BeginOffset),
            "endoffset" | "end" => Ok(Self::EndOffset),
            "datakey" | "key" | "datakeyid" | "keyid" => Ok(Self::DataKey),
            "datavalue" | "value" => Ok(Self::DataValue),
            "text" => Ok(Self::Text),
            "textselections" | "textselection" => Ok(Self::TextSelections),
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
    text: Option<&'a Vec<&'a str>>,
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
            textselections: None,
            text: None,
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
            Self::Resource => "ResourceId",
            Self::Resources => "Resources",
            Self::AnnotationData => "AnnotationData",
            Self::AnnotationDataSet => "AnnotationDataSet",
            Self::Offset => "Offset",
            Self::BeginOffset => "BeginOffset",
            Self::EndOffset => "EndOffset",
            Self::DataKey => "DataKey",
            Self::DataValue => "DataValue",
            Self::Text => "Text",
            Self::TextSelections => "TextSelections",
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
            Column::TextSelections => {
                print!(
                    "{}",
                    context
                        .textselections
                        .map(|textselections| {
                            textselections
                                .iter()
                                .map(|(reshandle, t)| {
                                    let resource = store
                                        .resource(&AnyId::from(*reshandle))
                                        .expect("resource must exist");
                                    format!(
                                        "{}#{}-{}",
                                        resource.id().unwrap_or(""),
                                        t.begin(),
                                        t.end()
                                    )
                                })
                                .collect::<Vec<String>>()
                                .join(delimiter)
                        })
                        .unwrap_or(null.to_string())
                );
            }
            Column::Text => print!(
                "{}",
                context
                    .text
                    .map(|text| text.join(delimiter).replace("\n", " "))
                    .unwrap_or(null.to_string()),
            ),
            Column::Resources => print!(
                "{}",
                context
                    .textselections
                    .map(|textselections| {
                        textselections
                            .iter()
                            .map(|(reshandle, _t)| {
                                let resource = store
                                    .resource(&AnyId::from(*reshandle))
                                    .expect("resource must exist");
                                format!("{}", resource.id().unwrap_or(""))
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    })
                    .unwrap_or(null.to_string())
            ),
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
            Column::Resource => print!(
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

struct ColumnConfig(Vec<Column>);

impl ColumnConfig {
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
    let columns = ColumnConfig(
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
            let want_textselections = columns.0.contains(&Column::TextSelections);
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
                        text: text.as_ref(),
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
                        text: if flatten { text.as_ref() } else { None },
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
    }
}
