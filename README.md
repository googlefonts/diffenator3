# Diffenator3

This is a Rust port of
[diffenator2](https://github.com/googlefonts/diffenator2), a utility for
comparing two font files. It comes in two flavours, command line and
WASM.

The command line version compares two fonts and either outputs a JSON dictionary showing the differences between them or a textual report.

The WASM version is a bit more interesting; it can take two font files
over the web and display a HTML report similar to diffenator2. It
doesn't yet compare variable fonts in the same way.

You can use the WASM version at https://simoncozens.github.io/diffenator3

## To rebuild the WASM version

* wasm-pack build
* cd www; npm run build

The results appear in docs/