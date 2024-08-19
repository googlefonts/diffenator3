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
    glyph.string.charCodeAt(0).toString(16).padStart(4, "0").toUpperCase();
  where.append(`
        <div class="cell-glyph font-before">
        ${glyph.string}
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
		<span data-toggle="tooltip" data-html="true" data-title="Before: <pre>${
      diff.buffer_a
    }</pre>After: <pre>${diff.buffer_b}</pre><br>difference: ${
    Math.round(diff.percent * 100) / 100
  }%">
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
  setupAnimation,
};
