use clap::Arg;
use stam::{
    Annotation, AnnotationBuilder, AnnotationData, AnnotationDataBuilder, AnnotationDataSet,
    AnnotationStore, Cursor, DataKey, DataOperator, DataValue, Item, Offset, Selector, StamError,
    Storable, StoreFor, Text, TextResource, TextResourceHandle, TextSelection, WrappedItem,
};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Deref;
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
                "Choose from the following known columns names (case insensitive, comma seperated list):

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

Instead of the above columns, you may also set a *custom* column by  specifying an AnnotationDataSet and DataKey within, seperated by a slash. The rows will then be filled with the
data values corresponding to the data key. Which works great when you set --type Annotation and want specific AnnotationData outputted. Example:

* my_set/part_of_speech
* my_set/lemma

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

#[derive(Clone, PartialEq, Debug)]
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
    Custom { set: String, key: String },
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
        if val.find("/").is_some() {
            let (set, key) = val.rsplit_once("/").unwrap();
            Ok(Self::Custom {
                set: set.to_string(),
                key: key.to_string(),
            })
        } else {
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
                "resource" | "resourceid" | "textresource" | "textresources" => {
                    Ok(Self::TextResource)
                }
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
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[derive(Clone)]
struct Context<'a> {
    id: Option<&'a str>,
    textselections: Option<&'a Vec<WrappedItem<'a, TextSelection>>>,
    text: Option<&'a str>,
    annotation: Option<WrappedItem<'a, Annotation>>,
    data: Option<WrappedItem<'a, AnnotationData>>,
    resource: Option<WrappedItem<'a, TextResource>>,
    set: Option<WrappedItem<'a, AnnotationDataSet>>,
    key: Option<WrappedItem<'a, DataKey>>,
    value: Option<&'a DataValue>,
}

impl<'a> Default for Context<'a> {
    fn default() -> Self {
        Context {
            id: None,
            textselections: None, //multiple
            text: None,           //single text reference
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
    fn to_string(&self) -> String {
        match self {
            Self::Type => "Type".to_string(),
            Self::Id => "Id".to_string(),
            Self::Annotation => "Annotation".to_string(),
            Self::TextResource => "TextResource".to_string(),
            Self::AnnotationData => "AnnotationData".to_string(),
            Self::AnnotationDataSet => "AnnotationDataSet".to_string(),
            Self::Offset => "Offset".to_string(),
            Self::BeginOffset => "BeginOffset".to_string(),
            Self::EndOffset => "EndOffset".to_string(),
            Self::Utf8Offset => "Utf8Offset".to_string(),
            Self::BeginUtf8Offset => "BeginUtf8Offset".to_string(),
            Self::EndUtf8Offset => "EndUtf8Offset".to_string(),
            Self::DataKey => "DataKey".to_string(),
            Self::DataValue => "DataValue".to_string(),
            Self::Text => "Text".to_string(),
            Self::TextSelection => "TextSelection".to_string(),
            Self::Ignore => "Ignore".to_string(),
            Self::Custom { set, key } => format!("{}/{}", set, key),
        }
    }

    fn print(
        &self,
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
                if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| {
                                format!(
                                    "{}#{}-{}",
                                    textselection.resource().id().unwrap_or(""),
                                    textselection.begin(),
                                    textselection.end()
                                )
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::Offset => {
                if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| {
                                format!("{}-{}", textselection.begin(), textselection.end())
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::BeginOffset => {
                if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| { format!("{}", textselection.begin()) })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::EndOffset => {
                if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| { format!("{}", textselection.end()) })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    );
                } else {
                    print!("{}", null)
                }
            }
            Column::Utf8Offset => {
                if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| {
                                format!(
                                    "{}-{}",
                                    textselection
                                        .resource()
                                        .utf8byte(textselection.begin())
                                        .expect("offset must be valid"),
                                    textselection
                                        .resource()
                                        .utf8byte(textselection.end())
                                        .expect("offset must be valid"),
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
                if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| {
                                format!(
                                    "{}",
                                    textselection
                                        .resource()
                                        .utf8byte(textselection.begin())
                                        .expect("offset must be valid"),
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
                if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| {
                                format!(
                                    "{}",
                                    textselection
                                        .resource()
                                        .utf8byte(textselection.end())
                                        .expect("offset must be valid"),
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
                } else if let Some(textselections) = context.textselections {
                    print!(
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| textselection.text().replace("\n", " "))
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    )
                } else {
                    print!("{}", null)
                }
            }
            Column::Annotation => print!(
                "{}",
                context
                    .annotation
                    .as_ref()
                    .map(|annotation| annotation.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::AnnotationData => print!(
                "{}",
                context
                    .data
                    .as_ref()
                    .map(|data| data.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::AnnotationDataSet => print!(
                "{}",
                context
                    .set
                    .as_ref()
                    .map(|set| set.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::TextResource => print!(
                "{}",
                context
                    .resource
                    .as_ref()
                    .map(|resource| resource.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::DataKey => print!(
                "{}",
                context
                    .key
                    .as_ref()
                    .map(|key| key.id().unwrap_or(null))
                    .unwrap_or(null)
            ),
            Column::DataValue => print!(
                "{}",
                context
                    .value
                    .as_ref()
                    .map(|value| value.to_string())
                    .unwrap_or(null.to_string())
            ),
            Column::Custom { set, key } => {
                let mut found = false;
                if let Some(annotation) = &context.annotation {
                    for (i, annotationdata) in annotation
                        .find_data(Some(set.into()), Some(key.into()), DataOperator::Any)
                        .into_iter()
                        .flatten()
                        .enumerate()
                    {
                        found = true;
                        print!(
                            "{}{}",
                            if i > 0 { delimiter } else { "" },
                            annotationdata.value()
                        )
                    }
                }
                if !found {
                    print!("{}", null)
                }
            }
            _ => print!("{}", null),
        }
        if colnr == col_len - 1 {
            print!("\n");
        }
    }
}

pub struct Columns(Vec<Column>);

impl Columns {
    fn printrow(&self, tp: Type, context: &Context, delimiter: &str, null: &str) {
        for (i, column) in self.0.iter().enumerate() {
            column.print(tp, i, self.len(), context, delimiter, null);
        }
    }

    fn printheader(&self) {
        for (i, column) in self.0.iter().enumerate() {
            if i > 0 {
                print!("\t")
            }
            print!("{}", column);
            if i == self.len() - 1 {
                print!("\n")
            }
        }
    }

    fn index(&self, coltype: &Column) -> Option<usize> {
        for (i, col) in self.0.iter().enumerate() {
            if col == coltype {
                return Some(i);
            }
        }
        None
    }

    fn has(&self, coltype: &Column) -> bool {
        self.index(coltype).is_some()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn iter(&self) -> std::slice::Iter<Column> {
        self.0.iter()
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
            let want_textselections =
                columns.0.contains(&Column::TextSelection) || columns.0.contains(&Column::Text);
            for annotation in store.annotations() {
                let textselections: Option<Vec<_>> = if want_textselections {
                    Some(annotation.textselections().collect())
                } else {
                    None
                };
                if !flatten {
                    let context = Context {
                        id: annotation.id(),
                        annotation: Some(annotation.clone()), //clones only the WrappedItem::Borrowed(), cheap
                        textselections: textselections.as_ref(),
                        ..Context::default()
                    };
                    columns.printrow(Type::Annotation, &context, delimiter, null);
                }
                for data in annotation.data() {
                    let context = Context {
                        id: if flatten { annotation.id() } else { data.id() },
                        annotation: Some(annotation.clone()),
                        textselections: if flatten {
                            textselections.as_ref()
                        } else {
                            None
                        },
                        key: Some(data.key()),
                        data: Some(data.clone()),
                        set: Some(data.set().wrap_in(store).unwrap()),
                        value: Some(data.value()),
                        ..Context::default()
                    };
                    columns.printrow(
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
                        set: Some(set.clone()),
                        ..Context::default()
                    };
                    columns.printrow(Type::AnnotationDataSet, &context, delimiter, null);
                }
                if tp == Type::AnnotationData {
                    for data in set.data() {
                        let context = Context {
                            id: data.id(),
                            set: Some(set.clone()),
                            key: Some(data.key()),
                            value: Some(data.value()),
                            ..Context::default()
                        };
                        columns.printrow(Type::AnnotationData, &context, delimiter, null);
                    }
                } else if tp == Type::DataKey {
                    for key in set.keys() {
                        let context = Context {
                            id: key.id(),
                            set: Some(set.clone()),
                            key: Some(key.clone()),
                            ..Context::default()
                        };
                        columns.printrow(Type::DataKey, &context, delimiter, null);
                    }
                }
            }
        }
        Type::TextResource | Type::TextSelection => {
            for res in store.resources() {
                if !flatten || tp == Type::TextResource {
                    let context = Context {
                        id: res.id(),
                        resource: Some(res.clone()),
                        text: if res.text().len() > 1024 || res.text().find("\n").is_some() {
                            Some("[too long to display]")
                        } else {
                            Some(res.text())
                        },
                        ..Context::default()
                    };
                    columns.printrow(Type::TextResource, &context, delimiter, null);
                }
                if tp == Type::TextSelection {
                    for textselection in res.textselections() {
                        let id = format!(
                            "{}#{}-{}",
                            res.id().unwrap_or(""),
                            textselection.begin(),
                            textselection.end()
                        );
                        let text = Some(textselection.text());
                        let textselections = vec![textselection];
                        let context = Context {
                            id: Some(id.as_str()),
                            resource: Some(res.clone()),
                            textselections: Some(&textselections),
                            text,
                            ..Context::default()
                        };
                        columns.printrow(Type::TextSelection, &context, delimiter, null);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParseMode {
    Simple,
    /// Align with an existing text resource
    AlignWithText,
    /// Reconstruct a text resource from scratch
    ReconstructText,
    ///  Tag all occurrences
    MultiTag,
    ///
    Metadata,
}

impl ParseMode {
    pub fn new(
        columns: &Columns,
        existing_resource: Option<&str>,
        new_resource: Option<&str>,
        sequential: bool,
    ) -> Result<Self, &'static str> {
        if columns.has(&Column::Text) {
            if columns.has(&Column::Offset)
                || (columns.has(&Column::BeginOffset) && columns.has(&Column::EndOffset))
                || columns.has(&Column::TextSelection)
            {
                Ok(Self::Simple)
            } else {
                //no offset information
                if columns.has(&Column::TextResource)
                    || existing_resource.is_some()
                    || columns.has(&Column::TextSelection)
                {
                    if sequential {
                        Ok(Self::AlignWithText)
                    } else {
                        Ok(Self::MultiTag)
                    }
                } else {
                    if sequential {
                        Ok(Self::ReconstructText)
                    } else {
                        Err("Can not reconstruct a text if rows in input data are not sequential")
                    }
                }
            }
        } else if !columns.has(&Column::Offset)
            && !columns.has(&Column::BeginOffset)
            && !columns.has(&Column::EndOffset)
            && !columns.has(&Column::TextSelection)
        {
            if columns.has(&Column::TextResource) || existing_resource.is_some() {
                eprintln!("Warning: Data has neither a Text nor an Offset column, interpreting data as metadata");
                Ok(Self::Metadata)
            } else {
                Err("Data has neither a Text nor an Offset column")
            }
        } else {
            Err("Unable to determine how to parse this data based on the available columns. Make sure there is at least an Offset or Text column")
        }
    }
}

pub fn from_tsv(
    store: &mut AnnotationStore,
    filename: &str,
    columnconfig: Option<&[&str]>,
    existing_resource: Option<&str>,
    new_resource: Option<&str>,
    default_set: &str,
    sequential: bool,
    delimiter: &str,      //input delimiter for multiple values in a cell
    header: Option<bool>, //None means autodetect
) {
    let f = File::open(filename).unwrap_or_else(|e| {
        eprintln!("Error opening rules {}: {}", filename, e);
        exit(1);
    });
    let reader = BufReader::new(f);

    let mut columns: Option<Columns> = None;
    let mut parsemode: Option<ParseMode> = None;

    for (i, line) in reader.lines().enumerate() {
        if let Ok(line) = line {
            if line.is_empty() {
                continue;
            } else if i == 0 && columns.is_none() && header != Some(false) {
                columns = Some(
                    Columns(
                        line.split("\t")
                            .map(|col| {
                                Column::try_from(col)
                                    .map_err(|err| {
                                        eprintln!("Unable to parse first line of TSV file as header (please provide a column configuration explicitly if the input file has none): {}", err);
                                        exit(1);
                                    })
                                    .unwrap()
                            })
                            .collect(),
                    )
                );
                parsemode = Some(
                    ParseMode::new(
                        columns.as_ref().unwrap(),
                        existing_resource,
                        new_resource,
                        sequential,
                    )
                    .unwrap_or_else(|e| {
                        eprintln!("Can't determine parse mode: {}", e);
                        exit(1);
                    }),
                );
            } else {
                if columns.is_none() {
                    if columnconfig.is_none() {
                        eprintln!("Please provide a configuration for the columns");
                        exit(1);
                    }
                    columns = Some(Columns(
                        columnconfig
                            .unwrap()
                            .iter()
                            .map(|col| {
                                Column::try_from(*col)
                                    .map_err(|err| {
                                        eprintln!("Unable to parse provided columns: {}", err);
                                        exit(1);
                                    })
                                    .unwrap()
                            })
                            .collect(),
                    ));
                    parsemode = Some(
                        ParseMode::new(
                            columns.as_ref().unwrap(),
                            existing_resource,
                            new_resource,
                            sequential,
                        )
                        .unwrap_or_else(|e| {
                            eprintln!("Can't determine parse mode: {}", e);
                            exit(1);
                        }),
                    );
                }
                if let (Some(columns), Some(parsemode)) = (&columns, parsemode) {
                    if let Err(e) = parse_row(
                        store,
                        &line,
                        &columns,
                        parsemode,
                        existing_resource,
                        new_resource,
                        default_set,
                    ) {
                        eprintln!("Error parsing tsv line {}: {}", i + 1, e);
                        exit(1);
                    }
                }
            }
        }
    }
}

pub fn parse_row(
    store: &mut AnnotationStore,
    line: &str,
    columns: &Columns,
    parsemode: ParseMode,
    existing_resource: Option<&str>,
    new_resource: Option<&str>,
    default_set: &str,
) -> Result<(), String> {
    let cells: Vec<&str> = line.split("\t").collect();
    if cells.len() != columns.len() {
        return Err(format!(
            "Number of cells is not equal to number of columns in header ({} vs {})",
            cells.len(),
            columns.len()
        ));
    }
    let resource_file: &str =
        parse_resource_file(store, &cells, columns, existing_resource, new_resource)?;
    let resource_handle: TextResourceHandle = get_resource_handle(store, resource_file)?;
    match parsemode {
        ParseMode::Simple => {
            let selector = build_selector(store, &cells, columns, resource_handle)?;
            let annotationbuilder = build_annotation(&cells, columns, default_set)?;
            if let Err(e) = store.annotate(annotationbuilder) {
                return Err(format!("{}", e));
            }
        }
    }
    Ok(())
}

pub fn build_annotation<'a>(
    cells: &'a [&'a str],
    columns: &Columns,
    default_set: &'a str,
) -> Result<AnnotationBuilder<'a>, String> {
    let mut annotationbuilder = AnnotationBuilder::new();
    if let Some(i) = columns.index(&Column::Id) {
        let id = cells.get(i).expect("cell must exist");
        annotationbuilder = annotationbuilder.with_id(id.to_string());
    } else if let Some(i) = columns.index(&Column::Annotation) {
        //same as above
        let id = cells.get(i).expect("cell must exist");
        annotationbuilder = annotationbuilder.with_id(id.to_string());
    } else if let (Some(ikey), Some(ivalue)) = (
        columns.index(&Column::DataKey),
        columns.index(&Column::DataValue),
    ) {
        let mut databuilder = AnnotationDataBuilder::new();
        if let Some(i) = columns.index(&Column::AnnotationData) {
            let id = cells.get(i).expect("cell must exist");
            databuilder = databuilder.with_id(Item::IdRef(id));
        } else {
            databuilder = databuilder.with_id(Item::IdRef(default_set));
        }
        if let Some(i) = columns.index(&Column::AnnotationDataSet) {
            let set = cells.get(i).expect("cell must exist");
            databuilder = databuilder.with_annotationset(Item::Id(set.to_string()));
        }
        let key = cells.get(ikey).expect("cell must exist");
        let value = cells.get(ivalue).expect("cell must exist");
        databuilder = databuilder.with_key(Item::from(key.deref()));
        databuilder = databuilder.with_value(DataValue::from(value.deref()));
        annotationbuilder = annotationbuilder.with_data_builder(databuilder);
    }
    //process custom columns
    for (column, cell) in columns.iter().zip(cells.iter()) {
        if let Column::Custom { set, key } = column {
            let value: DataValue = cell.deref().into();
            let databuilder = AnnotationDataBuilder::new()
                .with_annotationset(Item::Id(set.clone()))
                .with_key(Item::Id(key.clone()))
                .with_value(value);
            annotationbuilder = annotationbuilder.with_data_builder(databuilder);
        }
    }
    Ok(annotationbuilder)
}

pub fn parse_resource_file<'a>(
    store: &AnnotationStore,
    cells: &[&'a str],
    columns: &Columns,
    existing_resource: Option<&'a str>,
    new_resource: Option<&'a str>,
) -> Result<&'a str, String> {
    if let Some(i) = columns.index(&Column::TextResource) {
        Ok(cells.get(i).expect("cell must exist"))
    } else if let Some(i) = columns.index(&Column::TextSelection) {
        let textselection = cells.get(i).expect("cell must exist");
        if let Some(bytepos) = textselection.find('#') {
            Ok(&textselection[..bytepos])
        } else {
            Err("Text selection must have format: resource#beginoffset-endoffset".to_string())
        }
    } else if let Some(existing_resource) = existing_resource {
        Ok(existing_resource)
    } else if let Some(new_resource) = new_resource {
        Ok(new_resource)
    } else {
        Err("Can't find resource".to_string())
    }
}

pub fn get_resource_handle(
    store: &mut AnnotationStore,
    filename: &str,
) -> Result<TextResourceHandle, String> {
    if let Some(resource) = store.resource(&Item::from(filename)) {
        if let Some(handle) = resource.handle() {
            return Ok(handle);
        }
    }
    store
        .add_resource_from_file(filename)
        .map_err(|e| format!("Specified resource not found: {}: {}", filename, e))
}

pub fn build_selector(
    store: &AnnotationStore,
    cells: &[&str],
    columns: &Columns,
    resource_handle: TextResourceHandle,
) -> Result<Selector, String> {
    //TODO: for now this only returns a TextSelector, should be adapted to handle multiple offsets (with subdelimiter) and return a CompositeSelector then
    let offset = parse_offset(store, cells, columns)?;
    Ok(Selector::TextSelector(resource_handle, offset))
}

pub fn parse_offset(
    store: &AnnotationStore,
    cells: &[&str],
    columns: &Columns,
) -> Result<Offset, String> {
    if let (Some(b), Some(e)) = (
        columns.index(&Column::BeginOffset),
        columns.index(&Column::EndOffset),
    ) {
        let begin_str = cells.get(b).expect("cell must exist");
        let end_str = cells.get(e).expect("cell must exist");
        let begin: Cursor = begin_str.deref().try_into().map_err(|e| format!("{}", e))?;
        let end: Cursor = end_str.deref().try_into().map_err(|e| format!("{}", e))?;
        Ok(Offset::new(begin, end))
    } else if let Some(i) = columns.index(&Column::TextSelection) {
        let textselection = cells.get(i).expect("cell must exist");
        if let Some(bytepos) = textselection.find('#') {
            if let Some(delimiterpos) = &textselection[(bytepos + 2)..].find('-') {
                let delimiterpos = *delimiterpos + bytepos + 2; //we do 2 rather than 1 to not consider an immediate hyphen after the # , that would indicate a negative begin index
                let begin_str = &textselection[(bytepos + 1)..delimiterpos];
                let end_str = &textselection[(delimiterpos + 1)..];
                let begin: Cursor = begin_str.deref().try_into().map_err(|e| format!("{}", e))?;
                let end: Cursor = end_str.deref().try_into().map_err(|e| format!("{}", e))?;
                return Ok(Offset::new(begin, end));
            }
        }
        Err("Text selection must have format: resource#beginoffset-endoffset".to_string())
    } else {
        Err(format!("No offset information found"))
    }
}
