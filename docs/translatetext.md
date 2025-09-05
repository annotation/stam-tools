# stam translatetext: translation rules configuration 

The `stam translatetext` tool is used to 'translate' characters or character
groups in text to any substitutions.

These rules is provided in an external configuration file which is passed to
`stam translatetext` via the `--rules` parameter. The basic syntax for this configuration
file is [toml](https://toml.io/en/). Basic familiarity with toml is assumed,
the details of the actual configuration language are explained in this
document.

## Global configuration

### id_suffix

When translating a text, a new text will be created with an ID derived from the
source text. This tool derived an ID by appending the original ID with a period
and a suffix as determined by this property. The same suffix will be used in
deriving the filename (this suffix will always precede any `.txt` extension).

Example:

```toml
id_suffix = "normalized"
```

### discard_unmatched

Boolean, when set, there will be no fallback rule and text that does not match will be discarded and not copied. (default to false)

### debug

Boolean to enable debug mode (can also be set from the command-line interface)

## Translation Rule

Translation rules are evaluated in reverse order, that means you need to put
more generic rules before specific ones (and longer after shorter ones). The
latest matching rule will always be used and only one rule can match.

If no rule matches, text will be copied as-is, unless `discard_unmatched` is
set, in whicih case unmatching text will be discarded.

A translation rule has the following keys:

### source

The substring to match, if put between slashes it will be interpreted as a regular expression using [Rust's regex syntax](https://docs.rs/regex/latest/regex/#syntax).

### target

The substring to translate to. Three special values are supported in addition to string literals:

* `$UPPER` - an uppercase version of the matching source string
* `$LOWER` - an lowercase version of the matching source string
* `$REV` - the matching source string in reversed character order

### case_sensitive

Set this to `false` if you want case insensitive matching for the source fragment.

### left

The left context to match (case-sensitive), if put between slashes it will be interpreted as a regular expression (and can be made case-insensitive using `(?i)`.

### right

The right context to match (case-sensitive), if put between slashes it will be interpreted as a regular expression (and can be made case-insensitive using `(?i)`.

### invert_context_match

Boolean, if set to true, requires the left and right context NOT to match what was specified. Default to false.

### Example

Example of a rule for dehyphenation:

```
[[rules]]
source = "-\n"
target = ""
left = "/\\p{L}/"
right = "/\\p{L}/"
```

Example of a rule for lowercasing:

```
[[rules]]
source = "/\\p{Uppercase}/"
target = "$LOWER"
```
