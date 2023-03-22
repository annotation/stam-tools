use clap::{App, Arg, SubCommand};
use stam::{
    AnnotationDataSetBuilder, AnnotationStore, AnnotationStoreBuilder, AnyId, Handle, Storable,
    TextResourceBuilder, TextResourceHandle, TextSelection,
};
use std::process::exit;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn common_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("store")
            .help("Input file containing an annotation store in STAM JSON")
            .takes_value(true)
            .required(true)
            .multiple(true),
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

fn info(store: &AnnotationStore, verbose: bool) {
    if let Some(id) = store.id() {
        println!("ID: {}", id);
    }
    println!("Resources:              {}", store.resources_len());
    for resource in store.resources() {
        println!(
            "    - [{}] Resource ID: {}; textlength: {}",
            resource.handle().unwrap().unwrap(),
            resource.id().unwrap_or("(none)"),
            resource.textlen()
        );
        if verbose {
            for textselection in resource.textselections() {
                println!(
                    "        - [{}] TextSelection; begin: {}; end: {}",
                    textselection.handle().unwrap().unwrap(),
                    textselection.begin(),
                    textselection.end(),
                );
            }
        }
    }
    println!("Annotation datasets:    {}", store.annotationsets_len());
    for annotationset in store.annotationsets() {
        println!(
            "    - [{}] Set ID: {}; #keys: {}; #data: {}",
            annotationset.handle().unwrap().unwrap(),
            annotationset.id().unwrap_or("(none)"),
            annotationset.keys_len(),
            annotationset.data_len(),
        );
        if verbose {
            for key in annotationset.keys() {
                println!(
                    "        - [{}] Key ID: {}; #data: {}",
                    key.handle().unwrap().unwrap(),
                    key.id().unwrap_or("(none)"),
                    annotationset
                        .data_by_key(key.handle().unwrap())
                        .unwrap_or(&vec!())
                        .len()
                );
            }
            for data in annotationset.data() {
                let key = annotationset
                    .key(&AnyId::Handle(data.key()))
                    .expect("Key not found");
                println!(
                    "        - [{}] Data ID: {}; Key: {}; Value: {:?}",
                    data.handle().unwrap().unwrap(),
                    data.id().unwrap_or("(none)"),
                    key.id().unwrap_or("(none)"),
                    data.value()
                );
            }
        }
    }
    println!("Annotations:            {}", store.annotations_len());
    if verbose {
        for annotation in store.annotations() {
            println!(
                "    - [{}] Annotation ID: {}; target: {:?}; #data: {}",
                annotation.handle().unwrap().unwrap(),
                annotation.id().unwrap_or("(none)"),
                annotation.target(),
                annotation.len(),
            );
            for (key, data, annotationset) in store.data_by_annotation(annotation) {
                println!(
                    "        - [{}] Set ID: {}; Data ID: {}; Key: {}; Value: {:?}",
                    data.handle().unwrap().unwrap(),
                    annotationset.id().unwrap_or("(none)"),
                    data.id().unwrap_or("(none)"),
                    key.id().unwrap_or("(none)"),
                    data.value()
                );
            }
        }
    }
}

fn to_tsv(store: &AnnotationStore, verbose: bool) {
    for annotation in store.annotations() {
        let id = annotation.id().unwrap_or("(none)");
        for (key, data, dataset) in store.data_by_annotation(annotation) {
            // get the text to which this annotation refers (if any)
            let text: Vec<&str> = store.text_by_annotation(annotation).collect();
            if verbose {
                let textselections: Vec<(TextResourceHandle, TextSelection)> =
                    store.textselections_by_annotation(annotation).collect();
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    id,
                    dataset.id().unwrap(),
                    key.id().unwrap(),
                    data.value(),
                    text.join("|").replace("\n", " "),
                    textselections
                        .iter()
                        .map(|(reshandle, t)| {
                            let resource = store
                                .resource(&AnyId::Handle(*reshandle))
                                .expect("resource must exist");
                            format!("{}#{}-{}", resource.id().unwrap_or(""), t.begin(), t.end())
                        })
                        .collect::<Vec<String>>()
                        .join("|")
                );
            } else {
                println!(
                    "{}\t{}\t{}\t{}",
                    id,
                    key.id().unwrap(),
                    data.value(),
                    text.join("|").replace("\n", " ")
                );
            }
        }
    }
}

fn validate(store: &AnnotationStore, verbose: bool, no_include: bool) {
    if no_include || !verbose {
        store.set_serialize_mode(stam::SerializeMode::NoInclude);
    }
    let result = store.to_string();
    match result {
        Ok(result) => {
            if verbose {
                println!("{}", result)
            }
        }
        Err(err) => {
            eprintln!("Error during serialization: {}", err);
            exit(1);
        }
    }
    if no_include || !verbose {
        //reset
        store.set_serialize_mode(stam::SerializeMode::AllowInclude);
    }
}

fn init(
    resourcefiles: &[&str],
    setfiles: &[&str],
    storefiles: &[&str],
    annotationfiles: &[&str],
    id: Option<&str>,
    no_include: bool,
) {
    let mut store = AnnotationStore::new();
    if let Some(id) = id {
        store = store.with_id(id.to_string());
    }
    for filename in storefiles {
        store = store.with_file(filename).unwrap_or_else(|err| {
            eprintln!("Error merging annotation store {}: {}", filename, err);
            exit(1);
        });
    }
    let mut builder = AnnotationStoreBuilder::default();
    for filename in setfiles {
        builder.annotationsets.push(
            AnnotationDataSetBuilder::from_file(filename, !no_include).unwrap_or_else(|err| {
                eprintln!("Error loading AnnotationDataSet {}: {}", filename, err);
                exit(1);
            }),
        );
    }
    for filename in resourcefiles {
        builder.resources.push(
            TextResourceBuilder::from_file(filename, !no_include).unwrap_or_else(|err| {
                eprintln!("Error loading TextResource {}: {}", filename, err);
                exit(1);
            }),
        );
    }
    for filename in annotationfiles {
        //TODO
    }
}

fn main() {
    let rootargs = App::new("STAM")
        .version(VERSION)
        .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>")
        .about("CLI tool to work with standoff text annotation (STAM)")
        .subcommand(
            SubCommand::with_name("info")
                .about("Return information regarding a STAM model. Set --verbose for extra details.")
                .args(&common_arguments()),
        )
        .subcommand(
            SubCommand::with_name("validate")
                .about("Validate a STAM model. Set --verbose to have it output the STAM JSON to standard output.")
                .args(&common_arguments())
                .arg(
                    Arg::with_name("no-include")
                        .long("no-include")
                        .help("Serialize as one file, do not output @include directives nor standoff-files")
                        .required(false),
                ),
        )
        .subcommand(
            SubCommand::with_name("to-tsv")
                .about("Output all annotations in a simple TSV format. Set --verbose for extra columns.")
                .args(&common_arguments()),
        )
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialize a new stam annotationstore")
                .arg(
                    Arg::with_name("set")
                        .long("set")
                        .short('s')
                        .help("STAM JSON file containing an annotation data set")
                        .takes_value(true)
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("resource")
                        .long("resource")
                        .short('r')
                        .help("Plain text or STAM JSON file containing a text resource")
                        .takes_value(true)
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("store")
                        .long("store")
                        .short('i')
                        .help("STAM JSON file containing an annotation store, will be merged into the new store")
                        .takes_value(true)
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("annotation")
                        .long("annotation")
                        .short('a')
                        .help("STAM JSON file containing one or more annotations, will be merged into the new store")
                        .takes_value(true)
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("no-include")
                        .long("no-include")
                        .help("Serialize as one file, do not output @include directives nor standoff-files")
                        .required(false),
                )
                .arg(
                    Arg::with_name("id")
                        .long("id")
                        .help("Sets the identifier for the annotation store")
                        .takes_value(true)
                ),
        )
        .get_matches();

    let args = if let Some(args) = rootargs.subcommand_matches("info") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("to-tsv") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("validate") {
        args
    } else if let Some(args) = rootargs.subcommand_matches("init") {
        args
    } else {
        eprintln!("No command specified, please see stam --help");
        exit(2);
    };

    let mut store = AnnotationStore::new();

    if args.is_present("store") {
        let storefiles = args.values_of("store").unwrap().collect::<Vec<&str>>();
        for (i, filename) in storefiles.iter().enumerate() {
            eprintln!("Loading annotation store {}", filename);
            if i == 0 {
                store = AnnotationStore::from_file(filename).unwrap_or_else(|err| {
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

    if rootargs.subcommand_matches("info").is_some() {
        info(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("to-tsv").is_some() {
        to_tsv(&store, args.is_present("verbose"));
    } else if rootargs.subcommand_matches("validate").is_some() {
        validate(
            &store,
            args.is_present("verbose"),
            args.is_present("no-include"),
        );
    } else if rootargs.subcommand_matches("init").is_some() {
        let resourcefiles = args.values_of("resource").unwrap().collect::<Vec<&str>>();
        let setfiles = args.values_of("setfiles").unwrap().collect::<Vec<&str>>();
        let storefiles = args.values_of("storefiles").unwrap().collect::<Vec<&str>>();
        let annotationfiles = args.values_of("annotation").unwrap().collect::<Vec<&str>>();
        init(
            &resourcefiles,
            &setfiles,
            &storefiles,
            &annotationfiles,
            args.value_of("id"),
            args.is_present("no-include"),
        );
    }
}
