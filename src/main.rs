use clap::{App, Arg, ArgAction, ArgMatches, SubCommand};
use stam::{
    AnnotationStore, AssociatedFile, Config, TextResourceBuilder, TranslateConfig, TransposeConfig,
    WebAnnoConfig,
};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use std::process::exit;

use stamtools::align::*;
use stamtools::annotate::*;
use stamtools::grep::*;
use stamtools::info::*;
use stamtools::print::*;
use stamtools::query::*;
use stamtools::split::*;
use stamtools::tag::*;
use stamtools::to_text::*;
use stamtools::translate::*;
use stamtools::transpose::*;
use stamtools::tsv::*;
use stamtools::validate::*;
use stamtools::view::*;
use stamtools::xml::*;
use stamtools::*;

use stam::Offset;

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

const HELP_INPUT: &'static str = "Input file containing an annotation store in STAM JSON or STAM CSV. Set value to - for standard input. Multiple are allowed and will be merged into one. You may also provide *.txt files to be added to the store automatically.";
const HELP_INPUT_OUTPUT: &'static str = "Input file containing an annotation store in STAM JSON or STAM CSV. Set value to - for standard input. Multiple are allowed and will be merged into one. The *first* file mentioned also serves as output file unless --dry-run or --output is set. You may also provide *.txt files to be added to the store automatically.";
const HELP_OUTPUT_OPTIONAL_INPUT: &'static str = "Output file containing an annotation store in STAM JSON or STAM CSV. If the file exists, it will be loaded and augmented. Multiple store files are allowed but will only act as input and will be merged into one. (the *first* file mentioned).  If  --dry-run or --output is set, this will not be used for output.";
const SUBCOMMANDS: [&'static str; 18] = [
    "batch",
    "info",
    "export",
    "query",
    "import",
    "print",
    "view",
    "validate",
    "init",
    "annotate",
    "tag",
    "grep",
    "align",
    "transpose",
    "translate",
    "translatetext",
    "fromxml",
    "split",
];

fn store_arguments<'a>(input_required: bool, outputs: bool, batchmode: bool) -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    if !batchmode {
        if outputs {
            args.push(
                Arg::with_name("outputstore")
                    .long("output")
                    .short('o')
                    .help(
                        "The annotation store to use as output. If this is not specified and --dry-run is not set, the *first* annotation store that was used for input will also be used as output. The file type is derived rom the extension, you can use the following:
                        * .json (recommended: .store.json) - STAM JSON - Very verbose but also more interopable.
                        * .csv (recommended: .store.csv) - STAM CSV - Not very verbose, less interoperable",
                    )
                    .takes_value(true)
                    .required(false)
            );
            args.push(
                Arg::with_name("force-new")
                    .long("force-new")
                    .help("Force a new AnnotationStore, do not reload but simply overwrite any existing ones")
                    .required(false),
            );
        }
        args.push(
            Arg::with_name("annotationstore")
                .help(if !input_required && outputs {
                    HELP_OUTPUT_OPTIONAL_INPUT
                } else if outputs {
                    HELP_INPUT_OUTPUT
                } else {
                    HELP_INPUT
                })
                .takes_value(true)
                .multiple(true)
                .required(input_required),
        );
    }
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
            .help("(for Web Annotation output only) URL to a JSON-LD context to include. STAM Datasets with this ID will have their datakeys translated as-is (as aliases), leaving it up to the JSON-LD context to be interpreted.")
            .takes_value(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("no-auto-context")
            .long("no-auto-context")
            .help("(for Web Annotation output only) Do not automatically add STAM datasets ending in `.jsonld` or `.json` to the `--add-context` list")
            .required(false)
    );
    args.push(
        Arg::with_name("extra-target-template")
            .long("extra-target-template")
            .help("Adds an extra target alongside the usual target with TextPositionSelector. This extra target can be used for provide a direct IRI/URI to fetch the exact textselection (if the backend system supports it). In the template, you should specify an IRI with the variables {resource} (which is the resource IRI), {begin}, and {end}, they will be substituted accordingly. A common value is: {resource}/{begin}/{end}")
            .takes_value(true)
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
        .with_use_include(
            args.get_one("no-include")
                .map(|x: &bool| !x)
                .unwrap_or(true),
        )
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
            .help("Column Format, comma separated list of column names to output (or input depending on context)")
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
                "Do text validation on the TSV, values: strict, loose (case insensitive testing, this is the default), no"
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
        Arg::with_name("trim")
            .long("trim")
            .help("Trim leading and trailing whitespace (including newlines) from matches, ensuring the aligned matches are minimal. This may lead to whitespace being unaligned even though there is an alignment."),
    );
    args.push(
        Arg::with_name("algorithm")
            .long("algorithm")
            .takes_value(true)
            .default_value("smith_waterman")
            .help("Alignment algorithm, can be 'smith_waterman' (aka 'local', default) or 'needleman_wunsch' (aka 'global'). The former is intended for local alignment (align a subsequence with larger sequence), the latter for global alignment (aligning two complete sequences)"),
    );
    args.push(
        Arg::with_name("id-prefix")
            .long("id-prefix")
            .takes_value(true)
            .help("Prefix to use globally in assigning all annotation IDs. New IDs for transpositions and resegmentations will also have a random component."),
    );
    args.push(
        Arg::with_name("minimal-align-length")
            .long("minimal-align-length")
            .long("min-length")
            .takes_value(true)
            .help("The minimal number of characters that must be aligned (absolute number) for a transposition to be valid"),
    );
    args.push(
        Arg::with_name("max-errors")
            .long("max-errors")
            .takes_value(true)
            .help("The maximum number of errors that may occur (absolute number) for a transposition to be valid, each insertion/deletion counts as 1. This is more efficient than `minimal_align_length` In other words; this represents the number of characters in the search string that may be missed when matching in the larger text. The transposition itself will only consist of fully matching parts, use `grow` if you want to include non-matching parts."),
    );
    args.push(
        Arg::with_name("grow")
            .long("grow")
            .short('g')
            .help("Grow aligned parts into larger alignments by incorporating non-matching parts. This will return translations rather than transpositions. You'll want to set `max_errors` in combination with this one to prevent very low-quality alignments.")
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
            .help("A query in STAMQL to retrieve the transposition pivot annotation, or just the exact transposition ID. See
                https://github.com/annotation/stam/tree/master/extensions/stam-query for an
                explanation of the query language's syntax. The query should produce only one result (if
                not only the first is taken). Use may use one --transposition parameter for each --query parameter (in the same order).
                If this parameter is not specified at all, the first transposition that can be found in your model will be used by default.")
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
            .help("Prefix to use globally in assigning all annotation IDs. New IDs for translations and resegmentations will also have a random component."),
    );
    args.push(
        Arg::with_name("id-strategy")
            .long("id-strategy")
            .takes_value(true)
            .default_value("updateversion")
            .help("Defines how the IDs for the new output annotations are derived from the IDs from the input annotations. Valid strategies are (foo is a value to replace with a custom string): 
    * addsuffix=foo      to add a suffix to the old ID to form the new ID
    * addprefix=foo      to add a prefix to the old ID to form the new ID
    * updateversion      adds or increments a version suffix (v1,v2,v3,etc)  (this is the default)
    * randomsuffix       adds a random suffix (nanoid)
    * replace=foo        replaces the entire ID with the new value (of limited use since you can only do this once)
    * replacerandom=foo;foo       to set a prefix (first foo), random component
                                  (nanoid) and suffix (second foo)
"),
    );
    args.push(
        Arg::with_name("no-transpositions")
            .long("no-transpositions")
            .help("Do not produce transposition annotations. Only the transposed annotations will be produced. This essentially throws away provenance information."),
    );
    args.push(
        Arg::with_name("no-resegmentations")
            .long("no-resegmentations")
            .help("Do not produce resegmentation annotations. Only the resegmented annotations will be produced if needed. This essentially throws away provenance information."),
    );
    args.push(
        Arg::with_name("ignore-errors")
            .long("ignore-errors")
            .help("Skip annotations that can not be transposed successfully and output a warning, this would produce a hard failure otherwise"),
    );
    args.push(
        Arg::with_name("debug-transpose")
            .long("debug-transpose")
            .help("Debug the transpose function only (more narrow than doing --debug in general)"),
    );
    args
}

fn translate_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("translation")
            .long("translation")
            .short('T')
            .help("A query in STAMQL to retrieve the translation pivot annotation, or just the exact translation ID. See
                https://github.com/annotation/stam/tree/master/extensions/stam-query for an
                explanation of the query language's syntax. The query should produce only one result (if
                not only the first is taken). Use may use one --translation parameter for each --query parameter (in the same order).
                If this parameter is not specified at all, the first translation that can be found in your model will be used by default.
                ")
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("use-translation")
            .long("use-translation")
            .help(
                "Name of the variable from the  --translation queries to use (must be the same for all). If not set, the last defined subquery will be used"
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
        Arg::with_name("id-strategy")
            .long("id-strategy")
            .takes_value(true)
            .default_value("updateversion")
            .help("Defines how the IDs for the new output annotations are derived from the IDs from the input annotations. Valid strategies are (foo is a value to replace with a custom string): 
    * addsuffix=foo      to add a suffix to the old ID to form the new ID
    * addprefix=foo      to add a prefix to the old ID to form the new ID
    * updateversion      adds or increments a version suffix (v1,v2,v3,etc)  (this is the default)
    * randomsuffix       adds a random suffix (nanoid)
    * replace=foo        replaces the entire ID with the new value (of limited use since you can only do this once)
    * replacerandom=foo;foo       to set a prefix (first foo), random component
                                  (nanoid) and suffix (second foo)
"));
    args.push(
        Arg::with_name("no-translations")
            .long("no-translations")
            .help("Do not produce translation annotations. Only the translated annotations will be produced. This essentially throws away provenance information."),
    );
    args.push(
        Arg::with_name("no-resegmentations")
            .long("no-resegmentations")
            .help("Do not produce resegmentation annotations. Only the resegmented annotations will be produced if needed. This essentially throws away provenance information."),
    );
    args.push(
        Arg::with_name("ignore-errors")
            .long("ignore-errors")
            .help("Skip annotations that can not be translated successfully and output a warning, this would produce a hard failure otherwise"),
    );
    args.push(
        Arg::with_name("debug-translate")
            .long("debug-translate")
            .help("Debug the translate function only (more narrow than doing --debug in general)"),
    );
    args
}

fn translatetext_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("rules")
            .long("rules")
            .short('R')
            .help("Filename of the configuration file (toml) that holds the translation rules")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("query")
            .long("query")
            .short('q')
            .help("A query in STAMQL to select text resources or text selections to translate. See
                https://github.com/annotation/stam/tree/master/extensions/stam-query for an
                explanation of the query language's syntax. The query may produce multiple results. If no resource arguments are specified at all, then all text resources will be taken for text translation.")
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("use")
            .long("use")
            .help("Name of the variable from --query to use for the selection of resources")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("id-suffix")
            .long("id-suffix")
            .takes_value(true)
            .help("The ID suffix to use when minting new IDs for resources and annotations (overrides the one in the --rules configuration, if any)"),
    );
    args.push(
        Arg::with_name("no-translations")
            .long("no-translations")
            .help("Do not produce translation annotations. Only produce the translated texts. This essentially throws away all provenance information and prevents being able to translate annotations between texts later on."),
    );
    args.push(
        Arg::with_name("force")
            .long("force")
            .help("Force output of texts and translations even if there is no change whatsoever"),
    );
    args.push(
        Arg::with_name("debug-translate")
            .long("debug-translate")
            .help(
                "Debug the translatetext function only (more narrow than doing --debug in general)",
            ),
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
        Arg::with_name("query-file")
            .long("query-file")
            .help("Read a query from file, use - for stdin.")
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

fn xml_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("inputfile")
            .long("inputfile")
            .short('f')
            .help("XML file to import. This option may be specified multiple times. Each input file will produce one output text.")
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("inputfilelist")
            .long("inputfilelist")
            .short('l')
            .help("Filename containing a list of input files (one per line, alternative to specifying --inputfile multiple times). You may also specify *multiple* files per line, seperated by tab characteers, these will then jointly produce a single text, named after the first file on the line.")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("config")
            .long("config")
            .short('c')
            .help("Configuration file that defines how to map a specific XML format to STAM")
            .required(true)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("provenance")
            .long("provenance")
            .help("Add provenance information by pointing back to the XML source files using W3C Web Annotation's XPathSelector"),
    );
    args.push(
        Arg::with_name("id-prefix")
            .long("id-prefix")
            .takes_value(true)
            .help("Prefix to use when assigning annotation IDs. If you use the special variable {resource}, it will be resolved to the resource ID, which is useful when annotation IDs in the XML are not globally unique."),
    );
    args.push(
        Arg::with_name("ignore-errors")
            .long("ignore-errors")
            .help("Skip XML files that have errors and output a warning, this would produce a hard failure otherwise"),
    );
    args.push(
        Arg::with_name("debug-xml")
            .long("debug-xml")
            .help("Debug the xml mapping only (more narrow than doing --debug in general)"),
    );
    args
}

fn validation_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("make")
            .long("make")
            .help("Compute text validation information, allowing the model to be validated later."),
    );
    args.push(
        Arg::with_name("allow-incomplete")
            .long("allow-incomplete")
            .help("Allow validation to pass even if validation information is missing for certain annotations (or for all)")
    );
    args
}

fn split_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("keep")
            .long("keep")
            .help("Queries will be interpreted as items to retain, others will be deleted"),
    );
    args.push(
        Arg::with_name("remove")
            .long("remove")
            .help("Queries will be interpreted as items to delete, others will be retained"),
    );
    args.push(
        Arg::with_name("resource")
            .long("resource")
            .help("Specify the ID of a resource to either --keep or --remove (depending on that parameter). This is a shortcut to use instead of specifying a full --query. May be used multiple times.")
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args.push(
        Arg::with_name("dataset")
            .long("dataset")
            .help("Specify the ID of a dataset to either --keep or --remove (depending on that parameter). This is a shortcut to use instead of specifying a full --query. May be used multiple times.")
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args
}

fn store_exists(args: &ArgMatches) -> bool {
    if args.is_present("annotationstore") {
        for filename in args.values_of("annotationstore").unwrap() {
            return Path::new(filename).exists();
        }
        false
    } else {
        false
    }
}

fn load_store(args: &ArgMatches) -> AnnotationStore {
    let mut store: AnnotationStore = AnnotationStore::new(config_from_args(args));
    for (i, filename) in args
        .values_of("annotationstore")
        .expect("an annotation store must be provided")
        .into_iter()
        .enumerate()
    {
        if filename.to_lowercase().ends_with(".txt") || filename.to_lowercase().ends_with(".md") {
            //we got a plain text file, add it to the store
            store
                .add_resource(TextResourceBuilder::new().with_filename(filename))
                .unwrap_or_else(|err| {
                    eprintln!("error loading text: {}", err);
                    exit(1);
                });
        } else if i == 0 {
            //first file
            store = AnnotationStore::from_file(filename, config_from_args(args)).unwrap_or_else(
                |err| {
                    eprintln!("error loading annotation store: {}", err);
                    exit(1);
                },
            );
        } else if !filename.ends_with(".json") {
            eprintln!("When loading multiple annotation store, the other ones must be in STAM JSON format (CSV and CBOR not supported)");
            exit(1);
        } else {
            store = store.with_file(filename).unwrap_or_else(|err| {
                eprintln!("Error loading annotation store: {}", err);
                exit(1);
            });
        }
    }
    if args.is_present("strip-ids") {
        store.strip_data_ids();
        store.strip_annotation_ids();
    }
    store
}

fn app<'a>(batchmode: bool) -> App<'a> {
    App::new("STAM Tools")
        .version(VERSION)
        .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>, KNAW Humanities Cluster")
        .about("CLI tool to work with standoff text annotation (STAM)")
        .subcommand(
            SubCommand::with_name("batch")
                .visible_alias("shell")
                .about("Batch mode, reads multiple STAM subcommands from standard input (or interactively). Loading takes place at the very beginning, and saving is deferred to the very end.")
                .args(&common_arguments())
                .args(&store_arguments(true, true, batchmode))
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("info")
                .about("Return information regarding a STAM model. Set --verbose for extra details.")
                .args(&common_arguments())
                .args(&store_arguments(true, false, batchmode))
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("validate")
                .about("Validate a STAM model. Checks if the integrity of the annotations is still valid by checking if the text they point at remains unchanged.")
                .args(&common_arguments())
                .args(&store_arguments(true, false, batchmode))
                .args(&config_arguments())
                .args(&validation_arguments()),
        )
        .subcommand(
            SubCommand::with_name("export")
                .about("Export annotations (or other data structures) as tabular data to a TSV format. If --verbose is set, a tree-like structure is expressed in which the order of rows matters.")
                .args(&common_arguments())
                .args(&store_arguments(true, true, batchmode))
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
                .args(&store_arguments(false, true, batchmode))
                .args(&config_arguments())
                .args(&tsv_arguments_in()),
        )
        .subcommand(
            SubCommand::with_name("fromxml")
                .about("Convert an XML file with inline annotations to STAM. A mapping for the conversion is provided separately in a configuration file and is specific to a type of XML format.")
                .args(&common_arguments())
                .args(&store_arguments(false, true, batchmode))
                .args(&config_arguments())
                .args(&xml_arguments()),
        )
        .subcommand(
            SubCommand::with_name("query")
                .about("Query annotations by data and output results to a TSV format. If --verbose is set, a tree-like structure is expressed in which the order of rows matters.")
                .args(&common_arguments())
                .args(&store_arguments(true, false, batchmode))
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
                .args(&common_arguments())
                .args(&config_arguments())
                .args(&store_arguments(true, false, batchmode))
                .about("Extract an offset from a plain text file (no STAM model needed).")
                .arg(
                    Arg::with_name("resource")
                        .long("resource")
                        .short('r')
                        .help(
                            "The resource ID (may be equal to base filename). If omitted, the first resource in the model will be grabbed if an offset was specified, otherwise ALL resources will be printed."
                        )
                        .takes_value(true)
                        .required(false)
                )
                .arg(
                    Arg::with_name("begin")
                        .long("begin")
                        .alias("start")
                        .short('b')
                        .help(
                            "Begin offset (0-indexed)"
                        )
                        .takes_value(true)
                        .default_value("0")
                )
                .arg(
                    Arg::with_name("end")
                        .long("end")
                        .short('e')
                        .help(
                            "End offset (0-indexed, non-inclusive)"
                        )
                        .takes_value(true)
                        .default_value("0")
                )
                .arg(
                    Arg::with_name("offset")
                        .long("offset")
                        .alias("O")
                        .help(
                            "Offset specification in begin:end or begin,end format. Where either is a (possibly signed) integer. Can be used instead of begin and end."
                        )
                        .takes_value(true)
                )
        )
        .subcommand(
            SubCommand::with_name("view")
                .about("Output the text and annotations of one or more resource(s) in HTML, suitable for visualisation in a browser. Requires --query or a simpler shortcut like --resource")
                .args(&common_arguments())
                .args(&store_arguments(true, false, batchmode))
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL that defines what to visualise. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. The query can have subqueries which will be marked as highlights. Alternative, you can specify no subqueries and specify multiple --query parameters on the command line. They will be automatically converted to subqueries of the first query. The first/main query is the primary selection and determines what text is shown. Highlight queries (subqueries) determine what parts inside this text are highlighted.

You can prepend the following *attributes* to `DATA` constraints in the query (in a `WHERE` clause before `DATA`), to determine how things are visualised:

* @KEYTAG - Outputs a tag with the key
* @KEYVALUETAG - Outputs a tag with the key and the value
* @VALUETAG - Outputs a tag with the value only.

You can prepend the following *attributes* to a query/subquery as a whole (before the SELECT statement), to determine how things are visualised:

* @HIDE - Do not show this query in the results (no underline)
* @STYLE=class - Associated a CSS class (replace class with any CSS class name) with these results. For HTML visualisation only.
* @IDTAG - Outputs a tag with the public identifier of the ANNOTATION that has been selected

You can also put `@KEYTAG`, `@KEYVALUETAG` and `@VALUETAG` before the whole `SELECT` query, in that case it will automatically apply to the first `DATA` constraint.

If no attributes are provided, there will be no tags shown for that query, only a highlight underline. In the highlight queries, the variable from the main selection query is available and you *should* use it in a constraint, otherwise performance will be sub-optimal.
" ))
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
                    Arg::with_name("collapse")
                        .long("collapse")
                        .help(
                        "Collapse all tags by default on loading HTML documents",
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
                .args(&store_arguments(true,true, batchmode))
                .args(&annotate_arguments())
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL to ADD or DELETE items. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. ")),
        )
        .subcommand(
            SubCommand::with_name("annotate")
                .visible_alias("add")
                .about("Add annotations (or datasets, resources) to an existing annotationstore")
                .args(&store_arguments(true,true, batchmode))
                .args(&annotate_arguments())
                .args(&common_arguments())
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL to ADD or DELETE items. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. ")),
        )
        .subcommand(
            SubCommand::with_name("tag")
                .about("Regular-expression based tagger on plain text")
                .args(&common_arguments())
                .args(&store_arguments(true,true, batchmode))
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
                .args(&store_arguments(true,false, batchmode))
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
                .args(&store_arguments(true,true, batchmode))
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL to retrieve a text. See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. 
You need to specify this parameter twice, the text of first query will be aligned with text of the second one. If specified more than twice, each text will be aligned (independently) with the first one"))
                .args(&align_arguments())
            )
        .subcommand(
            SubCommand::with_name("transpose")
                .about("Transpose annotations over a transposition pivot (annotation), effectively mapping them from one coordinate system to another (See https://github.com/annotation/stam/tree/master/extensions/stam-transpose). Queries correspond to the input annotations to transpose via the transposition pivot (--transposition). The new transposed annotations (and the transpositions that produced them) will be added to the store.")
                .args(&common_arguments())
                .args(&store_arguments(true,true, batchmode))
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL to retrieve annotation(s). See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax.
The first query should retrieve the transposition annotation to transpose over, it should produce only one result. Subsequent queries are the annotations to transpose."))
                .args(&transpose_arguments())
            )
        .subcommand(
            SubCommand::with_name("translate")
                .about("Translate annotations over a translation pivot (annotation), effectively mapping them from one coordinate system to another (See https://github.com/annotation/stam/tree/master/extensions/stam-translate). Queries correspond to the input annotations to translate via that translation pivot (--translation). The new translated annotations (and the translations that produced them) will be added to the store.")
                .args(&common_arguments())
                .args(&store_arguments(true,true, batchmode))
                .args(&config_arguments())
                .args(&query_arguments("A query in STAMQL to retrieve annotation(s). See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax.
The first query should retrieve the translation annotation to translate over, it should produce only one result. Subsequent queries are the annotations to translate."))
                .args(&translate_arguments())
            )
        .subcommand(
            SubCommand::with_name("translatetext")
                .about("Translates one text to another by following translation rules from a configuration file. This will produce Translation annotations that relate the two texts and enables translation of further/future annotations.")
                .visible_alias("tr")
                .args(&common_arguments())
                .args(&store_arguments(true,true, batchmode))
                .args(&config_arguments())
                .args(&translatetext_arguments())
            )
        .subcommand(
            SubCommand::with_name("split")
                .about("Load an annotation model and split one part off into a new one")
                .args(&common_arguments())
                .args(&store_arguments(true,true, batchmode))
                .args(&config_arguments())
                .args(&split_arguments())
                .args(&query_arguments("A query in STAMQL with the items to --keep or --remove. Use ?split as variable name if you use subqueries (otherwise the last/deepest variable is taken). See https://github.com/annotation/stam/tree/master/extensions/stam-query for an explanation of the query language's syntax. Multiple queries are allowed."))
        )
}

fn main() {
    let app = app(false);
    let rootargs = app.get_matches();

    let mut args: Option<&ArgMatches> = None;
    for subcommand in SUBCOMMANDS.iter() {
        if let Some(matchedargs) = rootargs.subcommand_matches(subcommand) {
            args = Some(matchedargs);
        }
    }
    if args.is_none() {
        eprintln!("[error] No command specified, please see 'stam help', for interactive mode use 'stam batch'");
        exit(2);
    }
    let args = args.unwrap();

    let mut store = if rootargs.subcommand_matches("import").is_some()
        || rootargs.subcommand_matches("init").is_some()
        || rootargs.subcommand_matches("fromxml").is_some()
    {
        let force_new = !args.is_present("outputstore")
            && (args.is_present("force-new") || rootargs.subcommand_matches("init").is_some());
        if !force_new && store_exists(args) {
            eprintln!("Existing annotation store found, loading");
            load_store(args)
        } else {
            eprintln!("New annotation store created");
            let mut store = AnnotationStore::new(config_from_args(args));
            if args.is_present("annotationstore") {
                if let Some(filename) = args.values_of("annotationstore").unwrap().next() {
                    store.set_filename(filename);
                }
            }
            store
        }
    } else {
        //loading one or more existing stores
        load_store(args)
    };
    if let Ok(Some(output)) = args.try_get_one::<String>("outputstore") {
        //set output filename
        store.set_filename(output);
    }
    if let Ok(Some(id)) = args.try_get_one::<String>("id") {
        store = store.with_id(id.to_string());
    }

    let changed: bool;

    match run(&mut store, &mut std::io::stdout(), &rootargs, false) {
        Ok(newchanged) => {
            changed = newchanged;
        }
        Err(err) => {
            eprintln!("[error] {}", &err);
            exit(1);
        }
    }

    if changed && !args.is_present("dry-run") {
        if let Some(filename) = store.filename() {
            eprintln!("Writing annotation store to {}", filename);
        }
        store.save().unwrap_or_else(|err| {
            eprintln!(
                "[error] Failed to write annotation store {:?}: {}",
                store.filename(),
                err
            );
            exit(1);
        });
    }
}

#[derive(Clone, Copy, PartialEq)]
enum BatchOutput<'a> {
    Stdout,
    WriteToFile(&'a str),
    AppendToFile(&'a str),
}

fn parse_batch_line<'a>(line: &'a str) -> (Vec<Cow<'a, str>>, BatchOutput<'a>) {
    let mut fields: Vec<Cow<str>> = Vec::new();
    fields.push(Cow::Borrowed("stam")); //binary name
    let mut quote = false;
    let mut escaped = false;
    let mut begin = 0;
    let mut output = BatchOutput::Stdout;
    for (i, c) in line.char_indices() {
        if c == '"' && !escaped {
            quote = !quote;
            if quote {
                begin = i + 1;
            } else {
                if line[begin..i].find("\\\"").is_some() {
                    fields.push(Cow::Owned(line[begin..i].replace("\\\"", "\"")));
                //unescape embedded contents
                } else {
                    fields.push(Cow::Borrowed(&line[begin..i]));
                }
                begin = i + 1;
            }
        } else if !quote {
            if c == ' ' || c == '\n' || c == '\t' {
                let field = &line[begin..i].trim();
                if !field.is_empty() {
                    fields.push(Cow::Borrowed(&line[begin..i]));
                }
                begin = i + 1;
            } else if c == '>' {
                if line[i..].starts_with(">>") {
                    output = BatchOutput::AppendToFile(&line[i + 2..].trim());
                } else {
                    output = BatchOutput::WriteToFile(&line[i + 1..].trim());
                }
                break;
            }
        }
        escaped = c == '\\';
    }
    (fields, output)
}

fn run<W: Write>(
    store: &mut AnnotationStore,
    writer: &mut W,
    rootargs: &ArgMatches,
    batchmode: bool,
) -> Result<bool, String> {
    let mut args: Option<&ArgMatches> = None;
    let mut changed = false;
    for subcommand in SUBCOMMANDS.iter() {
        if let Some(matchedargs) = rootargs.subcommand_matches(subcommand) {
            args = Some(matchedargs);
        }
    }
    if args.is_none() {
        return Err(format!("No command specified, please see stam --help"));
    }

    let args = args.unwrap();
    if rootargs.subcommand_matches("batch").is_some() {
        if batchmode {
            return Err(format!("Batch can't be used when already in batch mode"));
        }
        eprintln!("Batch mode enabled, enter stam commands as usual but without the initial 'stam' command\n but without store input/output arguments\nintermediate output may be redirected to file rather than stdout with the > and >> operators\ntype 'help' for help\ntype 'quit' or ^D to quit with saving (if applicable), 'cancel' or ^C to quit without saving");
        let mut line = String::new();
        let is_tty = atty::is(atty::Stream::Stdin);
        let mut seqnr = 0;
        loop {
            seqnr += 1;
            //prompt
            eprint!("stam[{}{}]> ", seqnr, if changed { "*" } else { "" });
            match io::stdin().lock().read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let line_trimmed = line.trim();
                    if !is_tty {
                        //non-interactive: copy input
                        eprintln!("{}", line);
                    }
                    if line_trimmed == "q" || line_trimmed == "quit" || line_trimmed == "exit" {
                        break;
                    } else if line_trimmed == "abort" || line_trimmed == "cancel" {
                        changed = false;
                        break;
                    }
                    let (fields, output) = parse_batch_line(&line);
                    let batchapp = app(true);
                    match batchapp.try_get_matches_from(fields.iter().map(|s| s.as_ref())) {
                        Ok(batchargs) => {
                            let result = match output {
                                BatchOutput::Stdout => {
                                    run(store, &mut io::stdout(), &batchargs, true)
                                }
                                BatchOutput::WriteToFile(filename) => {
                                    let mut f = fs::File::create(filename)
                                        .map_err(|err| format!("{}", err))?;
                                    run(store, &mut f, &batchargs, true)
                                }
                                BatchOutput::AppendToFile(filename) => {
                                    let mut f = fs::File::options()
                                        .append(true)
                                        .open(filename)
                                        .map_err(|err| format!("{}", err))?;
                                    run(store, &mut f, &batchargs, true)
                                }
                            };
                            match result {
                                Ok(newchanged) => {
                                    changed = changed || newchanged;
                                }
                                Err(err) => {
                                    eprintln!("[error] {}", &err);
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("[syntax error] {}", &err);
                        }
                    }
                }
                Err(err) => {
                    if !is_tty {
                        //non-interactive: copy input
                        eprintln!("{}", line);
                    }
                    eprintln!("{}", err);
                }
            }
            line.clear();
        }
    } else if rootargs.subcommand_matches("info").is_some() {
        info(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("export").is_some()
        || rootargs.subcommand_matches("query").is_some()
    {
        let verbose = args.is_present("verbose");

        let mut querystring_buffer = String::new();
        let querystring = if let Some(filename) = args.value_of("query-file") {
            if filename == "-" {
                io::stdin()
                    .lock()
                    .read_to_string(&mut querystring_buffer)
                    .map_err(|err| format!("Unable to read query from standard input: {}", err))?;
            } else {
                querystring_buffer = fs::read_to_string(filename).map_err(|err| {
                    format!("Unable to read query from file {}: {}", filename, err)
                })?;
            }
            querystring_buffer.as_str()
        } else {
            args.value_of("query").into_iter().next().unwrap_or(
            if args.is_present("alignments") {
                    "SELECT ANNOTATION ?annotation WHERE [ DATA \"https://w3id.org/stam/extensions/stam-transpose/\" \"Transposition\" OR DATA \"https://w3id.org/stam/extensions/stam-translate/\" \"Translation\" ];"
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
                        return Err(format!("Invalid --type specified"));
                    }
                }
            },
        )
        };
        let (query, _) = stam::Query::parse(querystring).map_err(|err| format!("{}", err))?;

        let resulttype = query.resulttype().expect("Query has no result type");

        if args.is_present("alignments") {
            alignments_tsv_out(&store, query, args.value_of("use"))
                .map_err(|err| format!("{}", err))?;
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
                        return Err(format!("Invalid --type specified"));
                    }
                }
            };

            to_tsv(
                &store,
                writer,
                query,
                &columns,
                args.is_present("verbose"),
                args.value_of("subdelimiter").unwrap(),
                args.value_of("null").unwrap(),
                !args.is_present("no-header"),
                args.value_of("setdelimiter").unwrap(),
                !args.is_present("strict-columns"),
            )
            .map_err(|err| format!("{}", err))?;
        } else if let Some("json") = args.value_of("format") {
            to_json(&store, writer, query).map_err(|err| format!("{}", err))?;
        } else if let Some("txt") = args.value_of("format") {
            to_text(&store, writer, query, args.value_of("use"))
                .map_err(|err| format!("{}", err))?;
        } else if let Some("webanno") | Some("w3anno") | Some("jsonl") = args.value_of("format") {
            let mut w3annoconfig = WebAnnoConfig {
                default_annotation_iri: args.value_of("annotation-prefix").unwrap().to_string(),
                default_set_iri: args.value_of("dataset-prefix").unwrap().to_string(),
                default_resource_iri: args.value_of("resource-prefix").unwrap().to_string(),
                auto_generated: !args.is_present("no-generated"),
                auto_generator: !args.is_present("no-generator"),
                extra_context: args
                    .values_of("add-context")
                    .unwrap_or(clap::Values::default())
                    .map(|x| x.to_string())
                    .collect(),
                extra_target_template: args
                    .get_one("extra-target-template")
                    .map(|s: &String| s.to_string()),
                context_namespaces: {
                    let mut namespaces = Vec::new();
                    for assignment in args
                        .values_of("namespaces")
                        .unwrap_or(clap::Values::default())
                    {
                        let result: Vec<_> = assignment.splitn(2, ":").collect();
                        if result.len() != 2 {
                            return Err(format!("Syntax for --ns should be `ns: uri_prefix`"));
                        }
                        namespaces
                            .push((result[1].trim().to_string(), result[0].trim().to_string()));
                    }
                    namespaces
                },
                ..WebAnnoConfig::default()
            };
            if !args.is_present("no-auto-context") {
                w3annoconfig = w3annoconfig.auto_extra_context(store);
            }
            to_w3anno(
                &store,
                writer,
                query,
                args.value_of("use").unwrap_or(match resulttype {
                    stam::Type::Annotation => "annotation",
                    stam::Type::TextSelection => "text",
                    _ => {
                        return Err(format!("Web Annotation output only supports queries with result type ANNOTATION or TEXT"));
                    }
                }),
                w3annoconfig
            );
        } else {
            return Err(format!(
                "Invalid output format, specify 'tsv', 'json', 'txt' or 'w3anno'"
            ));
        }
    } else if rootargs.subcommand_matches("import").is_some() {
        let inputfiles = args.values_of("inputfile").unwrap().collect::<Vec<&str>>();
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
                store,
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
                ValidationMode::try_from(args.value_of("validate").unwrap())
                    .map_err(|err| format!("{}", err))?,
                args.is_present("verbose"),
            )?;
        }
        changed = true;
    } else if rootargs.subcommand_matches("view").is_some() {
        let queries: Vec<&str> = args.values_of("query").unwrap_or_default().collect();
        let mut queries_iter = queries.into_iter();
        let mut querystring_buffer = String::new();
        if let Some(queryfile) = args.value_of("query-file") {
            if queryfile == "-" {
                io::stdin()
                    .lock()
                    .read_to_string(&mut querystring_buffer)
                    .map_err(|err| format!("Unable to read query from standard input: {}", err))?;
            } else {
                querystring_buffer = fs::read_to_string(queryfile).map_err(|err| {
                    format!("Unable to read query from file {}: {}", queryfile, err)
                })?;
            }
        };
        let mut querystring = queries_iter.next().unwrap_or("SELECT RESOURCE ?res;");
        if !querystring_buffer.is_empty() {
            querystring = querystring_buffer.as_str();
        } else if querystring.trim().ends_with("}") {
            if queries_iter.next().is_some() {
                return Err(format!("You can't supply multiple --query parameters on the command line if the first query already contains subqueries (use either one or the other)"));
            }
        } else {
            for (i, subquerystring) in queries_iter.enumerate() {
                if i == 0 {
                    querystring_buffer += " { ";
                } else {
                    querystring_buffer += " | ";
                }
                querystring_buffer += subquerystring;
            }
            if !querystring_buffer.is_empty() {
                querystring_buffer.insert_str(0, querystring);
                querystring_buffer += " }";
                querystring = querystring_buffer.as_str();
            }
        }

        let (query, _) = stam::Query::parse(querystring)
            .map_err(|err| format!("Query syntax error in first query: {}", err))?;

        match args.value_of("format") {
            Some("html") => {
                let htmlwriter = HtmlWriter::new(&store, query, args.value_of("use"))?
                    .with_autocollapse(args.is_present("collapse"))
                    .with_legend(!args.is_present("no-legend"))
                    .with_titles(!args.is_present("no-titles"))
                    .with_annotation_ids(args.is_present("verbose"));
                write!(writer, "{}", htmlwriter)
                    .map_err(|e| format!("Failed to write HTML output: {}", e))?;
            }
            Some("ansi") => {
                let mut ansiwriter = AnsiWriter::new(&store, query, args.value_of("use"))?;
                if args.is_present("no-legend") {
                    ansiwriter = ansiwriter.with_legend(false)
                }
                if args.is_present("no-titles") {
                    ansiwriter = ansiwriter.with_titles(false)
                }
                ansiwriter
                    .write(writer)
                    .map_err(|e| format!("Failed to write ANSI output: {}", e))?;
            }
            Some(s) => {
                eprintln!("[error] Unknown output format: {}", s);
            }
            None => unreachable!(),
        }
    } else if rootargs.subcommand_matches("validate").is_some() {
        if args.is_present("make") {
            store
                .protect_text(stam::TextValidationMode::Auto)
                .map_err(|e| format!("Failed to generate validation information: {}", e))?;
            changed = true;
        } else {
            validate(
                &store,
                args.is_present("verbose"),
                args.is_present("allow-incomplete"),
            )?;
        }
    } else if rootargs.subcommand_matches("init").is_some()
        || rootargs.subcommand_matches("annotate").is_some()
    {
        let resourcefiles = args
            .values_of("resources")
            .unwrap_or_default()
            .collect::<Vec<&str>>();
        let setfiles = args
            .values_of("annotationsets")
            .unwrap_or_default()
            .collect::<Vec<&str>>();
        let annotationfiles = args
            .values_of("annotations")
            .unwrap_or_default()
            .collect::<Vec<&str>>();
        eprintln!(
            "{} store: {} new annotation file(s), {} new resource(s), {} new annotationset(s)",
            if rootargs.subcommand_matches("annotate").is_some() {
                "Adding to"
            } else {
                "Initializing"
            },
            annotationfiles.len(),
            resourcefiles.len(),
            setfiles.len(),
        );
        annotate(store, &resourcefiles, &setfiles, &annotationfiles)?;
        changed = true;

        if args.is_present("query") {
            let querystring = args.value_of("query").into_iter().next().unwrap();
            let (query, _) = stam::Query::parse(querystring).map_err(|err| format!("{}", err))?;
            store.query_mut(query).map_err(|err| format!("{}", err))?;
        }

        eprintln!(
            "  total: {} annotation(s), {} resource(s), {} annotationset(s)",
            store.annotations_len(),
            store.resources_len(),
            store.datasets_len()
        );
    } else if rootargs.subcommand_matches("tag").is_some() {
        //load the store
        tag(
            store,
            args.value_of("rules").expect("--rules must be provided"),
            args.is_present("allow-overlap"),
        )?;
        changed = true;
    } else if rootargs.subcommand_matches("grep").is_some() {
        //load the store
        grep(
            &store,
            args.values_of("expression")
                .expect("--expression must be provided")
                .collect(),
            args.is_present("allow-overlap"),
        )?;
    } else if rootargs.subcommand_matches("align").is_some() {
        //load the store
        let mut querystrings: Vec<_> = args
            .values_of("query")
            .unwrap_or_default()
            .map(|x| x.to_string())
            .collect();
        querystrings.extend(
            args.values_of("resource")
                .unwrap_or_default()
                .map(|x| format!("SELECT RESOURCE WHERE ID \"{}\"", x)),
        );

        let mut queries = VecDeque::new();
        for (i, querystring) in querystrings.iter().enumerate() {
            queries.push_back(
                stam::Query::parse(querystring.as_str())
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }
        if queries.len() < 2 {
            return Err(format!(
                "Expected at least two --query (or --resource) parameters"
            ));
        }

        if let Err(err) = align(
            store,
            queries.pop_front().unwrap(),
            queries.into_iter().collect(),
            args.value_of("use"),
            args.value_of("use2"),
            &AlignmentConfig {
                case_sensitive: !args.is_present("ignore-case"),
                algorithm: match args.value_of("algorithm") {
                    Some("smith_waterman") | Some("local") => AlignmentAlgorithm::SmithWaterman {
                        equal: args
                            .value_of("match-score")
                            .unwrap()
                            .parse()
                            .expect("score must be integer"),
                        align: args
                            .value_of("mismatch-score")
                            .unwrap()
                            .parse()
                            .expect("score must be integer"),
                        insert: args
                            .value_of("insertion-score")
                            .unwrap()
                            .parse()
                            .expect("score must be integer"),
                        delete: args
                            .value_of("deletion-score")
                            .unwrap()
                            .parse()
                            .expect("score must be integer"),
                    },
                    Some("needleman_wunsch") | Some("global") => {
                        AlignmentAlgorithm::NeedlemanWunsch {
                            equal: args
                                .value_of("match-score")
                                .unwrap()
                                .parse()
                                .expect("score must be integer"),
                            align: args
                                .value_of("mismatch-score")
                                .unwrap()
                                .parse()
                                .expect("score must be integer"),
                            insert: args
                                .value_of("insertion-score")
                                .unwrap()
                                .parse()
                                .expect("score must be integer"),
                            delete: args
                                .value_of("deletion-score")
                                .unwrap()
                                .parse()
                                .expect("score must be integer"),
                        }
                    }
                    Some(x) => {
                        return Err(format!("[error] Not a valid alignment algorithm: {}, set smith_waterman or needleman_wunsch", x));
                    }
                    None => unreachable!("No alignment algorithm set"),
                },
                annotation_id_prefix: args.value_of("id-prefix").map(|x| x.to_string()),
                simple_only: args.is_present("simple-only"),
                trim: args.is_present("trim"),
                max_errors: if args.is_present("max-errors") {
                    Some(args.value_of("max-errors").unwrap().parse().expect(
                        "value for --max-errors must be integer (absolute) or float (relative)",
                    ))
                } else {
                    None
                },
                minimal_align_length: if args.is_present("minimal-align-length") {
                    args.value_of("minimal-align-length")
                        .unwrap()
                        .parse()
                        .expect("value for --minimal-align-length must be integer")
                } else {
                    0
                },
                grow: args.is_present("grow"),
                verbose: args.is_present("verbose"),
            },
        ) {
            return Err(format!("[error] Alignment failed: {:?}", err));
        }
        changed = true;
    } else if rootargs.subcommand_matches("transpose").is_some() {
        let transposition_querystrings: Vec<_> = args
            .values_of("transposition")
            .unwrap_or_default()
            .map(|q| {
                if q.find(" ").is_some() {
                    //already a query
                    q.to_string()
                } else {
                    //probably an ID, transform to query
                    format!("SELECT ANNOTATION WHERE ID \"{}\";", q)
                }
            })
            .collect();

        let querystrings: Vec<_> = args.values_of("query").unwrap_or_default().collect();

        let mut transposition_queries = Vec::new();
        for (i, querystring) in transposition_querystrings.iter().enumerate() {
            transposition_queries.push(
                stam::Query::parse(querystring)
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }
        if transposition_queries.len() < 1 {
            //grab the first transposition by default
            let querystring = "SELECT ANNOTATION WHERE DATA \"https://w3id.org/stam/extensions/stam-transpose/\" \"Transposition\";";
            transposition_queries.push(
                stam::Query::parse(querystring)
                    .map_err(|err| format!("Query syntax error query (INTERNAL!): {}", err))?
                    .0,
            );
        }

        let mut queries = Vec::new();
        for (i, querystring) in querystrings.into_iter().enumerate() {
            queries.push(
                stam::Query::parse(querystring)
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }
        if queries.len() < 1 {
            return Err(format!("Expected at least one --query parameter"));
        }
        if let Err(err) = transpose(
            store,
            transposition_queries,
            queries,
            args.value_of("use-transposition"),
            args.value_of("use"),
            args.value_of("id-prefix").map(|x| x.to_string()),
            args.value_of("id-strategy")
                .map(|x| x.try_into().expect("invalid value for id-strategy"))
                .unwrap(),
            args.is_present("ignore-errors"),
            args.is_present("verbose"),
            TransposeConfig {
                existing_source_side: true,
                no_transposition: args.is_present("no-transpositions"),
                no_resegmentation: args.is_present("no-resegmentations"),
                debug: args.is_present("debug") || args.is_present("debug-transpose"),
                ..Default::default()
            },
        ) {
            return Err(format!("Transposition failed: {:?}", err));
        }
        changed = true;
    } else if rootargs.subcommand_matches("translate").is_some() {
        let translation_querystrings: Vec<_> = args
            .values_of("translation")
            .unwrap_or_default()
            .map(|q| {
                if q.find(" ").is_some() {
                    //already a query
                    q.to_string()
                } else {
                    //probably an ID, transform to query
                    format!("SELECT ANNOTATION WHERE ID \"{}\";", q)
                }
            })
            .collect();

        let querystrings: Vec<_> = args.values_of("query").unwrap_or_default().collect();

        let mut translation_queries = Vec::new();
        for (i, querystring) in translation_querystrings.iter().enumerate() {
            translation_queries.push(
                stam::Query::parse(querystring)
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }
        if translation_queries.len() < 1 {
            //grab the first translation by default
            let querystring = "SELECT ANNOTATION WHERE DATA \"https://w3id.org/stam/extensions/stam-translate/\" \"Translation\";";
            translation_queries.push(
                stam::Query::parse(querystring)
                    .map_err(|err| format!("Query syntax error query (INTERNAL!): {}", err))?
                    .0,
            );
        }

        let mut queries = Vec::new();
        for (i, querystring) in querystrings.into_iter().enumerate() {
            queries.push(
                stam::Query::parse(querystring)
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }
        if queries.len() < 1 {
            return Err(format!("Expected at least one --query parameter"));
        }
        if let Err(err) = translate(
            store,
            translation_queries,
            queries,
            args.value_of("use-translation"),
            args.value_of("use"),
            args.value_of("id-prefix").map(|x| x.to_string()),
            args.value_of("id-strategy")
                .map(|x| x.try_into().expect("invalid value for id-strategy"))
                .unwrap(),
            args.is_present("ignore-errors"),
            args.is_present("verbose"),
            TranslateConfig {
                existing_source_side: true,
                no_translation: args.is_present("no-translations"),
                no_resegmentation: args.is_present("no-resegmentations"),
                debug: args.is_present("debug") || args.is_present("debug-translate"),
                ..Default::default()
            },
        ) {
            return Err(format!("translation failed: {:?}", err));
        }
        changed = true;
    } else if rootargs.subcommand_matches("translatetext").is_some() {
        let mut querystrings: Vec<_> = args
            .values_of("query")
            .unwrap_or_default()
            .map(|q| {
                if q.find(" ").is_some() {
                    //already a query
                    q.to_string()
                } else {
                    //probably a resource ID, transform to query
                    format!("SELECT RESOURCE WHERE ID \"{}\";", q)
                }
            })
            .collect();
        if querystrings.is_empty() {
            querystrings.push(format!("SELECT RESOURCE"))
        }
        let mut queries = Vec::new();
        for (i, querystring) in querystrings.iter().enumerate() {
            queries.push(
                stam::Query::parse(querystring.as_str())
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }
        let configdata = if let Some(filename) = args.value_of("rules") {
            fs::read_to_string(filename).map_err(|e| {
                format!(
                    "Failure reading translation rules configuration file {}: {} ",
                    filename, e
                )
            })?
        } else {
            return Err(format!(
                "A configuration file that defines translation rules is required"
            ));
        };
        let mut config = TranslateTextConfig::from_toml_str(
            &configdata,
            args.is_present("debug-translate") || args.is_present("debug"),
        )
        .map_err(|e| {
            format!(
                "Syntax error in translation rules configuration file {}: {}",
                args.value_of("rules").unwrap(),
                e
            )
        })?;
        if let Some(prefix) = args.value_of("id-suffix") {
            config = config.with_id_suffix(prefix);
        }
        if args.is_present("force") {
            config = config.with_force_when_unchanged();
        }
        match translate_text(store, queries, args.value_of("use"), &config) {
            Ok((resources, annotations)) => {
                for resource in resources {
                    if let Err(err) = store.add_resource(resource) {
                        return Err(format!("translation failed (adding resource): {:?}", err));
                    }
                    changed = true;
                }
                for annotation in annotations {
                    if let Err(err) = store.annotate(annotation) {
                        return Err(format!("translation failed (adding resource): {:?}", err));
                    }
                    changed = true;
                }
            }
            Err(err) => return Err(format!("translation failed: {:?}", err)),
        }
    } else if rootargs.subcommand_matches("fromxml").is_some() {
        let configdata = if let Some(filename) = args.value_of("config") {
            fs::read_to_string(filename).map_err(|e| {
                format!("Failure reading XML->STAM config file {}: {} ", filename, e)
            })?
        } else {
            return Err(format!(
                "A configuration file that defines the XML->STAM mapping is required"
            ));
        };
        let mut config = XmlConversionConfig::from_toml_str(&configdata)
            .map_err(|e| {
                format!(
                    "Syntax error in XML->STAM config file {}: {}",
                    args.value_of("config").unwrap(),
                    e
                )
            })?
            .with_debug(args.is_present("debug") || args.is_present("debug-xml"))
            .with_provenance(args.is_present("provenance"));
        if let Some(prefix) = args.value_of("id-prefix") {
            config = config.with_id_prefix(prefix);
        }
        let mut has_input = false;
        if args.is_present("inputfile") {
            for filename in args.values_of("inputfile").unwrap().into_iter() {
                if let Err(e) = from_xml(Path::new(filename), &config, store) {
                    if args.is_present("ignore-errors") {
                        eprintln!(
                            "WARNING: Skipped {} (or part thereof) due to errors: {}",
                            filename, e
                        )
                    } else {
                        return Err(e);
                    }
                }
                has_input = true;
            }
        }
        if let Some(listfilename) = args.value_of("inputfilelist") {
            has_input = true;
            let listdata = fs::read_to_string(listfilename)
                .map_err(|e| format!("Failure reading file list from {}: {}", listfilename, e))?;
            let filenames: Vec<&str> = listdata.split("\n").collect();
            for filename in filenames {
                if !filename.is_empty() {
                    if filename.find('\t').is_some() {
                        let filenames: Vec<&Path> =
                            filename.split('\t').map(|s| Path::new(s)).collect();
                        if let Err(e) = from_multi_xml(&filenames, &config, store) {
                            if args.is_present("ignore-errors") {
                                eprintln!(
                                    "WARNING: Skipped {} (or part thereof) due to errors: {}",
                                    filename, e
                                )
                            } else {
                                return Err(e);
                            }
                        }
                        has_input = true;
                    } else {
                        if let Err(e) = from_xml(Path::new(filename), &config, store) {
                            if args.is_present("ignore-errors") {
                                eprintln!(
                                    "WARNING: Skipped {} (or part thereof) due to errors: {}",
                                    filename, e
                                )
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
        }
        if !has_input {
            return Err(format!("No input files specified"));
        }
        changed = true;
    } else if rootargs.subcommand_matches("split").is_some() {
        let querystrings: Vec<_> = args.values_of("query").unwrap_or_default().collect();
        let mut queries = Vec::new();

        for (i, querystring) in querystrings.into_iter().enumerate() {
            queries.push(
                stam::Query::parse(querystring)
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }

        let resources: Vec<_> = args.values_of("resource").unwrap_or_default().collect();
        let mut extraquerystrings: Vec<String> = Vec::new();
        for resource in resources.iter() {
            extraquerystrings.push(format!("SELECT RESOURCE ?split WHERE ID \"{}\";", resource));
        }
        let datasets: Vec<_> = args.values_of("dataset").unwrap_or_default().collect();
        for dataset in datasets.iter() {
            extraquerystrings.push(format!("SELECT DATASET ?split WHERE ID \"{}\";", dataset));
        }

        for (i, querystring) in extraquerystrings.iter().enumerate() {
            queries.push(
                stam::Query::parse(querystring.as_str())
                    .map_err(|err| format!("Query syntax error query {}: {}", i + 1, err))?
                    .0,
            );
        }

        if queries.len() < 1 {
            return Err(format!("Expected at least one --query parameter"));
        }

        let mode = if args.is_present("remove") {
            SplitMode::Delete
        } else if args.is_present("keep") {
            SplitMode::Retain
        } else {
            return Err(format!("Expected either --keep or --remove, not both"));
        };

        split(store, queries, mode, args.is_present("verbose"));
        changed = true;
    } else if rootargs.subcommand_matches("print").is_some() {
        let offset: Offset = if let Some(offset) = args.value_of("offset") {
            offset.trim().try_into().map_err(|err| format!("{}", err))?
        } else {
            let begin: isize = args
                .value_of("begin")
                .unwrap()
                .parse()
                .expect("begin offset must be an integer");
            let end: isize = args
                .value_of("end")
                .unwrap()
                .parse()
                .expect("end offset must be an integer");
            (begin, end).into()
        };
        let resource: Option<&str> = args.value_of("resource");
        if let Err(e) = print(store, writer, resource, offset) {
            eprintln!("{}", e);
            exit(1);
        } else {
            exit(0);
        }
    }
    Ok(changed)
}
