# v0.15.0 - 2026-03-19

* fromxml: important bugfix in node matching behaviour, certain nodes were missed in older versions
* fromxml: support list values and multiple value matches ([#30](https://github.com/annotation/stam-tools/issues/30))
* fromxml: filters add,sub,mul,div,gt,gte,lt,lte can now deal with other types than just integers
* fromxml: added external_filters (uses arbitrary external scripts as template filters), see README for documentation.
    * Note that this has security implications, in restricted environments always ensure the fromxml configuration can not be supplied by arbitrary users

# v0.14.1 - 2026-02-04

* allow textsuffix and textprefix on annotations using TextSelectorBetweenMarkers. This implementation was missing, so markers could not have textprefixes or suffixes yet.





# v0.14.0 - 2025-12-04

* fromxml: annotation IDs are now automatically generated for all that have none explicitly assigned.
* fromxml: you can now include text prefixes and text suffixes in the main annotation by setting `include_textprefix` and/or 
 `include_textsuffix` to true.
* fromxml: you can also explicitly annotated text prefixes/suffixes using `annotatetextprefix` and `annotatetextsuffix`





# v0.13.8 - 2025-11-21

* xml: implemented XPath-like conditions on any nodes ([#29](https://github.com/annotation/stam-tools/issues/29))
* xml: allow templates to reference absolute paths from the root node  ([#28](https://github.com/annotation/stam-tools/issues/28))
* xml: added starts_with and ends_with filters
* dependency update for roxmltree (previous version was yanked)





# v0.13.6 - 2025-11-10

Updated for stam-rust v0.18.6





# v0.13.5 - 2025-10-27

Minor extra feature:

* fromxml: allow specifying a desired output file with `--inputfiles`. The output file can be in any column and has a > prepended to its name.







# v0.13.4 - 2025-10-27

Bugfix:

* fromxml: fix in handling context variables via --context-file





# v0.13.3 - 2025-10-22

* xml: added --id-strip-suffix parameter to strip certain suffixes (extensions) when deriving identifiers from filenames
* align: added --quiet parameter





# v0.13.2 - 2025-10-08

* Updated dependencies





# v0.13.1 - 2025-10-08

* Updated dependencies






# v0.13.0 - 2025-09-27

New features:

* fromxml: implemented metadata annotations and --context-file parameter
* fromxml: support simple XPath like conditional statements in templates

Bugfixes;

* fromxml: fixed complex template variable parsing (refactoring)
* fromxml: support simple XPath like conditional statements in templates





# v0.12.2 - 2025-09-21

* translate: allow multiple --extra-target-template
* translatetext: added --force parameter to force output text and translations even when the translation yields no difference from the input (default behaviour changed)





# v0.12.1 - 2025-09-12

* Fixes:
   * translatetext: fixed panic due to unchecked text slicing
   * transpose/translate: fixed usage information for --query parameter
   * transpose/translate: rewording usage help
   * transpose/translate: fallback to using the first transposition/translation if none was explicitly defined as pivot
   * translate/transpose: skip annotations that reference no text
* New:
   * transpose/translate: added --id-strategy parameter





# v0.12.0 - 2025-09-08

This release implements STAM v1.3 

* updated for stam-rust 0.17.0
* **translate**: 
   * implemented [STAM translate extension](https://github.com/annotation/stam/tree/master/extensions/stam-translate) (the `translate()` function)
* **translatetext** (aka **tr**)
   * implemented translatetext subcommand that transforms one text to another via externally-specified replacement rules (producing translation annotation in the process).
   * Though implemented, this should still be considered somewhat experimental and may be subject to changes.
* documentation for transpose/translate/translatetext
* minor dependency updates






# v0.11.1 - 2025-07-25

Bugfix:

* align: fixed case (in)sensitivity handling





# v0.11.0 - 2025-07-18

This release implements STAM v1.2 

* updated for stam-rust 0.17.0
* **fromxml**: 
   * allow untangling of multiple XML files to a single output txt [#18](https://github.com/annotation/stam-tools/issues/18)
   * added `{inputfile}` variable containing the input XML file, for use in templates.
   * added filters in templating language: `basename` and `noext`
   * provided a PageXML to STAM mapping
   * implemented provenance support by linking to the original XML files with an (W3C WebAnnotation) XPathSelector [#19](https://github.com/annotation/stam-tools/issues/19)
   * `value` can now take different types, not just strings, including maps (toml tables). All strings are interpreted as templates.
* minor dependency updates





# v0.10.0 - 2025-07-08

* allow loading text files directly and reimplemented `stam print`: 
    * `stam print` can now return text selections based on offsets
    * `stam query` has an extra output format txt, this replaces the old `stam print --query`.
    * Text files can now be provided directly instead of an annotationstores, as a shortcut,  in such cases an annotationstore will be constructed on-the-fly on the basis of it. Useful for one-time actions on text files such as `stam grep` or `stam print`.
* `fromxml`: Major update for the configuration language. (https://github.com/annotation/stam-tools/issues/17). This introduces more:
   * more powerful templating
   * base elements to prevent repetition
   * conditions in XPath expressions
   * **Breaking change: The fromxml toml configuration language has changed substantially, it is not backward compatible with earlier versions**
        





# v0.9.3 - 2025-05-28

* Updated for stam-rust 0.16.6





# v0.9.2 - 2024-11-18

*Updated for stam-rust 0.16.5
* webanno: added --extra-target-template parameter





# v0.9.1 - 2024-10-18

view: fix for ansi visualisation bug [#16](https://github.com/annotation/stam-tools/issues/16) 





# v0.9.0 - 2024-10-04

* Upgraded to stam-rust v0.16.3
* align: fixed wrong offsets in output
* align: added `grow` parameter to build grow alignments into translations rather than exact transpositions
* align: added max_errors and min_alignment_length parameters





# v0.8.3 - 2024-09-22

Upgraded to stam-rust v0.16.2





# v0.8.2 - 2024-09-22

updated to stam-rust 0.16.1





# v0.8.1 - 2024-09-22

* updated to stam-rust 0.16.0
* fromxml: removed `--single-output` parameter for now due to API changes





# v0.8.0 - 2024-08-29

* view: major refactoring, reworked visualisation mechanism ([#15](https://github.com/annotation/stam-tools/issues/15)) and reworked the way highlight queries are formulated (via subqueries now), and the way custom attributes for visualisation are conveyed
* info: added information on substores, and filenames for resources and datasets





# v0.7.4 - 2024-07-15

* split: implemented splitting annotation stores
* align: dropped ---global parameter, tied to algorithm now (Smith Watherman is local, Needleman Wunch is global)
* align: allow global/local as algorithm aliases
* added alignments tests
* added video demo





# v0.7.2 - 2024-05-27

* Updated against stam-rust v0.14.1 (minor bugfix release)





# v0.7.1 - 2024-05-26

* Updated against stam-rust v0.14.0
* stam validate: Implemented text validation (annotation/stam-rust#5)
* stam import: Fixed parsing TSV columns
* stam view: Fixed ANSI colour output for tags
* stam annotate: Fixed support for mutable queries (was missing in v0.7.0)
* stram view: minor style improvement for HTML





# v0.7.0 - 2024-05-14

* Updated against stam-rust v0.13.0
* `stam fromxml` - New subcommand to convert XML-based formats (like xHTML, TEI) to STAM. Effectively 'untangling' the text and their inline annotations to full stand-off. This tool is configurable via an external configuration that defines a mapping of XML elements and 
attributes to STAM annotations and annotationdata.
* `stam view`
    * New simplified default style
    * Support for zero-width annotations
    * New `@STYLE` attribute to associate an arbitrary CSS class with a query result
    * New `@HIDE` attribute to hide highlight underlines, and omit from the legend.
    * Major refactoring and performance improvement ([#10](https://github.com/annotation/stam-tools/issues/10))
    * Major improvements in whitespace handling for HTML output
* `stam batch` - Batch mode now allows redirecting output to file using shell-like `>` and `>>` operators. ([#13](https://github.com/annotation/stam-tools/issues/13))








# v0.6.1 - 2024-03-28

* Upgraded to use stam-rust v0.12.0





# v0.6.0 - 2024-03-15

This release introduces some major refactoring, many fixes, and some new tools:

* Upgraded to use stam-rust v0.11.0
* `stam align`: tool that aligns identical parts of two (sub)texts using Smith Waterman/Needleman Wunsch. Outputs a *transposition* following the  [STAM Transpose](https://github.com/annotation/stam/blob/master/extensions/stam-transpose/README.md) specification.
* `stam transpose`: new tool implementing the ability to map annotations from one coordinate system to another, given a transposition (e.g. like produced by `stam align`).
* `stam export`: added `--alignments` parameter that outputs transpositions in TSV format
* `stam batch` or `stam shell`: a tool to invoke stam tools subcommands in sequence. his is a good way to avoid the initialization/serialisation overhead that would normally come from invoking commands one-by-one on the command line. It also works for (limited) interactive use.
* Loading and merging multiple annotation stores was revised, fixed and applied more consistently across all tools.
* `stam view`: Fixes in newline visualisation and added `--collapse` parameter to hide tags when first loading html.

Removed:
* `stam save` is no longer needed, just use `stam init` / `stam annotate`.








# v0.5.0 - 2024-02-22

* Made `stam-tools` available as a library alongside the `stam` CLI tool. Note however that this Rust API is not considered stable yet and may be subject to change.
* `stam query`: implemented output to W3C Web Annotation via `--format w3anno` parameter.
* `stam view`: 
     * make tags togglable
     * show popups when tags are hidden
* `stam align`: new tool to align two texts (or parts thereof) using Smith Waterman or Needleman Wunsch, results in *transposition* annotations complying to the [STAM transpose](https://github.com/annotation/stam/blob/master/extensions/stam-transpose/README.md) specification
* `stam query`: allow outputting existing transpositions or other alignments using the `--alignments` parameter. This outputs to a simple TSV format and is currently limited to two-sided alignments (rather than multiple).





# v0.4.0 - 2024-01-24

* Added `stam view` tool to visualize annotations in HTML or ANSI text given STAMQL queries ([#2](https://github.com/annotation/stam-tools/issues/2))
* Revised `stam query` and `stam print` to use STAMQL queries.
* `stam export` is now just an alias for `stam query`.
* `stam query` default column output changed and is auto-detected now based on output type, may now also output row numbers and query variable names.
* various tools now output temporary IDs if no public ID exists
* Added a `stam grep` tool to search by regular expression and return offsets (TSV)
* Adapted to latest STAM library (v0.9.0)
* Improved documentation







# v0.3.0 - 2023-10-19

* Adapted to latest STAM library (v0.8.0)
* stam info: compute and show memory consumption
* added a `strip-ids` option to strip public identifiers
* documentation update





# v0.2.0 - 2023-06-07

* Renamed command `to-tsv` to `export`
* Renamed command `to-text` to `print`
* Implemented `import` command that imports from TSV files, with support for custom columns, automatic alignment with source text, and reconstructing source text from scratch [#1](https://github.com/annotation/stam-tools/issues/1) 
* export: added support for output custom columns corresponding to an AnnotationDataSet and DataKey
* Adapted to latest library (v0.7.0)






# v0.1.3 - 2023-04-19

* Adapted to latest library (stam-rust 0.6.0). 





# v0.1.2 - 2023-04-02

* Adapted to latest library (stam-rust 0.5.0). Add support for STAM CSV.
* Added `stam save` command





# v0.1.1 - 2023-03-27

Builds on stam-rust 0.4.0 (in experimental stage, so these tools are as well), fixes a number of bugs.

* Reimplemented to-tsv support to be more configurable





# v0.1.0 - 2023-03-25

Initial release, builds on stam-rust 0.3.0 (in experimental stage, so these tools are as well).

This release introduces several tools to work with STAM:

* stam annotate - Add an annotation from a JSON file
* stam info - Return information regarding a STAM model.
* stam init - Initialize a new STAM annotationstore
* stam to-text - Print the text of any resources in the model.
* stam to-tsv - Convert STAM to a simple TSV (Tab Separated Values) format. This is not lossless but provides a decent view on the data.
* stam validate - Validate a STAM model.
* stam tag - Regular-expression based tagger
