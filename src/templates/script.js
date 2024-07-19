function diffTables_statichtml() {
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

function cmapDiff_static_html() {
  if (report.cmap_diff && (report.cmap_diff.new || report.cmap_diff.missing)) {
	$("#cmapdiff").append(`<h4 class="mt-2">Added and Removed Encoded Glyphs</h4>`);
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
	let cssSetting = "";
	let textLocation = "Default";
	if (loc.coords) {
		cssSetting = Object.entries(loc.coords).map(function ([axis, value]) {
			return `"${axis}" ${value}`
		}).join(", ");
		textLocation = Object.entries(loc.coords).map(function ([axis, value]) {
			return `${axis}=${value}`
		}).join(" ");
		rule.setProperty("font-variation-settings", cssSetting)
	}

	$("#main").empty();

	$("#title").html(`<h4 class="mt-2">${textLocation}</h2>`);

	if (loc.glyphs) {
		$("#main").append("<h4>Modified Glyphs</h4>");
		let glyphs = $("<div>");
		for (let glyph of loc.glyphs) {
			addAGlyph(glyph, glyphs);
		}
		$("#main").append(glyphs);
	}

	if (loc.words) {
		$("#main").append("<h4>Modified Words</h4>");
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
  cmapDiff_static_html();
  $('[data-toggle="tooltip"]').tooltip()

  for (var [index, loc] of report["locations"].entries()) {
    var loc_nav = $(`<li class="nav-item">
		<a class="nav-link text-secondary" href="#" data-index="${index}">${loc.location.replaceAll(',', ',\u200b')}</a>
	</li>`);
    $("#locationnav").append(loc_nav);
  }
  $("#locationnav li a").on("click", function (e) {
	$("#locationnav li a").removeClass("active");
	$(this).addClass("active");
    buildLocation_statichtml(report.locations[$(this).data("index")]);
  });
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

  let animationHandle;
  function animate () {
	$("#fonttoggle").click()
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
});
