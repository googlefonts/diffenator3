function buildLocation_statichtml(loc) {
  // Set font styles to appropriate axis locations
  let rule = document.styleSheets[0].cssRules[2].style;
  let cssSetting = "";
  let textLocation = "Default";
  if (loc.coords) {
    cssSetting = Object.entries(loc.coords)
      .map(function ([axis, value]) {
        return `"${axis}" ${value}`;
      })
      .join(", ");
    textLocation = Object.entries(loc.coords)
      .map(function ([axis, value]) {
        return `${axis}=${value}`;
      })
      .join(" ");
    rule.setProperty("font-variation-settings", cssSetting);
  }

  $("#main").empty();

  $("#title").html(`<h4 class="mt-2">${textLocation}</h2>`);

  if (loc.glyphs) {
    loc.glyphs.sort((ga, gb) =>
      new Intl.Collator().compare(ga.string, gb.string)
    );
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
  $('[data-toggle="tooltip"]').tooltip();
}

$(function () {
  if (report["tables"]) {
    diffTables(report);
    diffFeatures(report);
  }
  if (report["kerns"]) {
    diffKerns(report);
  }
  cmapDiff(report);
  $('[data-toggle="tooltip"]').tooltip();
  if (!report["locations"]) {
    $("#title").html("<h4 class='mt-2'>No differences found</h4>");
    $("#ui-nav").hide();
    return;
  }

  for (var [index, loc] of report["locations"].entries()) {
    var loc_nav = $(`<li class="nav-item">
		<a class="nav-link text-secondary" href="#" data-index="${index}">${loc.location.replaceAll(
      ",",
      ",\u200b"
    )}</a>
	</li>`);
    $("#locationnav").append(loc_nav);
  }
  $("#locationnav li a").on("click", function (e) {
    $("#locationnav li a").removeClass("active");
    $(this).addClass("active");
    buildLocation_statichtml(report.locations[$(this).data("index")]);
  });
  $("#locationnav li a").eq(0).click();

  document.styleSheets[0].cssRules[0].style.setProperty(
    "src",
    "url({{ old_filename }})"
  );
  document.styleSheets[0].cssRules[1].style.setProperty(
    "src",
    "url({{ new_filename }})"
  );
  setupAnimation();
});
