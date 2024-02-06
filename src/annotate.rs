use clap::{Arg, ArgAction};
use stam::{AnnotationDataSet, AnnotationStore, Configurable, StoreFor, TextResource};
use std::process::exit;

pub fn annotate_arguments<'a>() -> Vec<clap::Arg<'a>> {
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

pub fn annotate(
    mut store: AnnotationStore,
    resourcefiles: &[&str],
    setfiles: &[&str],
    storefiles: &[&str],
    annotationfiles: &[&str],
) -> AnnotationStore {
    for filename in storefiles {
        store = store.with_file(filename).unwrap_or_else(|err| {
            eprintln!("Error merging annotation store {}: {}", filename, err);
            exit(1);
        });
    }
    for filename in setfiles {
        let annotationset = AnnotationDataSet::from_file(filename, store.config().clone())
            .unwrap_or_else(|err| {
                eprintln!("Error loading AnnotationDataSet {}: {}", filename, err);
                exit(1);
            });
        store.insert(annotationset).unwrap_or_else(|err| {
            eprintln!("Error adding AnnotationDataSet {}: {}", filename, err);
            exit(1);
        });
    }
    for filename in resourcefiles {
        let resource =
            TextResource::from_file(filename, store.config().clone()).unwrap_or_else(|err| {
                eprintln!("Error loading TextResource {}: {}", filename, err);
                exit(1);
            });
        store.insert(resource).unwrap_or_else(|err| {
            eprintln!("Error adding TextResource {}: {}", filename, err);
            exit(1);
        });
    }
    for filename in annotationfiles {
        store.annotate_from_file(filename).unwrap_or_else(|err| {
            eprintln!("Error parsing annotations from {}: {}", filename, err);
            exit(1);
        });
    }
    store
}
