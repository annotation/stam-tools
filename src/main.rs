use clap::{App, Arg, ArgAction, ArgMatches, SubCommand};
use stam::{AnnotationStore, AssociatedFile, Config, WebAnnoConfig, TransposeConfig};
use std::path::Path;
use std::process::exit;
use std::collections::VecDeque;

use stamtools::*;
use stamtools::align::*;
use stamtools::view::*;
use stamtools::grep::*;
use stamtools::tsv::*;
use stamtools::query::*;
use stamtools::tag::*;
use stamtools::to_text::*;
use stamtools::validate::*;
use stamtools::info::*;
use stamtools::annotate::*;
use stamtools::transpose::*;

fn common_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("verbose")
            .long("verbose")
            .short('V')
            .help("Produce verbose output")
            .required(false),
    );
    args.push(
        Arg::with_name("dry-run")
            .long("dry-run")
            .short('n')
            .help("Dry run, do not write changes to file")
            .required(false),
    );
    args
}

fn store_argument<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("annotationstore")
            .help(
                "Input and output file for the annotation store (will be overwritten if it already exists!). Set to - for standard input/output. Note that for 'stam init', this is only used as output.",
            )
            .takes_value(true)
            .required(true),
    );
    args
}

fn multi_store_arguments<'a>(required: bool) -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("annotationstore")
            .help(
                "Input file containing an annotation store in STAM JSON or STAM CSV. Set value to - for standard input. Multiple are allowed.",
            )
            .takes_value(true)
            .required(required)
            .action(ArgAction::Append),
    );
    args
}

fn config_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("debug")
            .long("debug")
            .short('d')
            .help("Set debug mode for the underlying library")
            .required(false),
    );
    args.push(
        Arg::with_name("no-include")
            .long("no-include")
            .short('I')
            .help("Serialize as one file, do not output @include directives nor standoff-files")
            .required(false),
    );
    args.push(
        Arg::with_name("strip-ids")
            .long("strip-ids")
            .help("Strip public identifiers for annotations and annotation data (may save considerable memory)")
            .required(false),
    );
    args
}

fn format_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("format")
            .long("format")
            .short('F')
            .help("Output format, can be 'tsv', or 'w3anno' (W3C Web Annotation in JSON Lines output, i.e. one annotation in JSON-LD per line)")
            .takes_value(true)
            .default_value("tsv"),
    );
    args
}

fn w3anno_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("annotation-prefix")
            .long("annotation-prefix")
            .help("(for Web Annotation output only): default prefix when forming IRIs from STAM public identifiers for annotations")
            .takes_value(true)
            .default_value("_:"),
    );
    args.push(
        Arg::with_name("dataset-prefix")
            .long("dataset-prefix")
            .help("(for Web Annotation output only): default prefix when forming IRIs from STAM public identifiers for annotation data sets")
            .takes_value(true)
            .default_value("_:"),
    );
    args.push(
        Arg::with_name("resource-prefix")
            .long("resource-prefix")
            .help("(for Web Annotation output only): default prefix when forming IRIs from STAM public identifiers for text resources")
            .takes_value(true)
            .default_value("_:"),
    );
    args.push(
        Arg::with_name("no-generator")
            .long("no-generator")
            .help("(for Web Annotation output only) Do not output an automatically generated 'generator' predicate")
            .required(false),
    );
    args.push(
        Arg::with_name("no-generated")
            .long("no-generated")
            .help("(for Web Annotation output only) Do not output an automatically generated 'generated' predicate")
            .required(false),
    );
    args.push(
        Arg::with_name("add-context")
            .long("add-context")
            .help("(for Web Annotation output only) URL to a JSONLD context to include")
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("namespaces")
            .long("ns")
            .help("(for Web Annotation output only) Add a namespace to the JSON-LD context, syntax is: namespace: uri")
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args
}

/// Translate command line arguments to stam library's configuration structure
fn config_from_args(args: &ArgMatches) -> Config {
    Config::default()
        .with_use_include(!args.is_present("no-include"))
        .with_debug(args.is_present("debug"))
}

fn tsv_arguments_common<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("subdelimiter")
            .long("subdelimiter")
            .help("Delimiter for multiple values in a single column")
            .takes_value(true)
            .default_value("|"),
    );
    args.push(
        Arg::with_name("setdelimiter")
            .long("setdelimiter")
            .help(
                "The delimiter between the annotation set and the key in custom columns. If the delimiter occurs multiple times, only the rightmost one is considered (the others are part of the set)"
            )
            .takes_value(true)
            .default_value("/"),
    );
    args.push(
        Arg::with_name("null")
            .long("null")
            .help("Text to use for NULL values")
            .takes_value(true)
            .default_value("-"),
    );
    args
}

fn tsv_arguments_out<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = tsv_arguments_common();
    args.push(
        Arg::with_name("type")
            .long("type")
            .help("Select the data type to focus on for the TSV output. If you supply a --query then there is no need to supply this as well.")
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
        Arg::with_name("columns")
            .long("columns")
            .short('C')
            .help("Column Format, comma separated list of column names to output")
            .long_help(
                "In most cases, you do not need to explicitly specify this as it will be automatically guessed based on the --type or --query parameter.
However, if you want full control, you can choose from the following known columns names (case insensitive, comma seperated list):

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

In addition to the above columns, you may also set a *custom* column by specifying an
AnnotationDataSet and DataKey within, seperated by the set/key delimiter (by default a slash). The
rows will then be filled with the data values corresponding to the data key. Example:

* my_set/part_of_speech
* my_set/lemma
",
            )
            .takes_value(true),
    );
    args.push(
        Arg::with_name("strict-columns")
            .long("strict-columns")
            .short('x')
            .help(
            "Do not automatically add columns based on constraints found in the specified query",
        ),
    );
    args.push(
        Arg::with_name("no-header")
            .long("no-header")
            .short('H')
            .help("Do not output a header on the first line")
            .takes_value(false),
    );
    args.push(
        Arg::with_name("alignments")
            .long("alignments")
            .short('A')
            .alias("transpositions")
            .help(
            "Output alignments (transpositions and translations). This overrides the --column specification and outputs the following tab separated columns instead: annotation ID, resource 1, offset 1, resource 2, offset 2, text 1, text 2 ",
        ),
    );
    args
}

fn tsv_arguments_in<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = tsv_arguments_common();
    args.push(
        Arg::with_name("columns")
            .long("columns")
            .short('C')
            .help("Column Format, comma separated list of column names to output")
            .long_help(
                "Choose from the following known columns names (case insensitive, comma seperated list):

* Id                   - The ID of the annotation item
* Annotation           - (same as above) 
* AnnotationData       - The ID of the annotation data, used with `DataKey` and `DataValue`
* AnnotationDataSet    - The ID of the associated AnnotationDataSet
* TextResource         - The ID/filename of text resource, IDs are assumed to be filenames by this importer
* DataKey              - The key
* DataValue            - The value
* TextSelection        - A combination of resource identifier(s) with an offset in the following format: resource#beginoffset-endoffset
* Text                 - The text of the selection, target of the annotation 
* Offset               - Offset in unicode character points (0-indexed, end is non-inclusive) seperated by a hyphen: beginoffset-endoffset
* BeginOffset          - Begin offset in unicode character points
* EndOffset            - End offset in unicode character points
* BeginUtf8Offset      - Begin offset in UTF-8 bytes
* EndUtf8Offset        - End offset in UTF8-bytes

In addition of the above columns, you may also parse a *custom* column by specifying an AnnotationDataSet and DataKey , separated by the set/key delimiter (by default a slash). Example:

* my_set/part_of_speech
* my_set/lemma

",
            )
            .takes_value(true),
    );
    args.push(
        Arg::with_name("no-header")
            .long("no-header")
            .short('H')
            .help("Data starts on the first line, there is no header row")
            .takes_value(false),
    );
    args.push(
        Arg::with_name("no-seq")
            .long("no-seq")
            .short('Q')
            .help("Rows in TSV file are not sequential but in arbitrary order"),
    );
    args.push(
        Arg::with_name("inputfile")
            .long("inputfile")
            .short('f')
            .help("TSV file to import. This option may be specified multiple times.")
            .action(ArgAction::Append)
            .required(true)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("resource")
            .long("resource")
            .help("Interpret data in the TSV file as pertaining to this existing text resource (a plain text file), unless made explicit in the data otherwise. The file must be present and will be loaded. If necessary, data will be aligned automatically to this resource.")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("new-resource")
            .long("new-resource")
            .help(
                "Interpret data in the TSV file as pertaining to this text resource, and reconstruct it from the data. Will write a separate txt file unless you provide the --no-include option.",
            )
            .takes_value(true),
    );
    args.push(
        Arg::with_name("annotationset")
            .long("annotationset")
            .help(
                "Interpret data in the TSV file as pertaining to this annotation set (unless made explicit in the data otherwise)",
            )
            .takes_value(true),
    );
    args.push(
        Arg::with_name("validate")
            .long("validate")
            .help(
                "Do text validation, values: strict, loose (case insensitive testing, this is the default), no"
            )
            .default_value("loose")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("no-case")
            .long("no-case")
            .help("Do case insensitive matching when attempting to align text from the TSV input with a text resource"),
    );
    args.push(
        Arg::with_name("no-escape")
            .long("no-escape")
            .help("Do not parse escape sequences for tabs (\\t) and newlines (\\n), leave as is"),
    );
    args.push(Arg::with_name("no-comments").long("no-comments").help(
        "Do not allow comments, if not set, all lines starting with # are treated as comments",
    ));
    args.push(
        Arg::with_name("outputdelimiter")
            .long("outputdelimiter")
            .help("Output delimiter when reconstructing text, after each row, this string is outputted. In most scenarios, like when having one word per row, you'll want this to be a space (which is the default).")
            .takes_value(true)
            .default_value(" "),
    );
    args.push(
        Arg::with_name("outputdelimiter2")
            .long("outputdelimiter2")
            .help("Output delimiter when reconstructing text and when an empty line is found in the input data. In most scenarios, you will want this to be a newline (the default)")
            .takes_value(true)
            .default_value("\n"),
    );
    args
}

fn align_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("use2")
            .long("use2")
            .help(
                "Name of the variable from the *second* --query to use. If not set, the last defined subquery will be used (still pertaining to the second --query statement!)"
            )
            .takes_value(true)
    );
    args.push(
        Arg::with_name("resource")
            .long("resource")
            .short('r')
            .help(
                "The ID of the resource to align; specify this argument twice. It is an alternative to specifying two full --query parameters"
            )
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("simple-only")
            .long("simple-only")
            .help("Only allow for alignments that consist of one contiguous text selection on either side. This is a so-called simple transposition."),
    );
    args.push(
        Arg::with_name("ignore-case")
            .long("ignore-case")
            .help("Do case-insensitive matching, this has more performance overhead"),
    );
    args.push(
        Arg::with_name("global")
            .long("global")
            .help("Perform global alignment instead of local"),
    );
    args.push(
        Arg::with_name("algorithm")
            .long("algorithm")
            .takes_value(true)
            .default_value("smith_waterman")
            .help("Alignment algorithm, can be smith_waterman (default) or needleman_wunsch"),
    );
    args.push(
        Arg::with_name("id-prefix")
            .long("id-prefix")
            .takes_value(true)
            .help("Prefix to use when assigning annotation IDs. The actual ID will have a random component."),
    );
    args.push(
        Arg::with_name("match-score")
            .long("match-score")
            .takes_value(true)
            .default_value("2")
            .help("Score for matching alignments, positive integer"),
    );
    args.push(
        Arg::with_name("mismatch-score")
            .long("mismatch-score")
            .takes_value(true)
            .default_value("-1")
            .help("Score for mismatching alignments, negative integer"),
    );
    args.push(
        Arg::with_name("insertion-score")
            .long("insertion-score")
            .takes_value(true)
            .default_value("-1")
            .help("Score for insertions (gap penalty), negative integer"),
    );
    args.push(
        Arg::with_name("deletion-score")
            .long("deletion-score")
            .takes_value(true)
            .default_value("-1")
            .help("Score for deletions (gap penalty), negative integer"),
    );
    args
}

fn transpose_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("transposition")
            .long("transposition")
            .short('T')
            .help("A query in STAMQL to retrieve the transposition annotation, or the exact transposition ID. See
                https://github.com/annotation/stam/tree/master/extensions/stam-query for an
                explanation of the query language's syntax. The query should produce only one result (if
                not only the first is taken). If you have the exact ID of the transposition already, then
                simply use `SELECT ANNOTATION WHERE ID \"your-id\";`. Use may use one --transposition parameter for each --query parameter (in the same order).")
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("use-transposition")
            .long("use-transposition")
            .help(
                "Name of the variable from the  --transposition queries to use (must be the same for all). If not set, the last defined subquery will be used"
            )
            .takes_value(true)
    );
    args.push(
        Arg::with_name("id-prefix")
            .long("id-prefix")
            .takes_value(true)
            .help("Prefix to use when assigning annotation IDs."),
    );
    args.push(
        Arg::with_name("no-transpositions")
            .long("no-transpositions")
            .help("Do not produce transpositions. Only the transposed annotations will be produced. This essentially throws away provenance information."),
    );
    args.push(
        Arg::with_name("no-resegmentations")
            .long("no-resegmentations")
            .help("Do not produce resegmentations. Only the resegmented annotations will be produced if needed. This essentially throws away provenance information."),
    );
    args.push(
        Arg::with_name("ignore-errors")
            .long("ignore-errors")
            .help("Skip annotations that can not be transposed successfully and output a warning, this would produce a hard failure otherwise"),
    );
    args
}

fn query_arguments<'a>(help: &'static str) -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("query")
            .long("query")
            .short('q')
            .help(help)
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("use")
            .long("use")
            .help(
                "Name of the variable from --query to use for the main output. If not set, the last defined subquery will be used (still pertaining to the first --query statement!)"
            )
            .takes_value(true)
    );
    args
}

fn annotate_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("annotationsets")
            .long("annotationset")
            .short('s')
            .help("STAM JSON file containing an annotation data set. Set value to - for standard input.")
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("resources")
            .long("resource")
            .short('r')
            .help("Plain text or STAM JSON file containing a text resource. Set value to - for standard input.")
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("stores")
            .long("store")
            .short('i')
            .help(
                "STAM JSON or STAM CSV file containing an annotation store, will be merged into the new store. Set value to - for standard input.",
            )
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("annotations")
            .long("annotations")
            .short('a')
            .help("STAM JSON file containing an array of annotations, will be merged into the new store. Set value to - for standard input.")
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("id")
            .long("id")
            .help("Sets the identifier for the annotation store")
            .takes_value(true),
    );
    args
}

fn load_store(args: &ArgMatches) -> AnnotationStore {
    let filename = args
        .value_of("annotationstore")
        .expect("an annotation store must be provided");
    let mut store =
        AnnotationStore::from_file(filename, config_from_args(args)).unwrap_or_else(|err| {
            eprintln!("Error loading annotation store: {}", err);
            exit(1);
        });
    if args.is_present("strip-ids") {
        store.strip_data_ids();
        store.strip_annotation_ids();
    }
    store
}


fn main() {
    let rootargs = App::new("STAM Tools")
        .version(VERSION)
        .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>, KNAW Humanities Cluster")
        .about("CLI tool to work with standoff text annotation (STAM)")
        .subcommand(
            SubCommand::with_name("info")
                .about("Return information regarding a STAM model. Set --verbose for extra details.")
                .args(&common_arguments())
                .args(&multi_store_arguments(true))
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("validate")
                .about("Validate a STAM model. Set --verbose to have it output the STAM JSON or STAM CSV to standard output.")
                .args(&common_arguments())
                .args(&multi_store_arguments(true))
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("save")
                .about("Save the annotation store and all underlying files to the the specified location and data format (detected by extension).")
                .args(&common_arguments())
                .args(&store_argument())
                .args(&config_arguments())
                .arg(
                    Arg::with_name("outputfile")
                        .long("outputfile")
                        .short('o')
                        .help(
                            "Output filename for the annotation store, other filenames will be derived automatically. You can use the following extensions:
                                * .json (recommended: .store.json) - STAM JSON - Very verbose but also more interopable.
                                * .csv (recommended: .store.csv) - STAM CSV - Not very verbose, less interoperable",
                        )
                        .takes_value(true)
                        .required(true)
                ),
        )
        .subcommand(
            SubCommand::with_name("export")
                .about("Export annotations (or other data structures) as tabular data to a TSV format. If --verbose is set, a tree-like structure is expressed in which the order of rows matters.")
                .args(&common_arguments())
                .args(&multi_store_arguments(true))
                .args(&config_arguments())
                .args(&format_arguments())
                .args(&tsv_arguments_out())
                .args(&w3anno_arguments())
                .args(&query_arguments("
A query in STAMQL. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. Only one query (with possible subqueries) is allowed.
"))
        )
        .subcommand(
            SubCommand::with_name("import")
                .about("Import annotations from a TSV format.")
                .args(&common_arguments())
                .args(&store_argument())
                .args(&config_arguments())
                .args(&tsv_arguments_in()),
        )
        .subcommand(
            SubCommand::with_name("query")
                .about("Query annotations by data and output results to a TSV format. If --verbose is set, a tree-like structure is expressed in which the order of rows matters.")
                .args(&common_arguments())
                .args(&multi_store_arguments(true))
                .args(&config_arguments())
                .args(&format_arguments())
                .args(&tsv_arguments_out())
                .args(&w3anno_arguments())
                .args(&query_arguments("
A query in STAMQL. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. Only one query (with possible subqueries) is allowed.
"))
        )
        .subcommand(
            SubCommand::with_name("print")
                .about("Output the plain text of given a query, or of all resources if no query was provided")
                .args(&common_arguments())
                .args(&multi_store_arguments(true))
                .args(&config_arguments())
                .args(&query_arguments("
A query in STAMQL. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. Only one query (with possible subqueries) is allowed.
"))
        )
        .subcommand(
            SubCommand::with_name("view")
                .about("Output the text and annotations of one or more resource(s) in HTML, suitable for visualisation in a browser. Requires --resource")
                .args(&common_arguments())
                .args(&multi_store_arguments(true))
                .args(&config_arguments())
                .args(&query_arguments("One or more queries in STAMQL. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. You can specify multiple queries here by repeating the parameter, the first query is the primary selection query and determines what text is shown. Any subsequent queries are highlight queries and determine what is highlighted. You can prepend the following *attributes* to the query (before the SELECT statement), to determine how things are visualised:

* @KEYTAG - Outputs a tag with the key, pertaining to the first DATA/KEY constraint in the query
* @KEYVALUETAG - Outputs a tag with the key and the value, pertaining to the first DATA/KEY constraint in the query
* @VALUETAG - Outputs a tag with the value only, pertaining to the first DATA/KEY constraint in the query
* @IDTAG - Outputs a tag with the public identifier of the ANNOTATION that has been selected

If no attribute is provided, there will be no tags shown for that query, only a highlight underline. In the highlight queries, the variable from the main selection query is available and you *should* use it in a constraint, otherwise performance will be sub-optimal.
" ))
                .arg(
                    Arg::with_name("highlight")
                        .long("highlight")
                        .help(
                            "Define an annotation set and key which you want to highlight in the results. This will highlight text pertaining to annotations that have this data and output the key and value in a tag.

                             This option can be provided multiple times and is essentialy a shortcut for a certain type of --query. The set and key are delimited by the set delimiter (by default a /), which is configurable via --setdelimiter"
                        )
                        .action(ArgAction::Append)
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("format")
                        .long("format")
                        .short('F')
                        .help(
                            "The output format, can be set to 'html' (default) or 'ansi' (coloured terminal output)"
                        )
                        .takes_value(true)
                        .default_value("html")
                )
                .arg(
                    Arg::with_name("setdelimiter")
                        .long("setdelimiter")
                        .help(
                            "The delimiter between the annotation set and the key when specifying highlights (--highlight). If the delimiter occurs multiple times, only the rightmost one is considered (the others are part of the set)"
                        )
                        .takes_value(true)
                        .default_value("/")
                )
                .arg(
                    Arg::with_name("auto-highlight")
                        .long("auto")
                        .short('a')
                        .help(
                        "Automatically add highlights based on DATA constraints found in the main (first) query",
                    ),
                )
                .arg(
                    Arg::with_name("prune")
                        .long("prune")
                        .help(
                        "Prune results to show only the highlights",
                    ),
                )
                .arg(
                    Arg::with_name("no-legend")
                        .long("no-legend")
                        .help("Do not output a legend",
                    )
                )
                .arg(
                    Arg::with_name("no-titles")
                        .long("no-titles")
                        .help("Do not output titles (identifiers) for the primary selected items",
                    )
                )
        )
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialize a new stam annotationstore")
                .args(&common_arguments())
                .args(&store_argument())
                .args(&annotate_arguments())
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("annotate")
                .about("Add annotations (or datasets, resources) to an existing annotationstore")
                .args(&annotate_arguments())
                .args(&store_argument())
                .args(&common_arguments())
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("tag")
                .about("Regular-expression based tagger on plain text")
                .args(&common_arguments())
                .args(&store_argument())
                .args(&config_arguments())
                .arg(
                    Arg::with_name("rules")
                        .long("rules")
                        .help(
                            "A TSV file containing regular expression rules for the tagger.",
                        )
                        .long_help("A TSV file containing regular expression rules for the tagger.
The file contains the following columns:

1. The regular expressions follow the following syntax: https://docs.rs/regex/latest/regex/#syntax
   The expression may contain one or or more capture groups containing the items that will be
   tagged, in that case anything else is considered context and will not be tagged.
2. The ID of annotation data set
3. The ID of the data key
4. The value to set. If this follows the syntax $1,$2,etc.. it will assign the value of that capture group (1-indexed).")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("allow-overlap")
                        .long("allow-overlap")
                        .short('O')
                        .help("Allow regular expression matches to overlap")
                        .required(false),
                ))
        .subcommand(
            SubCommand::with_name("grep")
                .about("Regular-expression based search on plain text")
                .args(&common_arguments())
                .args(&store_argument())
                .args(&config_arguments())
                .arg(
                    Arg::with_name("expression")
                        .long("expression")
                        .short('e')
                        .help(
                            "A regular expression.",
                        )
                        .long_help("
The regular expressions follow the following syntax: https://docs.rs/regex/latest/regex/#syntax
The expression may contain one or or more capture groups containing the items that will be
returned, in that case anything else is considered context and will not be returned.")
                        .takes_value(true)
                        .required(true)
                        .action(ArgAction::Append),
                )
                .arg(
                    Arg::with_name("allow-overlap")
                        .long("allow-overlap")
                        .short('O')
                        .help("Allow regular expression matches to overlap")
                        .required(false),
                ))
        .subcommand(
            SubCommand::with_name("align")
                .about("Aligns two (or more) texts; computes a transposition annotation that maps the two (See https://github.com/annotation/stam/tree/master/extensions/stam-transpose) and adds it to the store. The texts are retrieved from the first two queries (--query) or (as a shortcut) from the first two --resource parameters. In --verbose mode, the alignments will be outputted to standard output as tab separated values with the follows columns: resource 1, offset 1, resource 2, offset 2, text 1, text 2")
                .args(&common_arguments())
                .args(&store_argument())
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL to retrieve a text. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. 
You need to specify this parameter twice, the text of first query will be aligned with text of the second one. If specified more than twice, each text will be aligned (independently) with the first one"))
                .args(&align_arguments())
            )
        .subcommand(
            SubCommand::with_name("transpose")
                .about("Transpose annotations over a transposition, effectively mapping them from one coordinate system to another (See https://github.com/annotation/stam/tree/master/extensions/stam-transpose). The first query corresponds to the transposition, further queries correspond to the annotations to transpose via that transposition. The new transposed annotations (and the transpositions that produced them) will be added to the store.")
                .args(&common_arguments())
                .args(&store_argument())
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL to retrieve annotation(s). See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax.
The first query should retrieve the transposition annotation to transpose over, it should produce only one result. Subsequent queries are the annotations to transpose."))
                .args(&transpose_arguments())
            )
        .get_matches();

    let args = if let Some(args) = rootargs.subcommand_matches("info") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("save") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("export") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("query") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("import") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("print") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("view") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("validate") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("init") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("annotate") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("tag") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("grep") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("align") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("transpose") {
        args
    } else {
        eprintln!("No command specified, please see stam --help");
        exit(2);
    };

    let mut store = AnnotationStore::new(config_from_args(args));

    if rootargs.subcommand_matches("info").is_some()
        || rootargs.subcommand_matches("export").is_some()
        || rootargs.subcommand_matches("query").is_some()
        || rootargs.subcommand_matches("print").is_some()
        || rootargs.subcommand_matches("view").is_some()
        || rootargs.subcommand_matches("validate").is_some()
        || rootargs.subcommand_matches("align").is_some()
        || rootargs.subcommand_matches("transpose").is_some()
    {
        if args.is_present("annotationstore") {
            let storefiles = args
                .values_of("annotationstore")
                .unwrap()
                .collect::<Vec<&str>>();
            for (i, filename) in storefiles.iter().enumerate() {
                eprintln!("Loading annotation store {}", filename);
                if i == 0 {
                    store = AnnotationStore::from_file(filename, config_from_args(args))
                        .unwrap_or_else(|err| {
                            eprintln!("Error loading annotation store: {}", err);
                            exit(1);
                        });
                } else {
                    store = store.with_file(filename).unwrap_or_else(|err| {
                        eprintln!("Error loading annotation store: {}", err);
                        exit(1);
                    });
                }
            }
        }

        if args.is_present("strip-ids") {
            store.strip_data_ids();
            store.strip_annotation_ids();
        }
    }

    if rootargs.subcommand_matches("info").is_some() {
        info(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("save").is_some() {
        store = load_store(args);
        store.set_filename(args.value_of("outputfile").unwrap());
        if !args.is_present("dry-run") {
            store.save().unwrap_or_else(|err| {
                eprintln!(
                    "Failed to write annotation store {:?}: {}",
                    store.filename(),
                    err
                );
                exit(1);
            });
        }
    } else if rootargs.subcommand_matches("export").is_some()
        || rootargs.subcommand_matches("query").is_some()
    {
        let verbose = args.is_present("verbose");

        let querystring = args.value_of("query").into_iter().next().unwrap_or(
            if args.is_present("alignments") {
                "SELECT ANNOTATION ?annotation WHERE DATA \"https://w3id.org/stam/extensions/stam-transpose/\" \"Transposition\";"
            } else {
                //default in case no query was provided
                match args
                    .value_of("type")
                    .unwrap_or("Annotation")
                    .to_lowercase()
                    .as_str()
                {
                    "annotation" => "SELECT ANNOTATION ?annotation",
                    "key" | "datakey" => "SELECT DATAKEY ?key",
                    "data" | "annotationdata" => "SELECT DATA ?data",
                    "resource" | "textresource" => "SELECT RESOURCE ?resource",
                    "dataset" | "annotationset" => "SELECT DATASET ?dataset",
                    "text" | "textselection" => "SELECT TEXT ?textselection",
                    _ => {
                        eprintln!("Invalid --type specified");
                        exit(1);
                    }
                }
            },
        );
        let (query, _) = stam::Query::parse(querystring).unwrap_or_else(|err| {
            eprintln!("Query syntax error: {}", err);
            exit(1);
        });

        let resulttype = query.resulttype().expect("Query has no result type");

        if args.is_present("alignments") {
            if let Err(err) = alignments_tsv_out(&store, query, args.value_of("use")) {
                eprintln!("{}", err);
                exit(1);
            }
        } else if args.value_of("format") == Some("tsv") {
            let columns: Vec<&str> = if let Some(columns) = args.value_of("columns") {
                columns.split(",").collect()
            } else {
                match resulttype {
                    stam::Type::Annotation => {
                        if verbose {
                            vec![
                                "Type",
                                "Id",
                                "AnnotationDataSet",
                                "DataKey",
                                "DataValue",
                                "Text",
                                "TextSelection",
                            ]
                        } else {
                            vec!["Type", "Id", "Text", "TextSelection"]
                        }
                    }
                    stam::Type::DataKey => vec!["Type", "AnnotationDataSet", "Id"],
                    stam::Type::AnnotationData => {
                        vec!["Type", "Id", "AnnotationDataSet", "DataKey", "DataValue"]
                    }
                    stam::Type::TextResource => vec!["Type", "Id"],
                    stam::Type::AnnotationDataSet => vec!["Type", "Id"],
                    stam::Type::TextSelection => vec!["Type", "TextSelection", "Text"],
                    _ => {
                        eprintln!("Invalid --type specified");
                        exit(1);
                    }
                }
            };

            to_tsv(
                &store,
                query,
                &columns,
                args.is_present("verbose"),
                args.value_of("subdelimiter").unwrap(),
                args.value_of("null").unwrap(),
                !args.is_present("no-header"),
                args.value_of("setdelimiter").unwrap(),
                !args.is_present("strict-columns"),
            );
        } else if let Some("json") = args.value_of("format") {
            if let Err(err) = to_json(&store, query) {
                eprintln!("{}", err);
                exit(1);
            }
        } else if let Some("webanno") | Some("w3anno") | Some("jsonl") = args.value_of("format") {
            to_w3anno(
                &store,
                query,
                args.value_of("use").unwrap_or(match resulttype {
                    stam::Type::Annotation => "annotation",
                    stam::Type::TextSelection => "text",
                    _ => {
                        eprintln!("Web Annotation output only supports queries with result type ANNOTATION or TEXT");
                        exit(1);
                    }
                }),
                WebAnnoConfig {
                    default_annotation_iri: args.value_of("annotation-prefix").unwrap().to_string(),
                    default_set_iri: args.value_of("dataset-prefix").unwrap().to_string(),
                    default_resource_iri: args.value_of("resource-prefix").unwrap().to_string(),
                    auto_generated: !args.is_present("no-generated"),
                    auto_generator: !args.is_present("no-generator"),
                    extra_context: args.values_of("add-context").unwrap_or(clap::Values::default()).map(|x| x.to_string()).collect(),
                    context_namespaces: { 
                        let mut namespaces = Vec::new(); 
                        for assignment in args.values_of("namespaces").unwrap_or(clap::Values::default()) {
                            let result: Vec<_> = assignment.splitn(2,":").collect();
                            if result.len() != 2 {
                                eprintln!("Syntax for --ns should be `ns: uri_prefix`");
                                exit(1);
                            }
                            namespaces.push((result[1].trim().to_string(), result[0].trim().to_string()));
                        }
                        namespaces
                    },
                    ..WebAnnoConfig::default()
                },
            );
        } else {
            eprintln!("Invalid output format, specify 'tsv', 'json' or 'w3anno'");
            exit(1);
        }
    } else if rootargs.subcommand_matches("import").is_some() {
        let storefilename = args
            .value_of("annotationstore")
            .expect("an annotation store must be provided");
        let inputfiles = args.values_of("inputfile").unwrap().collect::<Vec<&str>>();
        if Path::new(storefilename).exists() {
            eprintln!("Existing annotation store found");
            store = load_store(args);
        } else {
            eprintln!("New annotation store created");
            store.set_filename(storefilename);
        }
        let columns: Option<Vec<&str>> = if args.is_present("columns") {
            Some(args.value_of("columns").unwrap().split(",").collect())
        } else {
            None
        };
        let existing_resource: Option<&str> = if args.is_present("resource") {
            Some(args.value_of("resource").unwrap())
        } else {
            None
        };
        let new_resource: Option<&str> = if args.is_present("new-resource") {
            Some(args.value_of("new-resource").unwrap())
        } else {
            None
        };
        for inputfile in inputfiles {
            from_tsv(
                &mut store,
                &inputfile,
                columns.as_ref(),
                existing_resource,
                new_resource,
                args.value_of("annotationset"),
                !args.is_present("no-comments"),
                !args.is_present("no-seq"),
                !args.is_present("no-case"),
                !args.is_present("no-escape"),
                args.value_of("null").unwrap(),
                args.value_of("subdelimiter").unwrap(),
                args.value_of("setdelimiter").unwrap(),
                args.value_of("outputdelimiter").unwrap(),
                args.value_of("outputdelimiter2").unwrap(),
                Some(!args.is_present("no-header")),
                ValidationMode::try_from(args.value_of("validate").unwrap()).unwrap_or_else(
                    |err| {
                        eprintln!("{}", err);
                        exit(1);
                    },
                ),
                args.is_present("verbose"),
            );
        }
        if !args.is_present("dry-run") {
            store.save().unwrap_or_else(|err| {
                eprintln!(
                    "Failed to write annotation store {:?}: {}",
                    store.filename(),
                    err
                );
                exit(1);
            });
        }
    } else if rootargs.subcommand_matches("print").is_some() {
        let querystring = args
            .value_of("query")
            .into_iter()
            .next()
            .unwrap_or("SELECT RESOURCE ?res");
        let (query, _) = stam::Query::parse(querystring).unwrap_or_else(|err| {
            eprintln!("[error] Query syntax error: {}", err);
            exit(1);
        });
        to_text(&store, query, args.value_of("use"));
    } else if rootargs.subcommand_matches("view").is_some() {
        let queries: Vec<&str> = args.values_of("query").unwrap_or_default().collect();
        let mut queries_iter = queries.into_iter();
        let querystring = queries_iter.next().unwrap_or("SELECT RESOURCE ?res;");
        let (query, _) = stam::Query::parse(querystring).unwrap_or_else(|err| {
            eprintln!("[error] Query syntax error in first query: {}", err);
            exit(1);
        });
        let mut highlights = Vec::new();
        for (i, highlightquery) in queries_iter.enumerate() {
            let highlight =
                Highlight::parse_query(highlightquery, &store, i + 1).unwrap_or_else(|err| {
                    eprintln!("[error] Syntax error in query {}: {}", i + 1, err);
                    exit(1);
                });
            highlights.push(highlight);
        }
        let setdelimiter = args.value_of("setdelimiter").unwrap();
        if let Some(extrahighlights) = args.values_of("highlight") {
            highlights.extend(extrahighlights.filter_map(|set_and_key: &str| {
                if set_and_key.find(setdelimiter).is_some() {
                    let (set, key) = set_and_key.rsplit_once(setdelimiter).unwrap();
                    if let Some(key) = store.key(set, key) {
                        Some(Highlight::default().with_tag(Tag::Key(key)))
                    } else {
                        eprintln!(
                            "[error] Key specified in highlight not found: {}{}{}",
                            set, setdelimiter, key
                        );
                        exit(1);
                    }
                } else {
                    None
                }
            }));
        }

        match args.value_of("format") {
            Some("html") => {
                let mut writer = HtmlWriter::new(&store, query);
                for highlight in highlights {
                    writer = writer.with_highlight(highlight);
                }

                if args.is_present("auto-highlight") {
                    writer.add_highlights_from_query();
                }
                if args.is_present("no-legend") {
                    writer = writer.with_legend(false)
                }
                if args.is_present("no-titles") {
                    writer = writer.with_titles(false)
                }
                if args.is_present("prune") {
                    writer = writer.with_prune(true);
                }
                if let Some(var) = args.value_of("use") {
                    eprintln!("[info] Selecting variable ?{}...", var);
                    writer = writer.with_selectionvar(var);
                }
                print!("{}", writer);
            }
            Some("ansi") => {
                let mut writer = AnsiWriter::new(&store, query);
                for highlight in highlights {
                    writer = writer.with_highlight(highlight);
                }
                if args.is_present("auto-highlight") {
                    writer.add_highlights_from_query();
                }
                if args.is_present("no-legend") {
                    writer = writer.with_legend(false)
                }
                if args.is_present("no-titles") {
                    writer = writer.with_titles(false)
                }
                if args.is_present("prune") {
                    writer = writer.with_prune(true);
                }
                if let Some(var) = args.value_of("use") {
                    eprintln!("[info] Selecting variable ?{}...", var);
                    writer = writer.with_selectionvar(var);
                }
                writer.print();
            }
            Some(s) => {
                eprintln!("[error] Unknown output format: {}", s);
            }
            None => unreachable!(),
        }
    } else if rootargs.subcommand_matches("validate").is_some() {
        validate(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("init").is_some()
        || rootargs.subcommand_matches("annotate").is_some()
    {
        if rootargs.subcommand_matches("annotate").is_some() {
            //load the store
            store = load_store(args);
        } else {
            //init: associate the filename with the pre-created store
            let filename = args
                .value_of("annotationstore")
                .expect("an annotation store must be provided");
            store.set_filename(filename);
        }
        let resourcefiles = args
            .values_of("resources")
            .unwrap_or_default()
            .collect::<Vec<&str>>();
        let setfiles = args
            .values_of("annotationsets")
            .unwrap_or_default()
            .collect::<Vec<&str>>();
        let storefiles = args
            .values_of("stores")
            .unwrap_or_default()
            .collect::<Vec<&str>>();
        let annotationfiles = args
            .values_of("annotations")
            .unwrap_or_default()
            .collect::<Vec<&str>>();
        eprintln!(
            "{} store: {} annotation(s), {} resource(s), {} annotationset(s), {} additional store(s)",
            if rootargs.subcommand_matches("annotate").is_some() {
                "Adding to"
            } else {
                "Initializing"
            }, 
            annotationfiles.len(),
            resourcefiles.len(),
            setfiles.len(),
            storefiles.len()
        );
        if rootargs.subcommand_matches("init").is_some() {
            if let Some(id) = args.value_of("id") {
                store = store.with_id(id.to_string());
            }
        }
        store = annotate(
            store,
            &resourcefiles,
            &setfiles,
            &storefiles,
            &annotationfiles,
        );
        if !args.is_present("dry-run") {
            store.save().unwrap_or_else(|err| {
                eprintln!(
                    "Failed to write annotation store {:?}: {}",
                    store.filename(),
                    err
                );
                exit(1);
            });
        }
    } else if rootargs.subcommand_matches("tag").is_some() {
        //load the store
        store = load_store(args);
        tag(
            &mut store,
            args.value_of("rules").expect("--rules must be provided"),
            args.is_present("allow-overlap"),
        );
        if !args.is_present("dry-run") {
            store.save().unwrap_or_else(|err| {
                eprintln!(
                    "Failed to write annotation store {:?}: {}",
                    store.filename(),
                    err
                );
                exit(1);
            });
        }
    } else if rootargs.subcommand_matches("grep").is_some() {
        //load the store
        store = load_store(args);
        grep(
            &store,
            args.values_of("expression")
                .expect("--expression must be provided")
                .collect(),
            args.is_present("allow-overlap"),
        );
    } else if rootargs.subcommand_matches("align").is_some() {
        //load the store
        store = load_store(args);

        let mut querystrings: Vec<_> = args.values_of("query").unwrap_or_default().map(|x| x.to_string()).collect();
        querystrings.extend( args.values_of("resource").unwrap_or_default().map(|x| format!("SELECT RESOURCE WHERE ID \"{}\"",x)) );

        let mut queries = VecDeque::new();
        for (i, querystring) in querystrings.iter().enumerate() {
            queries.push_back(stam::Query::parse(querystring.as_str()).unwrap_or_else(|err| {
                eprintln!("[error] Query syntax error query {}: {}", i+1, err);
                exit(1);
            }).0);
        }
        if queries.len() < 2 {
            eprintln!("[error] Expected at least two --query (or --resource) parameters");
            exit(1);
        }

        if let Err(err) = align(
            &mut store,
            queries.pop_front().unwrap(),
            queries.into_iter().collect(),
            args.value_of("use"),
            args.value_of("use2"),
            &AlignmentConfig {
                case_sensitive: !args.is_present("ignore-case"),
                algorithm: match args.value_of("algorithm") {
                    Some("smith_waterman") => AlignmentAlgorithm::SmithWaterman { 
                        equal: args.value_of("match-score").unwrap().parse().expect("score must be integer"),
                        align: args.value_of("mismatch-score").unwrap().parse().expect("score must be integer"),
                        insert: args.value_of("insertion-score").unwrap().parse().expect("score must be integer"),
                        delete: args.value_of("deletion-score").unwrap().parse().expect("score must be integer")
                    },
                    Some("needleman_wunsch") => AlignmentAlgorithm::NeedlemanWunsch  {
                        equal: args.value_of("match-score").unwrap().parse().expect("score must be integer"),
                        align: args.value_of("mismatch-score").unwrap().parse().expect("score must be integer"),
                        insert: args.value_of("insertion-score").unwrap().parse().expect("score must be integer"),
                        delete: args.value_of("deletion-score").unwrap().parse().expect("score must be integer")
                    },
                    Some(x) => {
                        eprintln!("[error] Not a valid alignment algorithm: {}, set smith_waterman or needleman_wunsch", x);
                        exit(1);
                    }
                    None => unreachable!("No alignment algorithm set")
                },
                alignment_scope: if args.is_present("global") {
                    AlignmentScope::Global
                } else {
                    AlignmentScope::Local
                },
                annotation_id_prefix: args.value_of("id-prefix").map(|x| x.to_string()),
                simple_only: args.is_present("simple-only"),
                verbose: args.is_present("verbose")
            }
        ) {
            eprintln!("[error] Alignment failed: {:?}", err);
            exit(1);
        }
        if !args.is_present("dry-run") {
            store.save().unwrap_or_else(|err| {
                eprintln!(
                    "Failed to write annotation store {:?}: {}",
                    store.filename(),
                    err
                );
                exit(1);
            });
        }
    } else if rootargs.subcommand_matches("transpose").is_some() {
        //load the store
        store = load_store(args);
        let transposition_querystrings: Vec<_> = args.values_of("transposition").unwrap_or_default().map(|q|
            if q.find(" ").is_some() {
                //already a query
                q.to_string()
            } else {
                //probably an ID, transform to query
                format!("SELECT ANNOTATION WHERE ID \"{}\";", q)
            }
        ).collect();

        let querystrings: Vec<_> = args.values_of("query").unwrap_or_default().collect();

        let mut transposition_queries = Vec::new();
        for (i, querystring) in transposition_querystrings.iter().enumerate() {
            transposition_queries.push(stam::Query::parse(querystring).unwrap_or_else(|err| {
                eprintln!("[error] Query syntax error query {}: {}", i+1, err);
                exit(1);
            }).0);
        }
        if transposition_queries.len() < 1 {
            eprintln!("[error] Expected at least one --transposition parameter");
            exit(1);
        }

        let mut queries = Vec::new();
        for (i, querystring) in querystrings.into_iter().enumerate() {
            queries.push(stam::Query::parse(querystring).unwrap_or_else(|err| {
                eprintln!("[error] Query syntax error query {}: {}", i+1, err);
                exit(1);
            }).0);
        }
        if queries.len() < 1 {
            eprintln!("[error] Expected at least one --query parameter");
            exit(1);
        }
        if let Err(err) = transpose(
            &mut store,
            transposition_queries,
            queries,
            args.value_of("use-transposition"),
            args.value_of("use"), 
            args.value_of("id-prefix").map(|x| x.to_string()),
            stam::IdStrategy::default(),
            args.is_present("ignore-errors"),
            args.is_present("verbose"),
            TransposeConfig {
                existing_source_side: true,
                no_transposition: args.is_present("no-transpositions"),
                no_resegmentation: args.is_present("no-resegmentations"),
                debug: args.is_present("debug"),
                ..Default::default()
            }) {
            eprintln!("[error] Transposition failed: {:?}", err);
            exit(1);
        }
        if !args.is_present("dry-run") {
            store.save().unwrap_or_else(|err| {
                eprintln!(
                    "Failed to write annotation store {:?}: {}",
                    store.filename(),
                    err
                );
                exit(1);
            });
        }

    }
}
