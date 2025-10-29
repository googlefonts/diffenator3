const diffWorker = new Worker(new URL("./webworker", import.meta.url));

import {
  addAGlyph,
  addAWord,
  cmapDiff,
  setupAnimation,
  diffTables,
  diffKerns,
  diffFeatures,
  diffLanguages,
} from "./shared";

import type {
  AxesMessage,
  ReceivedMessage,
  WordDiffs,
  CmapDiff,
  GlyphDiff,
  Difference,
  SentMessage,
} from "./types";

declare global {
  interface JQuery {
    shake: (interval?: number, distance?: number, times?: number) => JQuery;
  }
}

jQuery.fn.shake = function (
  interval?: number,
  distance?: number,
  times?: number
) {
  interval = typeof interval == "undefined" ? 100 : interval;
  distance = typeof distance == "undefined" ? 10 : distance;
  times = typeof times == "undefined" ? 3 : times;
  var jTarget = $(this);
  jTarget.css("position", "relative");
  for (var iter = 0; iter < times + 1; iter++) {
    jTarget.animate(
      {
        left: iter % 2 == 0 ? distance : distance * -1,
      },
      interval
    );
  }
  return jTarget.animate(
    {
      left: 0,
    },
    interval
  );
};

class Diffenator {
  beforeFont: Uint8Array | null;
  afterFont: Uint8Array | null;
  customWords: string[];

  constructor() {
    this.beforeFont = null;
    this.afterFont = null;
    this.customWords = [];
  }

  get beforeCssStyle() {
    return (document.styleSheets[0]!.cssRules[0]! as CSSStyleRule).style;
  }
  get afterCssStyle() {
    return (document.styleSheets[0]!.cssRules[1]! as CSSStyleRule).style;
  }

  setVariationStyle(variations: string) {
    let rule = (document.styleSheets[0]!.cssRules[2]! as CSSStyleRule).style;
    rule.setProperty("font-variation-settings", variations);
  }

  dropFile(files: FileList, element: HTMLElement) {
    let file = files[0]!;
    if (!file.name.match(/\.[ot]tf$/i)) {
      $(element).shake();
      return;
    }
    var style;
    if (element.id == "fontbefore") {
      style = this.beforeCssStyle;
      $(element).find("h2").addClass("font-before");
    } else {
      style = this.afterCssStyle;
      $(element).find("h2").addClass("font-after");
    }
    // window["thing"] = files[0];
    $(element).find("h2").text(file.name);
    style.setProperty("src", "url(" + URL.createObjectURL(file) + ")");
    var reader = new FileReader();
    let that = this;
    reader.onload = function (e) {
      if (!this.result) return;
      let u8 = new Uint8Array(this.result as ArrayBuffer);
      if (element.id == "fontbefore") {
        that.beforeFont = u8;
      } else {
        that.afterFont = u8;
      }
      if (that.beforeFont && that.afterFont) {
        that.letsDoThis();
      }
    };
    reader.readAsArrayBuffer(file);
  }

  dropWordlist(files: FileList) {
    var reader = new FileReader();
    let that = this;
    reader.onload = function (e) {
      // Read file as text
      let contents = (e.target?.result as string) || "";
      that.customWords = contents
        .split("\n")
        .map(function (line) {
          return line.trim();
        })
        .filter(function (line) {
          return line.length > 0 && !line.startsWith("#");
        });
      $("#wordlistlabel").text(
        `${that.customWords.length} words loaded (you can drop more)`
      );
    };
    reader.readAsText(files[0]!);
  }

  setVariations() {
    let cssSetting = $<HTMLInputElement>("#axes input")
      .map(function () {
        return `"${this.id.replace("axis-", "")}" ${this.value}`;
      })
      .get()
      .join(", ");
    this.setVariationStyle(cssSetting);
    this.updateGlyphs();
  }

  setupAxes(message: AxesMessage) {
    $("#axes").empty();
    console.log(message);
    let { axes, instances } = message;
    for (var [tag, limits] of Object.entries(axes)) {
      let [axis_min, axis_def, axis_max] = limits;
      let axis = $(`<div class="axis">
				${tag}
				<input type="range" min="${axis_min}" max="${axis_max}" value="${axis_def}" class="slider" id="axis-${tag}">
			`);
      $("#axes").append(axis);
      axis.on("input", this.setVariations.bind(this));
      axis.on("change", this.updateWords.bind(this));
    }
    if (Object.keys(instances).length > 0) {
      let select = $<HTMLSelectElement>(
        "<select id='instance-select'></select>"
      );
      for (var [name, location] of instances) {
        let location_str = Object.entries(location)
          .map(([k, v]) => `${k}=${v}`)
          .join(",");
        let option = $(`<option value="${location_str}">${name}</option>`);
        select.append(option);
      }
      select.on("change", function () {
        let location = $(this).val() as string;
        let parts = location.split(",");
        for (let [i, part] of parts.entries()) {
          let [tag, value] = part.split("=");
          console.log(tag, value);
          $(`#axis-${tag}`).val(value as string);
        }
        $("#axes input").trigger("input");
        $("#axes input").trigger("change");
      });
      $("#axes").append(select);
    }
  }

  progress_callback(message: ReceivedMessage) {
    console.log("Got json ", message);
    if ("type" in message && message.type == "ready") {
      $("#bigLoadingModal").hide();
      $("#startModal").show();
    } else if (message.type == "axes") {
      this.setupAxes(message); // Contains axes and named instances
    } else if (message.type == "tables") {
      // console.log("Hiding spinner")
      $("#spinnerModal").hide();
      diffTables(message);
      diffKerns(message);
    } else if (message.type == "kerns") {
      // console.log("Hiding spinner")
      $("#spinnerModal").hide();
      diffKerns(message);
    } else if (message.type == "modified_glyphs") {
      $("#spinnerModal").hide();
      let glyph_diff = message.modified_glyphs;
      this.renderGlyphDiff(glyph_diff);
      $(".node").on("click", function (event) {
        $(this).children().toggle();
        event.stopPropagation();
      });
    } else if (message.type == "cmap_diff") {
      $("#spinnerModal").hide();
      this.renderCmapDiff(message.cmap_diff);
      $(".node").on("click", function (event) {
        $(this).children().toggle();
        event.stopPropagation();
      });
    } else if (message.type == "words") {
      $("#spinnerModal").hide();
      $("#wordspinner").hide();
      let diffs: WordDiffs = message.words;
      for (var [script, words] of Object.entries(diffs)) {
        this.renderWordDiff(script, words);
      }
    } else if (message.type == "languages") {
      $("#spinnerModal").hide();
      diffLanguages(message.languages);
    } else {
      console.log("Unknown message", message);
    }
  }

  variationLocation() {
    // Return the current axis location as a string of the form
    // tag=value,tag=value
    return $<HTMLInputElement>("#axes input")
      .map(function () {
        return `${this.id.replace("axis-", "")}=${this.value}`;
      })
      .get()
      .join(",");
  }

  letsDoThis() {
    $("#startModal").hide();
    $("#spinnerModal").show();
    for (let command of ["axes", "tables", "kerns", "cmap_diff", "languages"]) {
      console.log("Sending command", command);
      diffWorker.postMessage({
        command,
        beforeFont: this.beforeFont!,
        afterFont: this.afterFont!,
      } as SentMessage);
    }
    this.updateGlyphs();
    this.updateWords();
  }

  updateGlyphs() {
    let location = this.variationLocation();
    diffWorker.postMessage({
      command: "modified_glyphs",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
      location,
    } as SentMessage);
  }

  updateWords() {
    $("#wordspinner").show();
    $("#worddiffinner").empty();
    let location = this.variationLocation();
    diffWorker.postMessage({
      command: "words",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
      customWords: this.customWords,
      location,
    } as SentMessage);
  }

  renderCmapDiff(cmap_diff: CmapDiff) {
    $("#cmapdiff").empty();
    cmapDiff(cmap_diff);
    $('[data-bs-toggle="tooltip"]').tooltip();
  }

  renderGlyphDiff(glyph_diff: GlyphDiff[]) {
    $("#glyphdiff").empty();
    if (glyph_diff.length > 0) {
      $("#glyphdiff").append($(`<h4>Modified glyphs</h4>`));
      let place = $('<div class="glyphgrid"/>');
      $("#glyphdiff").append(place);

      glyph_diff.forEach((glyph) => {
        addAGlyph(glyph, place);
      });
      $('[data-bs-toggle="tooltip"]').tooltip();
    }
  }

  renderWordDiff(script: string, diffs: Difference[]) {
    $("#worddiffinner").append($(`<h6>${script}</h6>`));
    let place = $('<div class="wordgrid"/>');
    $("#worddiffinner").append(place);
    diffs.forEach((diff) => {
      addAWord(diff, place);
    });
    $('[data-bs-toggle="tooltip"]').tooltip();
  }
}

$(function () {
  let diffenator = new Diffenator();
  diffWorker.onmessage = (e) => diffenator.progress_callback(e.data);
  $("#bigLoadingModal").show();

  $(".drop").on("dragover dragenter", function (e) {
    e.preventDefault();
    e.stopPropagation();
    $(this).addClass("dragging");
  });
  $(".drop").on("dragleave dragend", function (e) {
    $(this).removeClass("dragging");
  });

  $(".fontdrop").on("drop", function (e) {
    $(this).removeClass("dragging");
    if (
      e.originalEvent!.dataTransfer &&
      e.originalEvent!.dataTransfer.files.length
    ) {
      e.preventDefault();
      e.stopPropagation();
      diffenator.dropFile(e.originalEvent!.dataTransfer.files, this);
    }
  });

  $("#worddrop").on("drop", function (e) {
    $(this).removeClass("dragging");
    if (
      e.originalEvent!.dataTransfer &&
      e.originalEvent!.dataTransfer.files.length
    ) {
      e.preventDefault();
      e.stopPropagation();
      diffenator.dropWordlist(e.originalEvent!.dataTransfer.files);
    }
  });

  setupAnimation();

  $("body").tooltip({
    selector: '[data-toggle="tooltip"]',
  });
});
