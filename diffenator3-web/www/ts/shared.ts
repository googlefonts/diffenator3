import {
  isSimpleDiff,
  isValue,
  type CmapDiff,
  type EncodedGlyph,
  type Diff,
  type Difference,
  type GlyphDiff,
  type ObjectDiff,
  type Report,
  type SimpleDiff,
  type Value,
  type ValueRecord,
  type LanguageDiff,
} from "./types";

function renderTableDiff(
  node: Diff | Value | Record<string, Diff>,
  toplevel: boolean
) {
  var wrapper = $("<div> </div>");
  if (!node) {
    return wrapper;
  }
  if (isSimpleDiff(node)) {
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
  if (isValue(node)) {
    var thing = $("<span/>");
    thing.html(node as string);
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

function addAGlyph(
  glyph: GlyphDiff | EncodedGlyph,
  where: JQuery<HTMLElement>
) {
  let title = "";
  if (glyph.name) {
    title = "name: " + glyph.name;
  }
  let cp =
    "<br>U+" +
    glyph.string.codePointAt(0)!.toString(16).padStart(4, "0").toUpperCase();
  let pixeldiff_title = "";
  if ("differing_pixels" in glyph) {
    pixeldiff_title = `${glyph.differing_pixels} pixels`;
  }
  where.append(`
        <div class="cell-glyph font-before">
        <div data-bs-toggle="tooltip" data-bs-html="true" title="${pixeldiff_title}"> ${glyph.string}
        <div class="codepoint" data-bs-toggle="tooltip" data-bs-html="true" title="${title}">
        ${cp}
        </div>
        </div>
    `);
}

function addAWord(diff: Difference, where: JQuery<HTMLElement>) {
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

function diffTables(report: Report) {
  $("#difftable").empty();
  $("#difftable").append(`<h4 class="mt-2 box-title">Table-level details</h4>`);
  $("#difftable").append(
    renderTableDiff({ tables: report.tables as Diff }, true).children()
  );
  $("#difftable .node").on("click", function (e) {
    $(this).toggleClass("closed open");
    $(this).children(".node").toggle();
    e.stopPropagation();
  });
}

function diffFeatures(report: Report) {
  $("#difffeatures").empty();
  let tables = report.tables as Record<string, Diff>;
  if (!tables) {
    $("#difffeatures").append(`<p>No changes to features</p>`);
    return;
  }
  const isAllNull = <T>(arr: T[]) =>
    arr.every((v) => v === null || v === undefined);
  let changes: Record<string, string> = {};
  for (var table of ["GPOS", "GSUB"]) {
    let layout_table = tables[table]! as ObjectDiff;
    if (table in tables && "feature_list" in layout_table) {
      let features = layout_table.feature_list as ObjectDiff;
      for (var [feature_name, lookups] of Object.entries(features)) {
        if (isSimpleDiff(lookups)) {
          lookups = { 0: lookups };
        }
        let lookupsNew = lookups as Record<number, SimpleDiff>;
        let left_lookups: Value[] = Object.values(lookupsNew).map(
          (l: SimpleDiff) => l && l[0]
        );
        let right_lookups: Value[] = Object.values(lookupsNew).map(
          (l: SimpleDiff) => l && l[1]
        );
        console.log(table, feature_name, left_lookups, right_lookups);
        let status = isAllNull(left_lookups)
          ? "added"
          : isAllNull(right_lookups)
          ? "removed"
          : left_lookups.length != right_lookups.length
          ? `modified (${left_lookups.length} → ${right_lookups.length})`
          : "modified";
        changes[`${table} ${feature_name}`] = status;
      }
    }
  }
  $("#difffeatures").append(
    `<h4 class="mt-2 box-title">Modified Features</h4>`
  );
  if (Object.keys(changes).length == 0) {
    $("#difffeatures").append(`<p>No changes to features</p>`);
    return;
  }
  $("#difffeatures").append(
    `<table class="table table-striped" id="difffeatures"><tr><th>Feature</th><th>Status</th></table>`
  );
  for (let [feature, status] of Object.entries(changes)) {
    let row = $("<tr>");
    row.append(`<td>${feature}</td>`);
    row.append(`<td>${status}</td>`);
    $("#difffeatures table").append(row);
  }
}

function diffLanguages(report: Record<string, LanguageDiff>) {
  $("#difflanguages").empty();
  $("#difflanguages").append(`<h4>Modified Languages</h4>`);
  let notSame = Object.entries(report).filter(
    ([name, diff]) =>
      diff.score_a !== diff.score_b || diff.level_a !== diff.level_b
  );
  if (notSame.length === 0) {
    $("#difflanguages").append(`<p>No changes to languages</p>`);
    return;
  }
  $("#difflanguages").append(
    `<table class="table table-striped" id="difflanguages"><tr><th>Language</th><th>Old</th><th>New</th></tr></table>`
  );
  for (let [name, diff] of notSame) {
    let row = $("<tr>");
    row.append(`<td>${name}</td>`);
    row.append(`<td>${diff.level_a} (${diff.score_a}%)</td>`);
    row.append(`<td>${diff.level_b} (${diff.score_b}%)</td>`);
    $("#difflanguages table").append(row);
  }
}

function diffKerns(report: Report) {
  $("#diffkerns").empty();
  if (!report["kerns"] || Object.keys(report["kerns"]).length == 0) {
    $("#diffkerns").append(`<p>No changes to kerning</p>`);
    return;
  }
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

function serializeKernBefore(kern: Diff) {
  if (isSimpleDiff(kern)) {
    return serializeKern(kern[0] as ValueRecord, -1);
  }
  return serializeKern(kern as ValueRecord, 0);
}

function serializeKernAfter(kern: Diff) {
  if (isSimpleDiff(kern)) {
    return serializeKern(kern[1] as ValueRecord, -1);
  }
  return serializeKern(kern as ValueRecord, 1);
}

function serializeKern(kern: ValueRecord, index: number) {
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

function serializeKernValue(
  kern: Value | Record<string, number>,
  index: number
) {
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

function cmapDiff(cmap_diff: CmapDiff | undefined) {
  if (cmap_diff && (cmap_diff.new || cmap_diff.missing)) {
    $("#cmapdiff").append(
      `<h4 class="mt-2">Added and Removed Encoded Glyphs</h4>`
    );
    if (cmap_diff.new) {
      $("#cmapdiff").append(`<h4 class="box-title">Added Glyphs</h4>`);
      let added = $("<div>");
      for (let glyph of cmap_diff.new) {
        addAGlyph(glyph, added);
      }
      $("#cmapdiff").append(added);
    }

    if (cmap_diff.missing) {
      $("#cmapdiff").append(`<h4 class="box-title">Removed Glyphs</h4>`);
      let missing = $("<div>");
      for (let glyph of cmap_diff.missing) {
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

  let animationHandle: number;
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
  diffFeatures,
  diffLanguages,
  setupAnimation,
};
