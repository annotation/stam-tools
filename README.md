<p align="center">
    <img src="https://github.com/annotation/stam/raw/master/logo.png" alt="stam logo" width="320" />
</p>

[![Crate](https://img.shields.io/crates/v/stam-tools.svg)](https://crates.io/crates/stam-tools)
[![GitHub release](https://img.shields.io/github/release/annotation/stam-tools.svg)](https://GitHub.com/annotation/stam-tools/releases/)
[![Project Status: Active – The project has reached a stable, usable state and is being actively developed.](https://www.repostatus.org/badges/latest/active.svg)](https://www.repostatus.org/#active)

# STAM Tools

A collection of command-line tools for working with [STAM](https://github.com/annotation/stam), a data-model for stand-off annotations on text.

Various tools are grouped under the `stam` tool, and invoked with a subcommand:

* ``stam annotate``  - Add annotations (or datasets or resources) from STAM JSON files
* ``stam info``      - Return information regarding a STAM model. 
* ``stam init``      - Initialize a new STAM annotationstore
* ``stam import``    - Import STAM data in tabular from from a simple TSV (Tab Separated Values) format, allows custom columns.
* ``stam print``     - Output the text of any resources in the model.
* ``stam export``    - Export STAM data in tabular form to a simple TSV (Tab Separated Values) format. This is not lossless but provides a decent view on the data. It provides a lot of flexibility by allowing you to configure the output columns as you see fit.
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

### stam init & stam annotate

The `stam init` command is used to initialize a new STAM annotationstore with
resources (`--resource`, plain text or STAM JSON), annotation data sets
(`--annotationset`, STAM JSON) and/or annotations (`--annotations`, JSON list
of annotations in STAM JSON).  

Example, the positional parameter (last one) is the annotation store to output,
it may be STAM JSON or STAM CSV:

```
$ stam init --resource document.txt new.store.stam.json
```

The `stam annotate` command is almost identical to `stam init`, except it reads
and modifies an existing annotation store, rather than starting a new one from
scratch:

```
$ stam annotate --resource document.txt existing.store.stam.json
```

Whenever you load annotations and annotation data sets using these commands,
they need to already be in STAM JSON format. To import data from other formats,
use `stam import` instead.

The `stam init` and `stam annotate` commands are also capable of merging
multiple annotation stores into one.

### stam save

This command is used to load a STAM annotationstore and save it under another
name and/or other format. It can be used to convert between STAM JSON
and STAM CSV. Example:

```
$ stam save -o my.store.stam.csv my.store.stam.json
```

### stam info

The `stam info` command provides either some high-level details on the
annotation store (number of resource, annotations, etc), or with the
`--verbose` flag it goes as far as presenting, in a fairly raw format, all the
data it holds.

Example:

```
$ stam info my.store.stam.json
```

### stam export

The `stam export` tool is used to export STAM data into a tabular data format
(TSV, tab separated values). You can configure precisely what columns you want
to export using the ``--colums`` parameter. See ``stam export --help`` for a
list of supported columns. 

One of the more powerful functions is that you can specify custom columns by
specifying a set ID, a delimiter and a key ID (the delimiter by default is a
slash), for instance: `my_set/part_of_speech`. This will then output the
corresponding value in that column, if it exist.

Example:

```
$ stam export -C Id,Text,TextResource,BeginOffset,EndOffset,my_set/part_of_speech
```

This export function is not lossless, that is, it can not encode everything
that STAM supports, unlike STAM JSON and STAM CSV. It does, however, give you a great
deal of flexibility to quickly output only the data relevant for whatever your specific purpose is.

### stam import

The `stam import` tool is used to import tabular data from a TSV (Tab Separated
Values) file into STAM. Like `stam export`, you can configure precisely what
columns you want to import, using the ``--columns`` parameter. By default, the
import function will attempt to parse the first line of your TSV file as the
header and use that to figure out the column configuration.  You will often
want to set ``--annotationset`` to set a default annotation set to use for
custom columns. If you set ``--annotationset my_set`` then a column like
`part_of_speech` will be interpreted in that set (same as if you wrote
`2my_set/part_of_speech` explicitly).

Here is a simple example of a possible import TSV file (with ``--annotationset my_set``):

```tsv
Text	TextResource	BeginOffset	EndOffset	part_of_speech
Hello	hello.txt	0	5	interjection
world	hello.txt	6	10	noun
```

The import function has some special abilities. If your TSV data does not
mention specific offsets in a text resource(s), they will be looked up
automatically during the import procedure. If the text resources don't even
exist in the first place, they can be reconstructed (within certain
constraints, the output text will likely be in tokenised form only). If your
data does not explicitly reference a resource, use the ``--resource`` parameter
to point to an existing resource that will act as a default, or
``--new-resource`` for the reconstruction behaviour.

By setting ``--resource hello.txt`` or ``--new-resource hello.txt`` you can import the following much more minimal TSV:

```tsv
Text	part_of_speech
Hello	interjection
world	noun
```

The importer supports empty lines within the TSV file. When reconstructing
text, these will map to (typically) a newline in the to-be-constructed text
(this configurable with ``--outputdelimiter2``). Likewise, the delimiter
between rows is configurable with `--outputdelimiter`, and defaults to a space.

Note that `stam import` can not import everything it can itself export. It can only import rows
exported with ``--type Annotation``  (the default), in which each row
corresponds with one annotation.

### stam tag

The `stam tag` tool can be used for matching regular expressions in text and
subsequently associated annotations with the found results. It is a tool to do
for example tokenization or other tagging tasks.

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

Example of the rules:

```tsv
#EXPRESSION	#ANNOTATIONSET	#DATAKEY	#DATAVALUE
\w+(?:[-_]\w+)*	simpletokens	type	word
[\.\?,/]+	simpletokens	type	punctuation
[0-9]+(?:[,\.][0-9]+)	simpletokens	type	number
```

Example of applying this to a text resource:

```
# first we create a store and add a text resource
$ stam init --resource sometext.txt my.store.stam.json

# then we start the tagging
$ stam tag --rules rules.tsv my.store.stam.json 
```
