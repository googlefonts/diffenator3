var module = import('../pkg/diffenator3.js');
async function init() {
    let wasm = await module;
    self.postMessage({ type: "ready" })
    console.log("Got wasm module", wasm);
    wasm.debugging();
    self.onmessage = async (event) => {
        console.log("Worker received message");
        console.log(event);
        const { command, beforeFont, location, afterFont } = event.data;
        if (command == "axes") {
            self.postMessage({
                "type": "axes",
                "axes": JSON.parse(wasm.axes(beforeFont, afterFont))["axes"]
            });
        } else if (command == "tables") {
            wasm.diff_tables(beforeFont, afterFont, (tables) => {
                self.postMessage({
                    "type": "tables",
                    "tables": JSON.parse(tables)["tables"]
                });
            });
        } else if (command == "glyphs") {
            wasm.diff_glyphs(beforeFont, afterFont, location, (glyphs) => {
                self.postMessage({
                    "type": "glyphs",
                    "glyphs": JSON.parse(glyphs)["glyphs"]
                });
            });
        } else if (command == "words") {
            wasm.diff_words(beforeFont, afterFont, location, (words) => {
                self.postMessage({
                    "type": "words",
                    "words": JSON.parse(words)["words"]
                });
            });
        }

    }
    return self;
}

init();
