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

## Translation Rule Contraints

Translation rules can formulate constraints using
[STAMQL](https://github.com/annotation/stam/tree/master/extensions/stam-query)
queries, and in doing so you can make use of annotation information in your
translation rules.

Based on whether a query succeeds or not, the translation rule matches or not.
We call these constraints. In your query you can use the variables `?source`,
`?left` and `?right`, corresponding to the source text selection under consideration and the
left and right context, respectively. There is also the variable `?resource`, representing the resource as a whole.

```
[[rules.constraints]]
query = "SELECT ANNOTATION ?a WHERE RELATION ?source EMBEDDED;"
test = "a"
```
A rule may have multiple independent constraints, all must match.

### query

The query in STAMQL. You will almost always want to make sure to use either `?source`, and/or `?left` and/or `?right` in your query (otherwise it has no relation to the rule that invokes it).

### test

The variable to test, if not specified, the first variable/result will be used.
If results are returned for this variable, the rule matches and the translation will be carried out. If it produces no results, the rule does not match and no translation will be done. If `invert` is set to true, this behaviour is inverted.

### invert

Boolean to invert the match.
