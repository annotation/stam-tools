use stam::AnnotationStore;

pub fn validate(
    store: &AnnotationStore,
    verbose: bool,
    allow_incomplete: bool,
) -> Result<(), String> {
    let result = store.validate_text(!verbose);
    let valid = if allow_incomplete {
        result.is_ok_maybe_incomplete()
    } else {
        result.is_ok()
    };
    if valid {
        if verbose {
            eprintln!("Succesfully validated {} annotations", result.valid());
        }
        Ok(())
    } else {
        Err(format!(
            "Failed to validate {} annotations, {} missing. {}",
            result.invalid(),
            result.missing(),
            if result.missing() > 0 {
                "Did you generate validation information? (run: stam validate --make)"
            } else {
                ""
            }
        ))
    }
}
