import Worker from "worker-loader!./webworker.js";
const diffWorker = new Worker();

import {
  addAGlyph,
  addAWord,
  cmapDiff,
  setupAnimation,
  diffTables,
  diffKerns,
} from "../../diffenator3-cli/templates/shared";

jQuery.fn.shake = function (interval, distance, times) {
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
  constructor() {
    this.beforeFont = null;
    this.afterFont = null;
  }

  get beforeCssStyle() {
    return document.styleSheets[0].cssRules[0].style;
  }
  get afterCssStyle() {
    return document.styleSheets[0].cssRules[1].style;
  }

  setVariationStyle(variations) {
    let rule = document.styleSheets[0].cssRules[2].style;
    rule.setProperty("font-variation-settings", variations);
  }

  dropFile(files, element) {
    if (!files[0].name.match(/\.[ot]tf$/i)) {
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
    window.thing = files[0];
    $(element).find("h2").text(files[0].name);
    style.setProperty("src", "url(" + URL.createObjectURL(files[0]) + ")");
    var reader = new FileReader();
    let that = this;
    reader.onload = function (e) {
      let u8 = new Uint8Array(this.result);
      if (element.id == "fontbefore") {
        that.beforeFont = u8;
      } else {
        that.afterFont = u8;
      }
      if (that.beforeFont && that.afterFont) {
        that.letsDoThis();
      }
    };
    reader.readAsArrayBuffer(files[0]);
  }

  setVariations() {
    let cssSetting = $("#axes input")
      .map(function () {
        return `"${this.id.replace("axis-", "")}" ${this.value}`;
      })
      .get()
      .join(", ");
    this.setVariationStyle(cssSetting);
    this.updateGlyphs();
  }

  setupAxes(message) {
    $("#axes").empty();
    console.log(message);
    let { axes, instances } = message;
    for (var [tag, limits] of Object.entries(axes)) {
      console.log(tag, limits);
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
      let select = $("<select id='instance-select'></select>");
      for (var [name, location] of instances) {
        console.log(location);
        let location_str = Object.entries(location)
          .map(([k, v]) => `${k}=${v}`)
          .join(",");
        let option = $(`<option value="${location_str}">${name}</option>`);
        select.append(option);
      }
      select.on("change", function () {
        let location = $(this).val();
        let parts = location.split(",");
        for (let [i, part] of parts.entries()) {
          let [tag, value] = part.split("=");
          console.log(tag, value);
          $(`#axis-${tag}`).val(value);
        }
        $("#axes input").trigger("input");
        $("#axes input").trigger("change");
      });
      $("#axes").append(select);
    }
  }

  progress_callback(message) {
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
    } else if (message.type == "new_missing_glyphs") {
      $("#spinnerModal").hide();
      this.renderCmapDiff(message);
      $(".node").on("click", function (event) {
        $(this).children().toggle();
        event.stopPropagation();
      });
    } else if (message.type == "words") {
      $("#spinnerModal").hide();
      $("#wordspinner").hide();
      let diffs = message.words;
      for (var [script, words] of Object.entries(diffs)) {
        this.renderWordDiff(script, words);
      }
    }
  }

  variationLocation() {
    // Return the current axis location as a string of the form
    // tag=value,tag=value
    return $("#axes input")
      .map(function () {
        return `${this.id.replace("axis-", "")}=${this.value}`;
      })
      .get()
      .join(",");
  }

  letsDoThis() {
    $("#startModal").hide();
    $("#spinnerModal").show();
    diffWorker.postMessage({
      command: "axes",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
    });
    diffWorker.postMessage({
      command: "tables",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
    });
    diffWorker.postMessage({
      command: "kerns",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
    });
    diffWorker.postMessage({
      command: "new_missing_glyphs",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
    });
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
    });
  }

  updateWords() {
    $("#wordspinner").show();
    $("#worddiffinner").empty();
    let location = this.variationLocation();
    diffWorker.postMessage({
      command: "words",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
      location,
    });
  }

  renderCmapDiff(glyph_diff) {
    $("#cmapdiff").empty();
    cmapDiff(glyph_diff);
    $('[data-toggle="tooltip"]').tooltip();
  }

  renderGlyphDiff(glyph_diff) {
    $("#glyphdiff").empty();
    if (glyph_diff.length > 0) {
      $("#glyphdiff").append($(`<h4>Modified glyphs</h4>`));
      let place = $('<div class="glyphgrid"/>');
      $("#glyphdiff").append(place);

      glyph_diff.forEach((glyph) => {
        addAGlyph(glyph, place);
      });
      $('[data-toggle="tooltip"]').tooltip();
    }
  }

  renderWordDiff(script, diffs) {
    $("#worddiffinner").append($(`<h6>${script}</h6>`));
    let place = $('<div class="wordgrid"/>');
    $("#worddiffinner").append(place);
    diffs.forEach((glyph) => {
      addAWord(glyph, place);
    });
    $('[data-toggle="tooltip"]').tooltip();
  }
}

$(function () {
  window.diffenator = new Diffenator();
  diffWorker.onmessage = (e) => window.diffenator.progress_callback(e.data);
  $("#bigLoadingModal").show();

  $(".fontdrop").on("dragover dragenter", function (e) {
    e.preventDefault();
    e.stopPropagation();
    $(this).addClass("dragging");
  });
  $(".fontdrop").on("dragleave dragend", function (e) {
    $(this).removeClass("dragging");
  });

  $(".fontdrop").on("drop", function (e) {
    $(this).removeClass("dragging");
    if (
      e.originalEvent.dataTransfer &&
      e.originalEvent.dataTransfer.files.length
    ) {
      e.preventDefault();
      e.stopPropagation();
      diffenator.dropFile(e.originalEvent.dataTransfer.files, this);
    }
  });

  setupAnimation();
});
