use clap::{Arg, ArgAction};
use stam::*;

pub fn query_arguments<'a>(help: &'static str) -> Vec<clap::Arg<'a>> {
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

pub fn textselection_from_queryresult<'a>(
    resultitems: &QueryResultItems<'a>,
    var: Option<&str>,
    names: &QueryNames,
) -> Result<(ResultTextSelection<'a>, bool, Option<&'a str>), &'a str> {
    //convert query result to text selection
    let resultitem = if let Some(var) = var {
        resultitems.get_by_name(names, var).ok()
    } else {
        resultitems.iter().last()
    };
    let (resulttextselection, whole_resource, id) = match resultitem {
        Some(QueryResultItem::TextSelection(textselection)) => (textselection.clone(), false, None),
        Some(QueryResultItem::TextResource(resource)) => (
            resource
                .textselection(&Offset::whole())
                .expect("textselection must succeed"),
            true,
            resource.id(),
        ),
        Some(QueryResultItem::Annotation(annotation)) => {
            let mut iter = annotation.textselections();
            if let Some(textselection) = iter.next() {
                if iter.next().is_some() {
                    return Err("Resulting annotation does not reference any text");
                }
                (textselection, false, annotation.id())
            } else {
                return Err("Resulting annotation does not reference any text");
            }
        }
        Some(QueryResultItem::AnnotationData(_)) => {
            return Err("Query produced result of type DATA, but this does not reference any text");
        }
        Some(QueryResultItem::DataKey(_)) => {
            return Err("Query produced result of type KEY, but this does not reference any text");
        }
        Some(QueryResultItem::AnnotationDataSet(_)) => {
            return Err("Query produced result of type SET, but this does not reference any text");
        }
        None | Some(QueryResultItem::None) => {
            return Err("Query produced no results");
        }
    };
    Ok((resulttextselection, whole_resource, id))
}
