use clap::{Arg, ArgAction, ArgMatches};
use stam::*;
use std::process::exit;

pub fn query_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("set")
            .long("set")
            .help("Annotation dataset to query")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("key")
            .long("key")
            .help("Datakey to query")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("value")
            .long("value")
            .help("Exact value to query")
            .takes_value(true),
    );
    args.push(
        Arg::with_name("value-in")
            .long("value-in")
            .help("A disjunction of values")
            .takes_value(true),
    );
    args
}

pub fn query<'a>(
    store: &'a AnnotationStore,
    args: &'a ArgMatches,
    setdelimiter: &str,
) -> (AnnotationsIter<'a>, bool, Option<String>) {
    //                     ^-- expresses whether there has been any filtering performed
    if let Some(set) = args.value_of("set") {
        let key: &str = args.value_of("key").expect("Expected argument: --key");
        let operator: DataOperator = if let Some(value) = args.value_of("value") {
            value.into()
        } else if let Some(values) = args.values_of("value-in") {
            DataOperator::Or(values.map(|x: &str| x.into()).collect())
        } else {
            DataOperator::Any
        };
        eprintln!(
            "Querying set \"{}\", key \"{}\", value {:?}...",
            set, key, operator
        );
        assert!(store.dataset(set).is_some());
        (
            store.find_data(set, key, operator).annotations(),
            true,
            Some(format!("{}{}{}", set, setdelimiter, key)),
        )
    } else if args.is_present("key") || args.is_present("value") {
        eprintln!("Expected argument: --set");
        exit(2);
    } else {
        (store.annotations(), false, None)
    }
}
