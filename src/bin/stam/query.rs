use clap::{Arg, ArgAction, ArgMatches};
use stam::*;
use std::process::exit;

pub fn query_arguments<'a>() -> Vec<clap::Arg<'a>> {
    let mut args: Vec<Arg> = Vec::new();
    args.push(
        Arg::with_name("query")
            .long("query")
            .short('q')
            .help("A query in STAMQL")
            .action(ArgAction::Append)
            .takes_value(true),
    );
    args
}

pub struct QueryResult<'a> {
    iter: QueryIter<'a>,
    names: QueryNames,
}

pub fn query<'a>(store: &'a AnnotationStore, args: &'a ArgMatches) -> Vec<QueryResult<'a>> {
    let queries = args
        .values_of("query")
        .unwrap()
        .map(|querystr| match querystr.try_into() {
            Ok(query) => query,
            Err(e) => {
                eprintln!("{}", e);
                exit(2);
            }
        })
        .collect::<Vec<Query<'a>>>();

    let mut results = Vec::new();
    for query in queries {
        let iter = store.query(query);
        let result = QueryResult {
            names: iter.names(),
            iter,
        };
        results.push(result);
    }
    results
}
