use clap::{App, Arg, ArgAction, ArgMatches, SubCommand};
use stam::{AnnotationStore, Config, Configurable};
use std::process::exit;

mod annotate;
mod info;
mod init;
mod tag;
mod to_text;
mod to_tsv;
mod validate;

use crate::annotate::*;
use crate::info::*;
use crate::init::*;
use crate::tag::*;
use crate::to_text::*;
use crate::to_tsv::*;
use crate::validate::*;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn common_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("store")
            .help(
                "Input file containing an annotation store in STAM JSON. Set value to - for standard input.",
            )
            .takes_value(true)
            .required(true)
            .action(ArgAction::Append),
    );
    args.push(
        Arg::with_name("verbose")
            .long("verbose")
            .short('V')
            .help("Produce verbose output")
            .required(false),
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
    args
}

/// Translate command line arguments to stam library's configuration structure
fn config_from_args(args: &ArgMatches) -> Config {
    let mut config = Config::default();
    if args.is_present("no-include") {
        config.use_include = false;
    }
    if args.is_present("debug") {
        config.debug = true;
    }
    config
}

fn main() {
    let rootargs = App::new("STAM Tools")
        .version(VERSION)
        .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>")
        .about("CLI tool to work with standoff text annotation (STAM)")
        .subcommand(
            SubCommand::with_name("info")
                .about("Return information regarding a STAM model. Set --verbose for extra details.")
                .args(&common_arguments())
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("validate")
                .about("Validate a STAM model. Set --verbose to have it output the STAM JSON to standard output.")
                .args(&common_arguments())
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("to-tsv")
                .about("Output all annotations in a simple TSV format. Set --verbose for extra columns.")
                .args(&common_arguments())
                .args(&config_arguments()),
        )
        .subcommand(
            SubCommand::with_name("to-text")
                .about("Output the plain text of one or more resource(s). Requires --resource")
                .args(&common_arguments())
                .args(&config_arguments())
                .arg(
                    Arg::with_name("resource")
                        .long("resource")
                        .short('r')
                        .help(
                            "The resource ID (not necessarily the filename!) of the text to output",
                        )
                        .takes_value(true)
                        .required(true)
                        .action(ArgAction::Append),
                ),
        )
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialize a new stam annotationstore")
                .args(&annotate_arguments())
                .args(&config_arguments())
                .arg(
                    Arg::with_name("annotationstore")
                        .help(
                            "Output file for the annotation store (will be overwritten if it already exists!). Set to - for standard output.",
                        )
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("annotate")
                .about("Add annotations (or datasets, resources) to an existing annotationstore")
                .args(&annotate_arguments())
                .args(&common_arguments())
                .args(&config_arguments())
                .arg(
                    Arg::with_name("annotationstore")
                        .help(
                            "Input and output file for the annotation store, will be edited in-place. Set to - for standard input and output",
                        )
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("tag")
                .about("Regular-expression based tagger on plain text")
                .args(&common_arguments())
                .args(&config_arguments())
                .arg(
                    Arg::with_name("rules")
                        .help(
                            "A TSV file containing regular expression rules for the tagger.",
                        )
                        .long_help("A TSV file containing regular expression rules for the tagger.
The file contains the following columns:

1. The regular expression following the following syntax: https://docs.rs/regex/latest/regex/#syntax
   The expression must contain one or or more capture groups containing the items that will be
   tagged (anything else is considered context and will not be tagged)
2. The ID of annotation data set
3. The ID of the data key
4. The value to set. If this follows the syntax $1,$2,etc.. it will assign the value of that capture group (1-indexed). Use $0 for all capture groups combined (space delimited).
")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .get_matches();

    let args = if let Some(args) = rootargs.subcommand_matches("info") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("to-tsv") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("to-text") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("validate") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("init") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("annotate") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("tag") {
        args
    } else {
        eprintln!("No command specified, please see stam --help");
        exit(2);
    };

    let mut store = AnnotationStore::new().with_config(config_from_args(args));

    if rootargs.subcommand_matches("info").is_some()
        || rootargs.subcommand_matches("to-tsv").is_some()
        || rootargs.subcommand_matches("to-text").is_some()
        || rootargs.subcommand_matches("validate").is_some()
    {
        if args.is_present("store") {
            let storefiles = args.values_of("store").unwrap().collect::<Vec<&str>>();
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
    }

    if rootargs.subcommand_matches("info").is_some() {
        info(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("to-tsv").is_some() {
        to_tsv(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("to-text").is_some() {
        let resource_ids = args.values_of("resource").unwrap().collect::<Vec<&str>>();
        to_text(&store, resource_ids);
    } else if rootargs.subcommand_matches("validate").is_some() {
        validate(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("init").is_some()
        || rootargs.subcommand_matches("annotate").is_some()
    {
        let filename = args
            .value_of("annotationstore")
            .expect("an annotation store must be provided");
        if rootargs.subcommand_matches("annotate").is_some() {
            //load the store
            store = AnnotationStore::from_file(filename, config_from_args(args)).unwrap_or_else(
                |err| {
                    eprintln!("Error loading annotation store: {}", err);
                    exit(1);
                },
            );
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
            store = init(
                &resourcefiles,
                &setfiles,
                &storefiles,
                &annotationfiles,
                args.value_of("id"),
                config_from_args(args),
            );
        } else {
            store = annotate(
                store,
                &resourcefiles,
                &setfiles,
                &storefiles,
                &annotationfiles,
            );
        }
        if !args.is_present("dry-run") {
            store.to_file(filename).unwrap_or_else(|err| {
                eprintln!("Failed to write annotation store {}: {}", filename, err);
                exit(1);
            });
        }
    } else if rootargs.subcommand_matches("tag").is_some() {
        tag(
            &mut store,
            args.value_of("rules").expect("--rules must be provided"),
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
    }
}
