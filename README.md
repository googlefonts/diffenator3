# Diffenator3

This is a Rust port of
[diffenator2](https://github.com/googlefonts/diffenator2), a utility for
comparing two font files. It comes in two flavours, command line and
WASM.

The _command line_ version compares two fonts and describes the differences
between them; it produces reports in text, JSON and HTML format. By default,
it compares variable fonts at their named instances, although you can also
ask for comparisons at specific points in the design space, at the
min/max/default for each axis or subdivisions in between, or at master
locations. See the `--help` documentation of `diffenator3` for more details.

You can customize the look and feel of the HTML report by editing the templates
in the `~/.diffenator3/templates` directory after running `diffenator3 --html`
for the first time. Additionally, you can supply a `--templates` directory for
per-project templates.

The _WASM_ version compares two font files over the web and displays a HTML
report of the differences. This runs the `diffenator3` code directly inside
your web browser - the fonts are *not* transferred across the Internet.
You can use the WASM version at
https://googlefonts.github.io/diffenator3

## diff3proof

As well as `diffenator3`, there is another utility called `diff3proof` used
to generate HTML proof files showing the difference between the fonts. This
can be used in two modes: `--sample-mode context` (the default), which
shows paragraphs of sample text for each language supported by the font, and
`--sample-mode cover`, which shows a minimal text to cover all the shared
codepoints in the font. These can be helpful for manually checking rendering
differences in different browsers.

## Additional utilities

If you build `diffenator3` from source, there are three additional workspace
crates which build some utilities which are mainly helpful for working on
`diffenator3` itself:

- [`ttj`](ttj/) serializes a TTF file to JSON in much the same way that `ttx`
  serializes to XML. However, there is no deserialization back to TTF at
  present.
- [`kerndiffer`](kerndiffer/) is a limited version of `diffenator3` just for
  checking kerning differences. You can achieve much the same functionality
  with `diffenator3 --no-tables --no-words --no-glyphs`.
- [`rendertest`](rendertest/) is used to test the rendering and bitmap comparison
  functionality of `diffenator3`. It generates bitmap images of words in both
  fonts and then overlays them.

## Installing

Binary versions can be obtained from the latest GitHub release; development
versions can be obtained via the latest GitHub Action. The `diffenator3` and
`diff3proof` binaries are all you need - they contain all the templates and
wordlists within them - so you can copy them to anywhere in your path.

Alternatively you can install from source with
`cargo install --git https://github.com/googlefonts/diffenator3`.

## License

This software is licensed under the [Apache 2.0 License](LICENSE.md).
