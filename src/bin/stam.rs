use clap::{App, Arg, SubCommand};
use stam::{AnnotationStore, AnyId, Handle, Storable};
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
            .help("Verbose output")
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

fn main() {
    let rootargs = App::new("STAM")
        .version(VERSION)
        .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>")
        .about("CLI tool to work with standoff text annotation (STAM)")
        .subcommand(
            SubCommand::with_name("info")
                .about("Return information regarding a STAM model")
                .args(&common_arguments()),
        )
        .get_matches();

    let args = if let Some(args) = rootargs.subcommand_matches("info") {
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
                store = AnnotationStore::from_file(filename).expect("Error loading file");
            } else {
                store.merge_from_file(filename).expect("Error merging file");
            }
        }
    }

    if rootargs.subcommand_matches("info").is_some() {
        info(&store, args.is_present("verbose"));
    }
}
