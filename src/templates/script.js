function diffTables_statichtml() {
  $("#difftable").empty();
  $("#difftable").append(`<h2 class="mt-2">Table Diff</h2>`);
  $("#difftable").append(
    renderTableDiff({ tables: report["tables"] }, true).children()
  );
  $("#difftable .node").on("click", function (e) {
    $(this).toggleClass("closed open");
    $(this).children(".node").toggle();
    e.stopPropagation();
  });
}

function cmapDiff_static_html() {
  if (report.cmap_diff && (report.cmap_diff.new || report.cmap_diff.missing)) {
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

function buildLocation_statichtml(loc) {
	// Set font styles to appropriate axis locations
	let rule = document.styleSheets[0].cssRules[2].style
	let cssSetting = Object.entries(loc.coords).map(function ([axis, value]) {
		return `"${axis}" ${value}`
	}).join(", ");
	let textLocation = Object.entries(loc.coords).map(function ([axis, value]) {
		return `${axis}=${value}`
	}).join(" ");
	rule.setProperty("font-variation-settings", cssSetting)

	$("#main").empty();

	$("#main").append(`<h2 class="mt-2">${loc.location}</h2>`);
	$("#main").append(`<h4>${textLocation}</h2>`);

	if (loc.glyphs) {
		$("#main").append("<h5 class='box-title'>Modified Glyphs</h5>");
		let glyphs = $("<div>");
		for (let glyph of loc.glyphs) {
			addAGlyph(glyph, glyphs);
		}
		$("#main").append(glyphs);
	}

	if (loc.words) {
		$("#main").append("<h5 class='box-title'>Modified Words</h5>");
		for (let [script, words] of Object.entries(loc.words)) {
			let scriptTitle = $(`<h6>${script}</h6>`);
			$("#main").append(scriptTitle);
			let worddiv = $("<div>");
			for (let word of words) {
				addAWord(word, worddiv);
			}
			$("#main").append(worddiv);
		}
	}
	$('[data-toggle="tooltip"]').tooltip()

}

$(function () {
  if (report["tables"]) {
    diffTables_statichtml();
  }
  $("#cmapdiff").append(`<h2 class="mt-2">Added and Removed Encoded Glyphs</h2>`);
  cmapDiff_static_html();
  $('[data-toggle="tooltip"]').tooltip()

  for (var [index, loc] of report["locations"].entries()) {
    var loc_nav = $(`<li class="nav-item">
		<a class="nav-link" href="#" data-index="${index}">${loc.location}</a>
	</li>`);
    $("#locationnav").append(loc_nav);
  }
  $("#locationnav li a").on("click", function (e) {
    buildLocation_statichtml(report.locations[$(this).data("index")]);
  });
  $("#locationnav li a").eq(0).addClass("active");
  $("#locationnav li a").eq(0).click();

  $("#fonttoggle").click(function () {
    if ($(this).text() == "Old") {
      $(this).text("New");
      $(".font-before").removeClass("font-before").addClass("font-after");
    } else {
      $(this).text("Old");
      $(".font-after").removeClass("font-after").addClass("font-before");
    }
  });

  document.styleSheets[0].cssRules[0].style.setProperty(
    "src",
    "url({{ old_filename }})"
  );
  document.styleSheets[0].cssRules[1].style.setProperty(
    "src",
    "url({{ new_filename }})"
  );
});
