function renderTableDiff(node, toplevel) {
  var wrapper = $("<div> </div>");
  if (!node) {
    return wrapper;
  }
  if (Array.isArray(node) && node.length == 2) {
    var before = $("<span/>");
    before.addClass("attr-before");
    before.html(" " + node[0] + " ");
    var after = $("<span/>");
    after.addClass("attr-after");
    after.append(renderTableDiff(node[1], true).children());
    wrapper.append(before);
    wrapper.append(after);
    return wrapper;
  }
  if (node.constructor != Object) {
    var thing = $("<span/>");
    thing.html(node);
    wrapper.append(thing);
    return wrapper;
  }
  for (const [key, value] of Object.entries(node)) {
    var display = $("<div/>");
    display.addClass("node");
    if (!toplevel) {
      display.hide();
    }
    display.append(key);
    display.append(renderTableDiff(value, false).children());
    if (display.children(".node").length > 0) {
      display.addClass("closed");
    }
    wrapper.append(display);
  }
  return wrapper;
}

function addAGlyph(glyph, where) {
  let title = "";
  if (glyph.name) {
    title = "name: " + glyph.name;
  }
  let cp =
    "<br>U+" +
    glyph.string.codePointAt(0).toString(16).padStart(4, "0").toUpperCase();
  where.append(`
        <div class="cell-glyph font-before">
        <div data-toggle="tooltip" data-html="true" data-title="${glyph.differing_pixels} pixels"> ${glyph.string}
        <div class="codepoint" data-toggle="tooltip" data-html="true" data-title="${title}">
		${cp}
        </div>
        </div>
    `);
}

function addAWord(diff, where) {
  if (!diff.buffer_b) {
    diff.buffer_b = diff.buffer_a;
  }
  where.append(`
		<div class="cell-word font-before">
		<span data-toggle="tooltip" data-html="true" data-title="Before: <pre>${diff.buffer_a}</pre>After: <pre>${diff.buffer_b}</pre><br>difference: ${diff.differing_pixels} pixels">
		${diff.word}
		</span>
		</div>
	`);
}

function diffTables(report) {
  $("#difftable").empty();
  $("#difftable").append(`<h4 class="mt-2 box-title">Table-level details</h4>`);
  $("#difftable").append(
    renderTableDiff({ tables: report["tables"] }, true).children()
  );
  $("#difftable .node").on("click", function (e) {
    $(this).toggleClass("closed open");
    $(this).children(".node").toggle();
    e.stopPropagation();
  });
}
function diffKerns(report) {
  $("#diffkerns").empty();
  $("#diffkerns").append(`<h4 class="mt-2">Modified Kerns</h4>`);
  $("#diffkerns").append(
    `<table class="table table-striped" id="diffkerns"><tr><th>Pair</th><th>Old</old><th>New</th></table>`
  );
  for (let [pair, value] of Object.entries(report["kerns"])) {
    if (pair == "error") {
      $("#diffkerns").append(`<p class="text-danger">Error: ${value}</p>`);
      continue;
    } else {
      let row = $("<tr>");
      row.append(`<td>${pair}</td>`);
      row.append(`<td>${serializeKernBefore(value)}</td>`);
      row.append(`<td>${serializeKernAfter(value)}</td>`);
      $("#diffkerns table").append(row);
    }
  }
}

function serializeKernBefore(kern) {
  if (Array.isArray(kern)) {
    return serializeKern(kern[0], -1);
  }
  return serializeKern(kern, 0);
}

function serializeKernAfter(kern) {
  if (Array.isArray(kern)) {
    return serializeKern(kern[1], -1);
  }
  return serializeKern(kern, 1);
}

function serializeKern(kern, index) {
  let string = "";
  if (kern === null || kern === undefined) {
    return "(null)";
  }
  if (kern.x) {
    string += serializeKernValue(kern.x, index);
  } else if (kern.y) {
    string = "0";
  }

  if (kern.y) {
    string += "," + serializeKernValue(kern.y, index);
  }
  if (!kern.x_placement && !kern.y_placement) {
    return string;
  }
  string += "@";
  if (kern.x_placement) {
    string += serializeKernValue(kern.x_placement, index);
  } else if (kern.y_placement) {
    string += "0";
  }
  if (kern.y_placement) {
    string += "," + serializeKernValue(kern.y_placement, index);
  }
  return string;
}

function serializeKernValue(kern, index) {
  if (typeof kern == "number") {
    return kern;
  }
  let string = "(";
  let verybig = Object.entries(kern).length > 5;
  for (let [key, value] of Object.entries(kern)) {
    if (key == "default") {
      string += value[index] + " ";
    } else {
      string += value[index] + "@" + key + " ";
    }
    if (verybig) {
      string += "<br>";
    }
  }
  return string.trim() + ")";
}

function cmapDiff(report) {
  if (report.cmap_diff && (report.cmap_diff.new || report.cmap_diff.missing)) {
    $("#cmapdiff").append(
      `<h4 class="mt-2">Added and Removed Encoded Glyphs</h4>`
    );
    if (report["cmap_diff"]["new"]) {
      $("#cmapdiff").append(`<h4 class="box-title">Added Glyphs</h4>`);
      let added = $("<div>");
      for (let glyph of report["cmap_diff"]["new"]) {
        addAGlyph(glyph, added);
      }
      $("#cmapdiff").append(added);
    }

    if (report["cmap_diff"]["missing"]) {
      $("#cmapdiff").append(`<h4 class="box-title">Removed Glyphs</h4>`);
      let missing = $("<div>");
      for (let glyph of report["cmap_diff"]["missing"]) {
        addAGlyph(glyph, missing);
      }
      $("#cmapdiff").append(missing);
    }
  } else {
    $("#cmapdiff").append(`<p>No changes to encoded glyphs</p>`);
  }
}

function setupAnimation() {
  $("#fonttoggle").click(function () {
    if ($(this).text() == "Old") {
      $(this).text("New");
      $(".font-before").removeClass("font-before").addClass("font-after");
    } else {
      $(this).text("Old");
      $(".font-after").removeClass("font-after").addClass("font-before");
    }
  });

  let animationHandle;
  function animate() {
    $("#fonttoggle").click();
    animationHandle = setTimeout(animate, 1000);
  }
  $("#fontanimate").click(function () {
    if ($(this).text() == "Animate") {
      $(this).text("Stop");
      animate();
    } else {
      $(this).text("Animate");
      clearTimeout(animationHandle);
    }
  });
}

export {
  renderTableDiff,
  addAGlyph,
  addAWord,
  cmapDiff,
  diffTables,
  diffKerns,
  setupAnimation,
};
