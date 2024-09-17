var module = import("../pkg/diffenator3_web.js");
async function init() {
  let wasm = await module;
  self.postMessage({ type: "ready" });
  // console.log("Got wasm module", wasm);
  wasm.debugging();
  self.onmessage = async (event) => {
    // console.log("Worker received message");
    // console.log(event);
    const { command, beforeFont, location, afterFont } = event.data;
    if (command == "axes") {
      let obj = JSON.parse(wasm.axes(beforeFont, afterFont));
      obj["type"] = "axes";
      self.postMessage(obj);
    } else if (command == "tables") {
      wasm.diff_tables(beforeFont, afterFont, (tables) => {
        self.postMessage({
          type: "tables",
          tables: JSON.parse(tables)["tables"],
        });
      });
    } else if (command == "kerns") {
      wasm.diff_kerns(beforeFont, afterFont, (kerns) => {
        self.postMessage({
          type: "kerns",
          kerns: JSON.parse(kerns)["kerns"],
        });
      });
    } else if (command == "new_missing_glyphs") {
      wasm.new_missing_glyphs(beforeFont, afterFont, (new_missing_glyphs) => {
        self.postMessage({
          type: "new_missing_glyphs",
          cmap_diff: JSON.parse(new_missing_glyphs)["new_missing_glyphs"],
        });
      });
    } else if (command == "modified_glyphs") {
      wasm.modified_glyphs(beforeFont, afterFont, location, (glyphs) => {
        self.postMessage({
          type: "modified_glyphs",
          modified_glyphs: JSON.parse(glyphs)["modified_glyphs"],
        });
      });
    } else if (command == "words") {
      wasm.diff_words(beforeFont, afterFont, location, (words) => {
        self.postMessage({
          type: "words",
          words: JSON.parse(words)["words"],
        });
      });
    }
  };
  return self;
}

init();
