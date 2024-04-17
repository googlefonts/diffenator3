# Diffenator3

This is a Rust port of
[diffenator2](https://github.com/googlefonts/diffenator2), a utility for
comparing two font files. It comes in two flavours, command line and
WASM.

The command line version is still in very early development, and doesn't
do very much at the moment; it compares two fonts (with burned-in path
names) and outputs a JSON dictionary showing the differences between
them. 

The WASM version is a bit more interesting; it can take two font files
over the web and display a HTML report similar to diffenator2. It
doesn't yet compare variable fonts in the same way.

You can use the WASM version at https://simoncozens.github.io/diffenator3

## To rebuild the WASM version

* cargo build --target wasm32-unknown-unknown 
* cd www; npm run build
