# STAM Tools

A collection of command-line tools for working with STAM.

Various tools are grouped under the `stam` tool, and invoked with a subcommand:

* ``stam annotate``  - Add an annotation from a JSON file
* ``stam info``      - Return information regarding a STAM model. 
* ``stam init``      - Initialize a new STAM annotationstore
* ``stam to-text``   - Print the text of any resources in the model.
* ``stam to-tsv``    - Convert STAM to a simple TSV (Tab Separated Values) format. This is not lossless but provides a decent view on the data.
* ``stam validate``  - Validate a STAM model.  

For many of these, you can set `--verbose` for extra details in the output.

## Installation

### From source

```
$ cargo install stam-tools
```

## Usage

Add the ``--help`` flag for usage instructions.

Most tools take as input a STAM JSON file containing an annotation store. Any
files mentioned via the `@include` mechanism are loaded automatically.

Instead of passing STAM JSON files, you can read from stdin and/or output to
stdout by setting the filename to ``-``, this works in many places.

