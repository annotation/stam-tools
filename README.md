<p align="center">
    <img src="https://github.com/annotation/stam/raw/master/logo.png" alt="stam logo" width="320" />
</p>

[![Crate](https://img.shields.io/crates/v/stam-tools.svg)](https://crates.io/crates/stam-tools)
[![GitHub release](https://img.shields.io/github/release/annotation/stam-tools.svg)](https://GitHub.com/annotation/stam-tools/releases/)
[![Project Status: Active – The project has reached a stable, usable state and is being actively developed.](https://www.repostatus.org/badges/latest/active.svg)](https://www.repostatus.org/#active)

# STAM Tools

A collection of command-line tools for working with STAM.

Various tools are grouped under the `stam` tool, and invoked with a subcommand:

* ``stam annotate``  - Add an annotation from a JSON file
* ``stam info``      - Return information regarding a STAM model. 
* ``stam init``      - Initialize a new STAM annotationstore
* ``stam print``     - Output the text of any resources in the model.
* ``stam export``    - Export STAM data in tabular form to a simple TSV (Tab Separated Values) format. This is not lossless but provides a decent view on the data. It provides a lot of flexibility by allowing you to configure the output columns.
* ``stam validate``  - Validate a STAM model.  
* ``stam save``      - Write a STAM model to file(s). This can be used to switch between STAM JSON and STAM CSV output, based on the extension.
* ``stam tag``       - Regular-expression based tagger on plain text. 

For many of these, you can set `--verbose` for extra details in the output.

## Installation

### From source

```
$ cargo install stam-tools
```

## Usage

Add the ``--help`` flag after the subcommand for extensive usage instructions.

Most tools take as input a STAM JSON file containing an annotation store. Any
files mentioned via the `@include` mechanism are loaded automatically.

Instead of passing STAM JSON files, you can read from stdin and/or output to
stdout by setting the filename to ``-``, this works in many places.

These tools also support reading and writing [STAM CSV](https://github.com/annotation/stam/tree/master/extensions/stam-csv).

## Tools

### stam tag

The `stam tag` tool can be used for matching regular expressions in text and subsequently associated annotations with the found results. It is a tool to do for example tokenization or other tagging tasks.

The `stam tag` command takes a TSV file ([example](https://github.com/knaw-huc/stam-experiments/blob/main/config/stam-tag/simpletagger.tsv)) containing regular expression rules for the tagger.
The file contains the following columns:
 
1. The regular expressions follow the [this syntax](https://docs.rs/regex/latest/regex/#syntax).
   The expression may contain one or or more capture groups containing the items that
will be
   tagged, in that case anything else is considered context and will not be tagged.
2. The ID of annotation data set
3. The ID of the data key
4. The value to set. If this follows the syntax $1,$2,etc.. it will assign the value of
that capture group (1-indexed).

Example:

```tsv
#EXPRESSION	#ANNOTATIONSET	#DATAKEY	#DATAVALUE
\w+(?:[-_]\w+)*	simpletokens	type	word
[\.\?,/]+	simpletokens	type	punctuation
[0-9]+(?:[,\.][0-9]+)	simpletokens	type	number
```


