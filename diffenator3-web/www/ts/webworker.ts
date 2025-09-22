import type { CmapDiff, GlyphDiff } from "./api.js";

var module = import("../../pkg/diffenator3_web.js");

type Command = "tables" | "kerns" | "new_missing_glyphs";

async function init() {
  let wasm = await module;
  self.postMessage({ type: "ready" });
  // console.log("Got wasm module", wasm);
  wasm.debugging();
  let commands = {
    tables: wasm.diff_tables,
    kerns: wasm.diff_kerns,
    new_missing_glyphs: wasm.new_missing_glyphs,
    languages: wasm.diff_languages,
  };

  self.onmessage = async (event) => {
    // console.log("Worker received message");
    // console.log(event);
    const { command, beforeFont, location, afterFont, customWords } =
      event.data;

    let simpleWasmDiff = (command: Command) => {
      let post = (result: string) => {
        self.postMessage({
          type: command,
          [command]: JSON.parse(result)[command],
        });
      };
      commands[command](beforeFont, afterFont, post);
    };

    if (command == "axes") {
      let obj = JSON.parse(wasm.axes(beforeFont, afterFont));
      obj["type"] = "axes";
      self.postMessage(obj);
    } else if (
      command == "tables" ||
      command == "kerns" ||
      command == "new_missing_glyphs" ||
      command == "languages"
    ) {
      simpleWasmDiff(command);
    } else if (command == "modified_glyphs") {
      wasm.modified_glyphs(
        beforeFont,
        afterFont,
        location,
        (glyphs: string) => {
          self.postMessage({
            type: "modified_glyphs",
            modified_glyphs: JSON.parse(glyphs)[
              "modified_glyphs"
            ] as GlyphDiff[],
          });
        }
      );
    } else if (command == "words") {
      wasm.diff_words(
        beforeFont,
        afterFont,
        customWords,
        location,
        (words: string) => {
          self.postMessage({
            type: "words",
            words: JSON.parse(words)["words"],
          });
        }
      );
    }
  };
  return self;
}

init();
