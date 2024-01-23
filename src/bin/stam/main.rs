use clap::{App, Arg, ArgAction, ArgMatches, SubCommand};
use stam::{AnnotationStore, AssociatedFile, Config};
use std::path::Path;
use std::process::exit;

mod annotate;
mod grep;
mod info;
mod query;
mod tag;
mod to_text;
mod tsv;
mod validate;
mod view;

use crate::annotate::*;
use crate::grep::*;
use crate::info::*;
use crate::query::*;
use crate::tag::*;
use crate::to_text::*;
use crate::tsv::*;
use crate::validate::*;
use crate::view::*;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

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

/// Translate command line arguments to stam library's configuration structure
fn config_from_args(args: &ArgMatches) -> Config {
    Config::default()
        .with_use_include(!args.is_present("no-include"))
        .with_debug(args.is_present("debug"))
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
                .args(&tsv_arguments_out())
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
                .args(&tsv_arguments_out())
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
            //default in case no query was provided
            match args
                .value_of("type")
                .unwrap_or("Annotation")
                .to_lowercase()
                .as_str()
            {
                "annotation" => "SELECT ANNOTATION ?annotation;",
                "key" | "datakey" => "SELECT DATAKEY ?key;",
                "data" | "annotationdata" => "SELECT DATA ?data;",
                "resource" | "textresource" => "SELECT RESOURCE ?resource;",
                "dataset" | "annotationset" => "SELECT DATASET ?dataset;",
                "text" | "textselection" => "SELECT TEXT ?textselection;",
                _ => {
                    eprintln!("Invalid --type specified");
                    exit(1);
                }
            },
        );
        let (query, _) = stam::Query::parse(querystring).unwrap_or_else(|err| {
            eprintln!("Query syntax error: {}", err);
            exit(1);
        });

        let resulttype = query.resulttype().expect("Query has no result type");

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

        let mut writer = HtmlWriter::new(&store, query);
        for (i, highlightquery) in queries_iter.enumerate() {
            let highlight =
                Highlight::parse_query(highlightquery, &store, i + 1).unwrap_or_else(|err| {
                    eprintln!("[error] Syntax error in query {}: {}", i + 1, err);
                    exit(1);
                });
            writer = writer.with_highlight(highlight);
        }

        let setdelimiter = args.value_of("setdelimiter").unwrap();
        if let Some(highlights) = args.values_of("highlight") {
            for highlight in highlights
                .filter_map(|set_and_key: &str| {
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
                })
                .collect::<Vec<Highlight>>()
            {
                writer = writer.with_highlight(highlight);
            }
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
            "Initializing store with {} annotation(s), {} resource(s), {} annotationset(s), {} additional store(s)",
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
    }
}
