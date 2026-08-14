import type { LocationResult, Report } from "./types";
import {
  renderTableDiff,
  renderLocationDiff,
  setVariationStyle,
  locationLabel,
  cmapDiff,
  diffTables,
  diffFeatures,
  diffSignificantTables,
  setupAnimation,
  diffLanguages,
} from "./shared";

declare var report: Report;

function buildLocation_statichtml(loc: LocationResult) {
  setVariationStyle(loc.coords);
  $("#title").html(`<h2 class="mt-2">${locationLabel(loc.coords)}</h2>`);
  renderLocationDiff(loc, $("#main"));
  $('[data-toggle="tooltip"]').tooltip();
}

$(function () {
  if (report["tables"]) {
    diffTables(report);
    diffSignificantTables(report);
    diffFeatures(report);
  }
  if (report["languages"]) {
    diffLanguages(report["languages"]);
  }
  cmapDiff(report.cmap_diff);
  $('[data-toggle="tooltip"]').tooltip();
  if (
    !report["locations"] &&
    !report["cmap_diff"] &&
    !report["tables"]
  ) {
    $("#title").html("<h3>No differences found</h3>");
    return;
  }

  if (report["locations"]) {
    for (var [index, loc] of report["locations"].entries()) {
      var loc_nav = $(`<li class="nav-item">
		<a class="nav-link text-secondary" href="#" data-index="${index}">${loc.location.replaceAll(
      ",",
      ",\u200b",
    )}</a>
	</li>`);
      $("#locationnav").append(loc_nav);
    }
    $("#locationnav li a").on("click", function (e) {
      $("#locationnav li a").removeClass("active");
      $(this).addClass("active");
      buildLocation_statichtml(report.locations![$(this).data("index")]!);
    });
    $("#locationnav li a").eq(0).click();
  }

  (document.styleSheets[0]!.cssRules[0]! as CSSStyleRule).style.setProperty(
    "src",
    "url({{ old_filename }})",
  );
  (document.styleSheets[0]!.cssRules[1]! as CSSStyleRule).style.setProperty(
    "src",
    "url({{ new_filename }})",
  );
  setupAnimation();
});
