use stam::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Clone, PartialEq, Debug)]
/// Represents a column in TSV output or input
pub enum Column {
    /// Sequence number, usually a row number but sometimes multiple rows may share the same number if hierarchical relations are expressed
    SeqNr,

    /// Variable name, as used in a STAMQL query
    VarName,

    /// Type of the result on this row
    Type,

    /// ID of this result on this row
    Id,

    /// ID of the annotation
    Annotation,

    /// ID of the text resource
    TextResource,

    /// ID of the annotation data
    AnnotationData,

    /// ID of the annotation dataset
    AnnotationDataSet,

    /// Offset in unicode points (begin-end), 0 indexed, end non-inclusive.
    Offset,

    ///Begin offset in unicode points, 0 indexed.
    BeginOffset,

    ///End offset in unicode points, 0 indexed, non-inclusive.
    EndOffset,

    Utf8Offset,

    ///Begin offset in bytes (UTF-8 encoding), 0 indexed
    BeginUtf8Offset,

    ///End offset in bytes (UTF-8 encoding), 0 indexed, non-inclusive
    EndUtf8Offset,

    /// ID of the data key
    DataKey,

    /// Value
    DataValue,

    /// The text
    Text,

    /// The text selection is a combination of `TextResource` and `Offset`, seperated by a '#`
    TextSelection,

    /// Ignore this column
    Ignore,

    /// Custom data column, represents the value for the given set and datakey.
    Custom {
        set: String,
        key: String,
    },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ValidationMode {
    Strict,
    Loose,
    No,
}

impl TryFrom<&str> for ValidationMode {
    type Error = String;
    fn try_from(val: &str) -> Result<Self, Self::Error> {
        let val_lower = val.to_lowercase();
        match val_lower.as_str() {
            "strict" | "yes" => Ok(Self::Strict),
            "loose" => Ok(Self::Loose),
            "no" => Ok(Self::No),
            _ => Err(format!(
                "Unknown value for --validate: {}, see --help for allowed values",
                val
            )),
        }
    }
}

impl Column {
    /// Parse a column header into a type
    pub fn parse(val: &str, setdelimiter: &str) -> Result<Self, String> {
        if val.find(setdelimiter).is_some() {
            let (set, key) = val.rsplit_once(setdelimiter).unwrap();
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
    id: Option<Cow<'a, str>>,
    varname: Option<Cow<'a, str>>,
    seqnr: usize,
    textselections: Option<&'a Vec<ResultTextSelection<'a>>>,
    text: Option<&'a str>,
    annotation: Option<ResultItem<'a, Annotation>>,
    data: Option<ResultItem<'a, AnnotationData>>,
    resource: Option<ResultItem<'a, TextResource>>,
    set: Option<ResultItem<'a, AnnotationDataSet>>,
    key: Option<ResultItem<'a, DataKey>>,
    value: Option<&'a DataValue>,
}

impl<'a> Default for Context<'a> {
    fn default() -> Self {
        Context {
            id: None,
            varname: None,
            seqnr: 0,
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
    /// Output a string for this column, to be used in e.g. a TSV header
    pub fn to_string(&self) -> String {
        match self {
            Self::SeqNr => "SeqNr".to_string(),
            Self::VarName => "Variable".to_string(),
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

    fn print<W: std::io::Write>(
        &self,
        writer: &mut W,
        tp: Type,
        colnr: usize,
        col_len: usize,
        context: &Context,
        delimiter: &str,
        null: &str,
    ) -> Result<(), std::io::Error> {
        if colnr > 0 {
            write!(writer, "\t")?;
        }
        match self {
            Column::SeqNr => write!(writer, "{}", context.seqnr)?,
            Column::VarName => write!(
                writer,
                "{}",
                context.varname.as_ref().unwrap_or(&Cow::Borrowed(null))
            )?,
            Column::Type => write!(writer, "{}", tp)?,
            Column::Id => write!(
                writer,
                "{}",
                context.id.as_ref().unwrap_or(&Cow::Borrowed(null))
            )?,
            Column::TextSelection => {
                if let Some(textselections) = context.textselections {
                    write!(
                        writer,
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
                    )?;
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::Offset => {
                if let Some(textselections) = context.textselections {
                    write!(
                        writer,
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| {
                                format!("{}-{}", textselection.begin(), textselection.end())
                            })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    )?;
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::BeginOffset => {
                if let Some(textselections) = context.textselections {
                    write!(
                        writer,
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| { format!("{}", textselection.begin()) })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    )?;
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::EndOffset => {
                if let Some(textselections) = context.textselections {
                    write!(
                        writer,
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| { format!("{}", textselection.end()) })
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    )?;
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::Utf8Offset => {
                if let Some(textselections) = context.textselections {
                    write!(
                        writer,
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
                    )?;
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::BeginUtf8Offset => {
                if let Some(textselections) = context.textselections {
                    write!(
                        writer,
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
                    )?;
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::EndUtf8Offset => {
                if let Some(textselections) = context.textselections {
                    write!(
                        writer,
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
                    )?;
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::Text => {
                if let Some(text) = context.text {
                    write!(writer, "{}", text)?
                } else if let Some(textselections) = context.textselections {
                    write!(
                        writer,
                        "{}",
                        textselections
                            .iter()
                            .map(|textselection| textselection.text().replace("\n", " "))
                            .collect::<Vec<String>>()
                            .join(delimiter)
                    )?
                } else {
                    write!(writer, "{}", null)?
                }
            }
            Column::Annotation => write!(
                writer,
                "{}",
                context
                    .annotation
                    .as_ref()
                    .map(|annotation| annotation
                        .id()
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| annotation.as_ref().temp_id().unwrap()))
                    .unwrap_or(null.to_string())
            )?,
            Column::AnnotationData => write!(
                writer,
                "{}",
                context
                    .data
                    .as_ref()
                    .map(|data| data.id().unwrap_or(null))
                    .unwrap_or(null)
            )?,
            Column::AnnotationDataSet => write!(
                writer,
                "{}",
                context
                    .set
                    .as_ref()
                    .map(|set| set.id().unwrap_or(null))
                    .unwrap_or(null)
            )?,
            Column::TextResource => write!(
                writer,
                "{}",
                context
                    .resource
                    .as_ref()
                    .map(|resource| resource.id().unwrap_or(null))
                    .unwrap_or(null)
            )?,
            Column::DataKey => write!(
                writer,
                "{}",
                context
                    .key
                    .as_ref()
                    .map(|key| key.id().unwrap_or(null))
                    .unwrap_or(null)
            )?,
            Column::DataValue => write!(
                writer,
                "{}",
                context
                    .value
                    .as_ref()
                    .map(|value| value.to_string())
                    .unwrap_or(null.to_string())
            )?,
            Column::Custom { set, key } => {
                let mut found = false;
                if let Some(annotation) = &context.annotation {
                    if let Some(key) = annotation.store().key(set.as_str(), key.as_str()) {
                        for (i, annotationdata) in annotation.data().filter_key(&key).enumerate() {
                            found = true;
                            write!(
                                writer,
                                "{}{}",
                                if i > 0 { delimiter } else { "" },
                                annotationdata.value()
                            )?
                        }
                    }
                }
                if !found {
                    write!(writer, "{}", null)?
                }
            }
            _ => write!(writer, "{}", null)?,
        }
        if colnr == col_len - 1 {
            write!(writer, "\n")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
/// A column specification, holds one or more [`Column`] instances.
pub struct Columns(Vec<Column>);

impl Columns {
    fn printrow<W: std::io::Write>(
        &self,
        writer: &mut W,
        tp: Type,
        context: &Context,
        delimiter: &str,
        null: &str,
    ) -> Result<(), std::io::Error> {
        for (i, column) in self.0.iter().enumerate() {
            column.print(writer, tp, i, self.len(), context, delimiter, null)?;
        }
        Ok(())
    }

    fn printheader<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        for (i, column) in self.0.iter().enumerate() {
            if i > 0 {
                write!(writer, "\t")?;
            }
            write!(writer, "{}", column)?;
            if i == self.len() - 1 {
                write!(writer, "\n")?;
            }
        }
        Ok(())
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

    fn add_from_query<'a>(&mut self, query: &Query<'a>) {
        for constraint in query.iter() {
            match constraint {
                Constraint::KeyValue { set, key, .. } | Constraint::DataKey { set, key, .. } => {
                    self.0.push(Column::Custom {
                        set: set.to_string(),
                        key: key.to_string(),
                    })
                }
                _ => {}
            }
        }
        if let Some(subquery) = query.subquery() {
            self.add_from_query(subquery);
        }
    }
}

pub fn to_tsv<'a, W: std::io::Write>(
    store: &'a AnnotationStore,
    writer: &mut W,
    query: Query<'a>,
    columnconfig: &[&str],
    verbose: bool,
    delimiter: &str,
    null: &str,
    header: bool,
    setdelimiter: &str,
    autocolumns: bool,
) -> Result<(), StamError> {
    let mut columns = Columns(
        columnconfig
            .iter()
            .map(|col| {
                Column::parse(*col, setdelimiter)
                    .map_err(|err| {
                        eprintln!("[warning] {}", err);
                    })
                    .unwrap()
            })
            .collect(),
    );

    if autocolumns {
        if (verbose || query.subquery().is_some()) && !columns.0.contains(&Column::SeqNr) {
            //output the sequence (row) number in verbose mode or if we have subqueries
            columns.0.insert(0, Column::SeqNr);
        }
        if query.subquery().is_some() && !columns.0.contains(&Column::VarName) {
            //output the variable name if we have subqueries
            columns.0.insert(1, Column::VarName);
        }

        columns.add_from_query(&query);
    }

    if header {
        columns.printheader(writer)?;
    }

    let want_textselections =
        columns.0.contains(&Column::TextSelection) || columns.0.contains(&Column::Text);

    let iter = store.query(query)?;
    let names = iter.names();
    let names_ordered = names.enumerate();
    for (seqnr, resultrow) in iter.enumerate() {
        let seqnr = seqnr + 1; //1-indexed
        for (i, result) in resultrow.iter().enumerate() {
            let varname = names_ordered.get(i).map(|x| Cow::Borrowed(x.1));
            match result {
                QueryResultItem::None => {}
                QueryResultItem::Annotation(annotation) => {
                    let textselections: Option<Vec<_>> = if want_textselections {
                        Some(annotation.textselections().collect())
                    } else {
                        None
                    };
                    let context = Context {
                        id: if let Some(id) = annotation.id() {
                            Some(Cow::Borrowed(id))
                        } else {
                            Some(Cow::Owned(annotation.as_ref().temp_id().unwrap()))
                        },
                        seqnr,
                        varname,
                        annotation: Some(annotation.clone()), //clones only the ResultItem, cheap
                        textselections: textselections.as_ref(),
                        ..Context::default()
                    };
                    columns.printrow(writer, Type::Annotation, &context, delimiter, null)?;
                    if verbose {
                        for data in annotation.data() {
                            let context = Context {
                                id: data.id().map(|x| Cow::Borrowed(x)),
                                seqnr,
                                annotation: Some(annotation.clone()),
                                key: Some(data.key()),
                                data: Some(data.clone()),
                                set: Some(data.set()),
                                value: Some(data.value()),
                                ..Context::default()
                            };
                            columns.printrow(
                                writer,
                                Type::AnnotationData,
                                &context,
                                delimiter,
                                null,
                            )?;
                        }
                    }
                }
                QueryResultItem::AnnotationData(data) => {
                    let context = Context {
                        id: data.id().map(|x| Cow::Borrowed(x)),
                        seqnr,
                        varname,
                        set: Some(data.set()),
                        key: Some(data.key()),
                        value: Some(data.value()),
                        ..Context::default()
                    };
                    columns.printrow(writer, Type::AnnotationData, &context, delimiter, null)?;
                }
                QueryResultItem::DataKey(key) => {
                    let context = Context {
                        id: key.id().map(|x| Cow::Borrowed(x)),
                        seqnr,
                        varname,
                        set: Some(key.set()),
                        key: Some(key.clone()),
                        ..Context::default()
                    };
                    columns.printrow(writer, Type::DataKey, &context, delimiter, null)?;
                }
                QueryResultItem::AnnotationDataSet(dataset) => {
                    let context = Context {
                        id: dataset.id().map(|x| Cow::Borrowed(x)),
                        seqnr,
                        varname: varname.clone(),
                        set: Some(dataset.clone()),
                        ..Context::default()
                    };
                    columns.printrow(writer, Type::AnnotationDataSet, &context, delimiter, null)?;
                    if verbose {
                        for key in dataset.keys() {
                            let context = Context {
                                id: key.id().map(|x| Cow::Borrowed(x)),
                                seqnr,
                                set: Some(key.set()),
                                key: Some(key.clone()),
                                ..Context::default()
                            };
                            columns.printrow(writer, Type::DataKey, &context, delimiter, null)?;
                        }
                        for data in dataset.data() {
                            let context = Context {
                                id: data.id().map(|x| Cow::Borrowed(x)),
                                seqnr,
                                set: Some(data.set()),
                                key: Some(data.key()),
                                value: Some(data.value()),
                                ..Context::default()
                            };
                            columns.printrow(
                                writer,
                                Type::AnnotationData,
                                &context,
                                delimiter,
                                null,
                            )?;
                        }
                    }
                }
                QueryResultItem::TextResource(resource) => {
                    let context = Context {
                        id: resource.id().map(|x| Cow::Borrowed(x)),
                        varname: varname.clone(),
                        seqnr,
                        resource: Some(resource.clone()),
                        ..Context::default()
                    };
                    columns.printrow(writer, Type::TextResource, &context, delimiter, null)?;
                }
                QueryResultItem::TextSelection(textselection) => {
                    let id = format!(
                        "{}#{}-{}",
                        textselection.resource().id().unwrap_or(""),
                        textselection.begin(),
                        textselection.end()
                    );
                    let text = Some(textselection.text());
                    let textselections: Vec<ResultTextSelection> = vec![textselection.clone()];
                    let context = Context {
                        id: Some(Cow::Owned(id)),
                        seqnr,
                        varname,
                        resource: Some(textselection.resource()),
                        textselections: Some(&textselections),
                        text,
                        ..Context::default()
                    };
                    columns.printrow(writer, Type::TextSelection, &context, delimiter, null)?;
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Determines how a TSV file is parsed in relation to references text files (which may or may not exist).
pub enum ParseMode {
    /// Normal parse mode, assumes a stand-off text file exists and alignment information is given.
    Simple,
    /// Align with an existing text resource. This is useful when a TSV file holds text and a stand-off file exist, but no alignment is provided. The alignment will be computed.
    AlignWithText,
    /// Reconstruct a text resource from scratch. This is useful when a TSV file holds the text and an stand-off file does not exist. It will be created.
    ReconstructText,
    /// Tag all occurrences. This is used when there is stand-off file and the TSV files applies occurrences in that stand-off text file, rather than to any single ones.
    MultiTag,
    /// This is used when a TSV file does not relate to a text at all.
    Metadata,
}

impl ParseMode {
    /// Automatically determines a ParseMode from a column configuration and presence or absence of an existing resource
    /// `sequential`: Is the information in the TSV file sequential/ordered? (e.g. a token or line on each subsequent row)
    pub fn new(
        columns: &Columns,
        existing_resource: Option<&str>,
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
        } else if columns.has(&Column::TextResource) || existing_resource.is_some() {
            if columns.has(&Column::Offset)
                || (columns.has(&Column::BeginOffset) && columns.has(&Column::EndOffset))
                || columns.has(&Column::TextSelection)
            {
                Ok(Self::Simple)
            } else {
                Err("Unable to determine how to parse this data based on the available columns. Make sure there is at least an Offset column (or BeginOffset, EndOffset columns)")
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
            Err("Unable to determine how to parse this data based on the available columns. Make sure there is at least an Offset, Text or Resource column (or supply --resource)")
        }
    }
}

/// Reads a TSV, with a flexible column configuration, into an Annotation Store
pub fn from_tsv(
    store: &mut AnnotationStore,
    filename: &str,
    columnconfig: Option<&Vec<&str>>,
    existing_resource: Option<&str>,
    new_resource: Option<&str>,
    default_set: Option<&str>,
    comments: bool,
    sequential: bool,
    case_sensitive: bool,
    escape: bool,
    nullvalue: &str,
    subdelimiter: &str,     //input delimiter for multiple values in a cell
    setdelimiter: &str,     //delimiter between key/set
    outputdelimiter: &str,  //outputted after each row when reconstructing text (space)
    outputdelimiter2: &str, //outputted after each empty line when reconstructing text (newline)
    header: Option<bool>,   //None means autodetect
    validation: ValidationMode,
    verbose: bool,
) -> Result<(), String> {
    let f =
        File::open(filename).map_err(|e| format!("Error opening TSV file {}: {}", filename, e))?;
    let reader = BufReader::new(f);

    let mut columns: Option<Columns> = None;
    let mut parsemode: Option<ParseMode> = None;
    let mut cursors: HashMap<TextResourceHandle, usize> = HashMap::new(); //used in AlignWithText mode to keep track of the begin of text offset (per resource)
    let mut buffer: Vec<String> = Vec::new(); //used in ReconstructText mode for a second pass over the data
    let mut bufferbegin: usize = 0; //line number where the buffer begins
    let mut texts: HashMap<String, String> = HashMap::new(); //used in ReconstructText mode
    let mut buffered_delimiter: Option<String> = None; // used in ReconstructText mode

    for (i, line) in reader.lines().enumerate() {
        if let Ok(line) = line {
            if line.is_empty() {
                buffered_delimiter = Some(outputdelimiter2.to_string()); //only affects ReconstructText mode
            } else if comments && !line.is_empty() && &line.get(0..1) == &Some("#") {
                //this is a comment, ignore
                continue;
            } else if i == 0 && columns.is_none() && header != Some(false) {
                if verbose {
                    eprintln!("Parsing first row as header...")
                }
                columns = Some(
                    Columns(
                        line.split("\t")
                            .map(|col| {
                                parse_column(col, default_set, setdelimiter).map_err(|err| {
                                    eprintln!("[warning] Unable to parse first line of TSV file as header (please provide a column configuration explicitly if the input file has none): {}. You may consider setting --annotationset if you want to interpret this column as a key in the specified annotationset", err);
                                }).unwrap()
                            })
                            .collect(),
                    )
                );
                parsemode = Some(
                    ParseMode::new(columns.as_ref().unwrap(), existing_resource, sequential)
                        .map_err(|e| format!("Can't determine parse mode: {}", e))?,
                );
                if verbose {
                    eprintln!("Columns: {:?}", columns.as_ref().unwrap());
                    eprintln!("Parse mode: {:?}", parsemode.unwrap());
                }
            } else if i == 0 && columns.is_some() && header != Some(false) {
                if verbose {
                    eprintln!("Skipping first row (assuming to be a header)...")
                }
                continue; //skip header row
            } else {
                if columns.is_none() {
                    if columnconfig.is_none() {
                        return Err(format!("Please provide a configuration for the columns"));
                    }
                    columns = Some(Columns(
                        columnconfig
                            .unwrap()
                            .iter()
                            .map(|col| {
                                parse_column(col, default_set, setdelimiter)
                                    .map_err(|err| {
                                        eprintln!(
                                            "[warning] Unable to parse provided column: {}",
                                            err
                                        );
                                    })
                                    .unwrap()
                            })
                            .collect(),
                    ));
                    parsemode = Some(
                        ParseMode::new(columns.as_ref().unwrap(), existing_resource, sequential)
                            .map_err(|e| format!("Can't determine parse mode: {}", e))?,
                    );
                    if verbose {
                        eprintln!("Columns: {:?}", columns.as_ref().unwrap());
                        eprintln!("Parse mode: {:?}", parsemode.unwrap())
                    }
                }
                if let (Some(columns), Some(parsemode)) = (&columns, parsemode) {
                    if parsemode == ParseMode::ReconstructText {
                        if let Err(e) = reconstruct_text(
                            &line,
                            &columns,
                            &mut texts,
                            existing_resource,
                            new_resource,
                            outputdelimiter,
                            &mut buffered_delimiter,
                        ) {
                            return Err(format!(
                                "Error reconstructing text (line {}): {}",
                                i + 1,
                                e
                            ));
                        }
                        if buffer.is_empty() {
                            bufferbegin = i;
                        }
                        buffer.push(line);
                    } else if let Err(e) = parse_row(
                        store,
                        &line,
                        &columns,
                        parsemode,
                        subdelimiter,
                        existing_resource,
                        new_resource,
                        default_set,
                        case_sensitive,
                        escape,
                        nullvalue,
                        validation,
                        &mut cursors,
                    ) {
                        return Err(format!("Error parsing tsv line {}: {}", i + 1, e));
                    }
                }
            }
        }
    }

    if parsemode == Some(ParseMode::ReconstructText) {
        if verbose {
            eprintln!("Creating resources...");
        }
        for (filename, text) in texts {
            if verbose {
                eprintln!("Creating resource {} (length={})", filename, text.len());
            }
            match TextResourceBuilder::new()
                .with_text(text)
                .with_filename(&filename)
                .build()
            {
                Ok(resource) => {
                    if let Err(e) = store.insert(resource) {
                        return Err(format!("Error adding reconstructed text to store: {}", e));
                    }
                }
                Err(e) => {
                    return Err(format!("Error loading resource: {}", e));
                }
            }
        }
        if verbose {
            eprintln!("Parsing buffered rows...");
        }
        let parsemode = ParseMode::AlignWithText;
        let columns = columns.unwrap();
        for (i, line) in buffer.iter().enumerate() {
            if let Err(e) = parse_row(
                store,
                &line,
                &columns,
                parsemode,
                subdelimiter,
                existing_resource,
                new_resource,
                default_set,
                case_sensitive,
                escape,
                nullvalue,
                validation,
                &mut cursors,
            ) {
                return Err(format!(
                    "Error parsing tsv line {}: {}",
                    i + bufferbegin + 1,
                    e
                ));
            }
        }
    }
    Ok(())
}

fn reconstruct_text(
    line: &str,
    columns: &Columns,
    texts: &mut HashMap<String, String>,
    existing_resource: Option<&str>,
    new_resource: Option<&str>,
    output_delimiter: &str,
    buffered_delimiter: &mut Option<String>,
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
        parse_resource_file(&cells, columns, existing_resource, new_resource)?;
    let textcolumn = columns.index(&Column::Text);
    if !texts.contains_key(resource_file) {
        texts.insert(resource_file.to_string(), String::new());
    }
    if let Some(text) = texts.get_mut(resource_file) {
        if let Some(buffered_delimiter) = buffered_delimiter {
            text.push_str(&buffered_delimiter);
        }
        text.push_str(&cells[textcolumn.expect("there must be a text column")]);
        *buffered_delimiter = Some(output_delimiter.to_string());
    }
    Ok(())
}

/// Parse a row (`line`) of a TSV file (provided as string)
fn parse_row(
    store: &mut AnnotationStore,
    line: &str,
    columns: &Columns,
    parsemode: ParseMode,
    subdelimiter: &str,
    existing_resource: Option<&str>,
    new_resource: Option<&str>,
    default_set: Option<&str>,
    case_sensitive: bool,
    escape: bool,
    nullvalue: &str,
    validation: ValidationMode,
    cursors: &mut HashMap<TextResourceHandle, usize>,
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
        parse_resource_file(&cells, columns, existing_resource, new_resource)?;
    let resource_handle: TextResourceHandle = get_resource_handle(store, resource_file)?;
    let textcolumn = columns.index(&Column::Text);
    let selector = match parsemode {
        ParseMode::Simple => build_selector(&cells, columns, resource_handle)?,
        ParseMode::AlignWithText => align_with_text(
            store,
            resource_handle,
            &cells,
            textcolumn.expect("text column is required when parsemode is set to AlignWithText"),
            case_sensitive,
            cursors,
        )?,
        _ => return Err("Not implemented yet".to_string()),
    };
    let mut annotationbuilder = build_annotation(
        &cells,
        columns,
        default_set,
        subdelimiter,
        escape,
        nullvalue,
    )?;
    annotationbuilder = annotationbuilder.with_target(selector);
    match store.annotate(annotationbuilder) {
        Err(e) => return Err(format!("{}", e)),
        Ok(handle) => {
            if parsemode == ParseMode::Simple {
                if let Some(textcolumn) = textcolumn {
                    validate_text(store, handle, &cells, textcolumn, validation)?;
                }
            }
        }
    }
    Ok(())
}

fn align_with_text<'a>(
    store: &AnnotationStore,
    resource_handle: TextResourceHandle,
    cells: &[&str],
    textcolumn: usize,
    case_sensitive: bool,
    cursors: &mut HashMap<TextResourceHandle, usize>,
) -> Result<SelectorBuilder<'a>, String> {
    let textfragment = cells[textcolumn];
    if textfragment.is_empty() {
        return Err("Value in text column can not be empty".to_string());
    }
    let cursor = cursors.entry(resource_handle).or_insert(0);
    let resource = store
        .resource(&BuildItem::from(resource_handle))
        .expect("resource must exist");
    let searchtext = resource
        .textselection(&Offset::new(
            Cursor::BeginAligned(*cursor),
            Cursor::EndAligned(0),
        ))
        .map_err(|e| format!("{}", e))?;
    if let Some(foundtextselection) = if case_sensitive {
        searchtext.find_text(textfragment).next()
    } else {
        searchtext.find_text_nocase(textfragment).next() //MAYBE TODO: this will be sub-optimal on large texts as it is lowercased each time -> use a smaller text buffer
    } {
        *cursor = foundtextselection.end();
        Ok(SelectorBuilder::textselector(
            resource_handle,
            Offset::simple(foundtextselection.begin(), foundtextselection.end()),
        ))
    } else {
        return Err(format!(
            "Unable to align specified text with the underlying resource: '{}' (lost track after character position {})",
            textfragment,
            *cursor
        ));
    }
}

fn validate_text(
    store: &AnnotationStore,
    annotation_handle: AnnotationHandle,
    cells: &[&str],
    textcolumn: usize,
    validation: ValidationMode,
) -> Result<(), String> {
    if validation == ValidationMode::No {
        return Ok(());
    }
    if let Some(annotation) = store.annotation(annotation_handle) {
        let text: Vec<&str> = annotation.text().collect();
        if text.is_empty() {
            return Err("No text found".to_string());
        } else if text.len() == 1 {
            if !match validation {
                ValidationMode::Strict => {
                    &text[0] == cells.get(textcolumn).expect("cell must exist")
                }
                ValidationMode::Loose => {
                    text[0].to_lowercase()
                        == cells
                            .get(textcolumn)
                            .expect("cell must exist")
                            .to_lowercase()
                }
                ValidationMode::No => true,
            } {
                return Err(format!(
                    "Text validation failed, TSV expects '{}', data has '{}'",
                    cells.get(textcolumn).unwrap(),
                    &text[0]
                ));
            }
        } else {
            let text: String = text.join(" ");
            if !match validation {
                ValidationMode::Strict => {
                    &text.as_str() == cells.get(textcolumn).expect("cell must exist")
                }
                ValidationMode::Loose => {
                    text.to_lowercase()
                        == cells
                            .get(textcolumn)
                            .expect("cell must exist")
                            .to_lowercase()
                }
                ValidationMode::No => true,
            } {
                return Err(format!(
                    "Text validation failed, TSV expects '{}', data has '{}'",
                    cells.get(textcolumn).unwrap(),
                    &text.as_str()
                ));
            }
        }
    } else {
        return Err("Annotation not found (should never happen)".to_string());
    }
    Ok(())
}

fn unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prevc = None;
    let mut do_unescape: bool = false;
    for c in s.chars() {
        if c == '\\' && prevc != Some('\\') {
            do_unescape = true;
        }
        if do_unescape {
            match c {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                _ => {
                    result.push('\\');
                    result.push(c);
                }
            }
        } else {
            result.push(c)
        }
        prevc = Some(c);
        do_unescape = false;
    }
    result
}

fn build_annotation<'a>(
    cells: &'a [&'a str],
    columns: &Columns,
    default_set: Option<&'a str>,
    subdelimiter: &str,
    escape: bool,
    nullvalue: &str,
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
            databuilder = databuilder.with_id(BuildItem::IdRef(id));
        } else if let Some(default_set) = default_set {
            databuilder = databuilder.with_id(BuildItem::IdRef(default_set));
        }
        if let Some(i) = columns.index(&Column::AnnotationDataSet) {
            let set = cells.get(i).expect("cell must exist");
            databuilder = databuilder.with_dataset(BuildItem::Id(set.to_string()));
        }
        let key = cells.get(ikey).expect("cell must exist");
        let value = cells.get(ivalue).expect("cell must exist");
        if !value.is_empty() && *value != nullvalue {
            if value.find(subdelimiter).is_some() {
                for value in value.split(subdelimiter) {
                    let mut multidatabuilder = AnnotationDataBuilder::new();
                    if let Some(i) = columns.index(&Column::AnnotationDataSet) {
                        let set = cells.get(i).expect("cell must exist");
                        multidatabuilder =
                            multidatabuilder.with_dataset(BuildItem::Id(set.to_string()));
                    }
                    multidatabuilder = multidatabuilder.with_key(BuildItem::from(*key));
                    if escape {
                        multidatabuilder =
                            multidatabuilder.with_value(DataValue::from(unescape(value)));
                    } else {
                        multidatabuilder = multidatabuilder.with_value(DataValue::from(value));
                    }
                    annotationbuilder = annotationbuilder.with_data_builder(multidatabuilder);
                }
            } else {
                databuilder = databuilder.with_key(BuildItem::from(*key));
                if escape {
                    databuilder = databuilder.with_value(DataValue::from(unescape(value)));
                } else {
                    databuilder = databuilder.with_value(DataValue::from(*value));
                }
                annotationbuilder = annotationbuilder.with_data_builder(databuilder);
            }
        }
    }
    //process custom columns
    for (column, cell) in columns.iter().zip(cells.iter()) {
        if let Column::Custom { set, key } = column {
            if cell.find(subdelimiter).is_some() {
                for value in cell.split(subdelimiter) {
                    let value: DataValue = if escape {
                        unescape(value).into()
                    } else {
                        value.into()
                    };
                    let databuilder = AnnotationDataBuilder::new()
                        .with_dataset(BuildItem::Id(set.clone()))
                        .with_key(BuildItem::Id(key.clone()))
                        .with_value(value);
                    annotationbuilder = annotationbuilder.with_data_builder(databuilder);
                }
            } else {
                let value: DataValue = if escape {
                    unescape(cell).into()
                } else {
                    (*cell).into()
                };
                let databuilder = AnnotationDataBuilder::new()
                    .with_dataset(BuildItem::Id(set.clone()))
                    .with_key(BuildItem::Id(key.clone()))
                    .with_value(value);
                annotationbuilder = annotationbuilder.with_data_builder(databuilder);
            }
        }
    }
    Ok(annotationbuilder)
}

fn parse_resource_file<'a>(
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
        Err(
            "Can't find resource (data doesn't make an explicit reference to it). You may want to specify a default (existing) resource using --resource"
                .to_string(),
        )
    }
}

fn get_resource_handle(
    store: &mut AnnotationStore,
    filename: &str,
) -> Result<TextResourceHandle, String> {
    if let Some(resource) = store.resource(filename) {
        return Ok(resource.handle());
    }
    store
        .add_resource_from_file(filename)
        .map_err(|e| format!("Specified resource not found: {}: {}", filename, e))
}

fn build_selector<'a>(
    cells: &[&str],
    columns: &Columns,
    resource_handle: TextResourceHandle,
) -> Result<SelectorBuilder<'a>, String> {
    //TODO: for now this only returns a TextSelector, should be adapted to handle multiple offsets (with subdelimiter) and return a CompositeSelector then
    let offset = parse_offset(cells, columns)?;
    Ok(SelectorBuilder::textselector(resource_handle, offset))
}

fn parse_offset(cells: &[&str], columns: &Columns) -> Result<Offset, String> {
    if let Some(ioffset) = columns.index(&Column::Offset) {
        let cell = cells.get(ioffset).expect("cell must exist");
        if let Some(delimiterpos) = &cell[1..].find('-') {
            let delimiterpos = *delimiterpos + 1; //we do 1 rather than 0 to not consider an immediate hyphen after the # , that would indicate a negative begin index
            let begin_str = &cell[0..delimiterpos];
            let end_str = &cell[(delimiterpos + 1)..];
            let begin: Cursor = begin_str.try_into().map_err(|e| format!("{}", e))?;
            let end: Cursor = end_str.try_into().map_err(|e| format!("{}", e))?;
            return Ok(Offset::new(begin, end));
        }
        Err("Offset must have format: beginoffset-endoffset".to_string())
    } else if let (Some(b), Some(e)) = (
        columns.index(&Column::BeginOffset),
        columns.index(&Column::EndOffset),
    ) {
        let begin_str = cells.get(b).expect("cell must exist");
        let end_str = cells.get(e).expect("cell must exist");
        let begin: Cursor = (*begin_str).try_into().map_err(|e| format!("{}", e))?;
        let end: Cursor = (*end_str).try_into().map_err(|e| format!("{}", e))?;
        Ok(Offset::new(begin, end))
    } else if let Some(i) = columns.index(&Column::TextSelection) {
        let textselection = cells.get(i).expect("cell must exist");
        if let Some(bytepos) = textselection.find('#') {
            if let Some(delimiterpos) = &textselection[(bytepos + 2)..].find('-') {
                let delimiterpos = *delimiterpos + bytepos + 2; //we do 2 rather than 1 to not consider an immediate hyphen after the # , that would indicate a negative begin index
                let begin_str = &textselection[(bytepos + 1)..delimiterpos];
                let end_str = &textselection[(delimiterpos + 1)..];
                let begin: Cursor = (*begin_str).try_into().map_err(|e| format!("{}", e))?;
                let end: Cursor = (*end_str).try_into().map_err(|e| format!("{}", e))?;
                return Ok(Offset::new(begin, end));
            }
        }
        Err("Text selection must have format: resource#beginoffset-endoffset".to_string())
    } else {
        Err(format!("No offset information found"))
    }
}

fn parse_column(
    column: &str,
    default_set: Option<&str>,
    setdelimiter: &str,
) -> Result<Column, String> {
    let result = Column::parse(column, setdelimiter)
        .map_err(|err| format!("Unable to parse provided columns: {}", err));
    if result.is_err() && default_set.is_some() {
        return Ok(Column::Custom {
            set: default_set.unwrap().to_string(),
            key: column.to_string(),
        });
    } else {
        result
    }
}
