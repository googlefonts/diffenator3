/******/ (() => { // webpackBootstrap
/******/ 	"use strict";
/******/ 	var __webpack_modules__ = ({

/***/ "./ts/shared.ts":
/*!**********************!*\
  !*** ./ts/shared.ts ***!
  \**********************/
/***/ ((__unused_webpack_module, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   addAGlyph: () => (/* binding */ addAGlyph),
/* harmony export */   addAWord: () => (/* binding */ addAWord),
/* harmony export */   cmapDiff: () => (/* binding */ cmapDiff),
/* harmony export */   diffFeatures: () => (/* binding */ diffFeatures),
/* harmony export */   diffKerns: () => (/* binding */ diffKerns),
/* harmony export */   diffLanguages: () => (/* binding */ diffLanguages),
/* harmony export */   diffSignificantTables: () => (/* binding */ diffSignificantTables),
/* harmony export */   diffTables: () => (/* binding */ diffTables),
/* harmony export */   renderTableDiff: () => (/* binding */ renderTableDiff),
/* harmony export */   setupAnimation: () => (/* binding */ setupAnimation)
/* harmony export */ });
/* harmony import */ var _types__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(/*! ./types */ "./ts/types.ts");

function toTitleCase(str) {
    return str.replace(/\w\S*/g, (text) => text.charAt(0).toUpperCase() + text.substring(1).toLowerCase());
}
function renderTableDiff(node, toplevel) {
    var wrapper = $("<div> </div>");
    if (!node) {
        return wrapper;
    }
    if ((0,_types__WEBPACK_IMPORTED_MODULE_0__.isSimpleDiff)(node)) {
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
    if ((0,_types__WEBPACK_IMPORTED_MODULE_0__.isValue)(node)) {
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
    let cp = "<br>U+" +
        glyph.string.codePointAt(0).toString(16).padStart(4, "0").toUpperCase();
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
    $("#difftable").append(renderTableDiff({ tables: report.tables }, true).children());
    $("#difftable .node").on("click", function (e) {
        $(this).toggleClass("closed open");
        $(this).children(".node").toggle();
        e.stopPropagation();
    });
}
function diffFeatures(report) {
    $("#difffeatures").empty();
    let tables = report.tables;
    if (!tables) {
        $("#difffeatures").append(`<p>No changes to features</p>`);
        return;
    }
    const isAllNull = (arr) => arr.every((v) => v === null || v === undefined);
    let changes = {};
    for (var table of ["GPOS", "GSUB"]) {
        let layout_table = tables[table];
        if (table in tables && "feature_list" in layout_table) {
            let features = layout_table.feature_list;
            for (var [feature_name, lookups] of Object.entries(features)) {
                if ((0,_types__WEBPACK_IMPORTED_MODULE_0__.isSimpleDiff)(lookups)) {
                    lookups = { 0: lookups };
                }
                let lookupsNew = lookups;
                let left_lookups = Object.values(lookupsNew).map((l) => l && l[0]);
                let right_lookups = Object.values(lookupsNew).map((l) => l && l[1]);
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
    $("#difffeatures").append(`<h3 class="border-top pt-2 border-dark-subtle">Modified Features</h3>`);
    if (Object.keys(changes).length == 0) {
        $("#difffeatures").append(`<p>No changes to features</p>`);
        return;
    }
    $("#difffeatures").append(`<table class="table table-striped" id="difffeatures"><tr><th>Feature</th><th>Status</th></table>`);
    for (let [feature, status] of Object.entries(changes)) {
        let row = $("<tr>");
        row.append(`<td>${feature}</td>`);
        row.append(`<td>${status}</td>`);
        $("#difffeatures table").append(row);
    }
}
function diffLanguages(report) {
    $("#difflanguages").empty();
    $("#difflanguages").append(`<h3 class="border-top pt-2 border-dark-subtle">Modified Languages</h3>`);
    let notSame = Object.entries(report).filter(([name, diff]) => diff.score_a !== diff.score_b || diff.level_a !== diff.level_b);
    if (notSame.length === 0) {
        $("#difflanguages").append(`<p>No changes to languages</p>`);
        return;
    }
    $("#difflanguages").append(`<table class="table table-striped" id="difflanguages"><tr><th>Language</th><th>Old</th><th>New</th></tr></table>`);
    for (let [name, diff] of notSame) {
        let row = $("<tr>");
        row.append(`<td>${name}</td>`);
        row.append(`<td>${diff.level_a} (${diff.score_a}%)</td>`);
        row.append(`<td>${diff.level_b} (${diff.score_b}%)</td>`);
        $("#difflanguages table").append(row);
    }
}
function diffSignificantTables(report) {
    // Pull things we care about out of the table diff
    let tables = report.tables;
    if (!tables) {
        return;
    }
    let result = $("<tbody/>");
    let table = $("<table/>").addClass("table table-striped");
    table.append(result);
    if ("name" in tables) {
        let name_table = tables["name"];
        result.append(`<tr><th colspan="3">Name Table Changes</th></tr>`);
        for (let [name_id, thing] of Object.entries(name_table)) {
            name_id = toTitleCase(name_id.replaceAll("_", " "));
            if (name_id.startsWith("Nameid ")) {
                continue;
            }
            for (let [language, entry] of Object.entries(thing)) {
                if (!(0,_types__WEBPACK_IMPORTED_MODULE_0__.isSimpleDiff)(entry)) {
                    let element = renderTableDiff(entry, true);
                    let row = $("<tr/>");
                    row.append(`<td>${name_id} (${language})</td>`);
                    let after_td = $("<td/>");
                    after_td.append(element.children());
                    row.append(after_td);
                    table.children().first().append(row);
                    continue;
                }
                let [before, after] = entry;
                let row = $("<tr/>");
                row.append(`<td>${name_id} (${language})</td>`);
                let before_td = $("<td/>").addClass("attr-before").text(before);
                let after_td = $("<td/>").addClass("attr-after").text(after);
                row.append(before_td);
                row.append(after_td);
                table.children().first().append(row);
            }
        }
    }
    // Vertical metrics
    let vmetricsRows = [];
    const vMetricsFields = [
        ["hhea", "ascent"],
        ["hhea", "descent"],
        ["hhea", "line_gap"],
        ["OS/2", "s_typo_ascender"],
        ["OS/2", "s_typo_descender"],
        ["OS/2", "s_typo_line_gap"],
        ["OS/2", "us_win_ascent"],
        ["OS/2", "us_win_descent"],
    ];
    for (let [table_name, field_name] of vMetricsFields) {
        if (table_name in tables) {
            let table = tables[table_name];
            if (field_name in table) {
                let entry = table[field_name];
                if ((0,_types__WEBPACK_IMPORTED_MODULE_0__.isSimpleDiff)(entry)) {
                    let [before, after] = entry;
                    let row = $("<tr/>");
                    row.append(`<td>${table_name}.${toTitleCase(field_name.replaceAll("_", " ")).replaceAll(" ", "")}</td>`);
                    let before_td = $("<td/>").addClass("attr-before").text(before);
                    let after_td = $("<td/>").addClass("attr-after").text(after);
                    row.append(before_td);
                    row.append(after_td);
                    vmetricsRows.push(row);
                }
            }
        }
    }
    if (vmetricsRows.length > 0) {
        result.append(`<tr><th colspan="3">Vertical Metrics Changes</th></tr>`);
        result.append(vmetricsRows);
    }
    // fvar, avar
    if ("fvar" in tables) {
        result.append(`<tr><th colspan="3">fvar Table Changes</th></tr>`);
        result.append(`<tr><td colspan="3">The fvar table has changed. See the full table diff below.</td></tr>`);
    }
    if ("avar" in tables) {
        result.append(`<tr><th colspan="3">avar Table Changes</th></tr>`);
        result.append(`<tr><td colspan="3">The avar table has changed. See the full table diff below.</td></tr>`);
    }
    // Other stuff.
    let lookIn = [
        ["head", "units_per_em"],
        ["head", "font_revision"],
        ["head", "flags"],
        ["head", "mac_style"],
        ["os/2", "fs_type"],
        ["os/2", "fs_selection"],
    ];
    let otherChanges = [];
    for (let [table_name, field_name] of lookIn) {
        if (table_name in tables) {
            let table = tables[table_name];
            if (field_name in table) {
                let entry = table[field_name];
                if ((0,_types__WEBPACK_IMPORTED_MODULE_0__.isSimpleDiff)(entry)) {
                    let [before, after] = entry;
                    let row = $("<tr/>");
                    row.append(`<td>${table_name}.${toTitleCase(field_name.replaceAll("_", " ")).replaceAll(" ", "")}</td>`);
                    if (field_name == "font_revision") {
                        before = parseFloat(before).toFixed(3);
                        after = parseFloat(after).toFixed(3);
                    }
                    else if (field_name == "flags" ||
                        field_name == "mac_style" ||
                        field_name == "fs_type" ||
                        field_name == "fs_selection") {
                        before = "0b" + before.toString(2).toUpperCase();
                        after = "0b" + after.toString(2).toUpperCase();
                    }
                    let before_td = $("<td/>").addClass("attr-before").text(before);
                    let after_td = $("<td/>").addClass("attr-after").text(after);
                    row.append(before_td);
                    row.append(after_td);
                    otherChanges.push(row);
                }
            }
        }
    }
    if (otherChanges.length > 0) {
        result.append(`<tr><th colspan="3">Other Significant Changes</th></tr>`);
        result.append(otherChanges);
    }
    $("#diffsignificanttables").empty();
    $("#diffsignificanttables").append(`<h3 class="border-top pt-2 border-dark-subtle">Significant Table Changes</h3>`);
    if (result.children().length == 0) {
        $("#diffsignificanttables").append(`<tr><th>No significant table changes</th></tr>`);
    }
    else {
        $("#diffsignificanttables").append(table);
    }
}
function diffKerns(report) {
    $("#diffkerns").empty();
    if (!report["kerns"] || Object.keys(report["kerns"]).length == 0) {
        $("#diffkerns").append(`<p>No changes to kerning</p>`);
        return;
    }
    $("#diffkerns").append(`<h3 class="border-top pt-2 border-dark-subtle">Modified Kerns</h3>`);
    $("#diffkerns").append(`<table class="table table-striped" id="diffkerns"><tr><th>Pair</th><th>Old</th><th>New</th></tr></table>`);
    for (let [pair, value] of Object.entries(report["kerns"])) {
        if (pair == "error") {
            $("#diffkerns").append(`<p class="text-danger">Error: ${value}</p>`);
            continue;
        }
        else {
            let row = $("<tr>");
            row.append(`<td>${pair}</td>`);
            row.append(`<td>${serializeKernBefore(value)}</td>`);
            row.append(`<td>${serializeKernAfter(value)}</td>`);
            $("#diffkerns table").append(row);
        }
    }
}
function serializeKernBefore(kern) {
    if ((0,_types__WEBPACK_IMPORTED_MODULE_0__.isSimpleDiff)(kern)) {
        return serializeKern(kern[0], -1);
    }
    return serializeKern(kern, 0);
}
function serializeKernAfter(kern) {
    if ((0,_types__WEBPACK_IMPORTED_MODULE_0__.isSimpleDiff)(kern)) {
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
    }
    else if (kern.y) {
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
    }
    else if (kern.y_placement) {
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
        }
        else {
            string += value[index] + "@" + key + " ";
        }
        if (verybig) {
            string += "<br>";
        }
    }
    return string.trim() + ")";
}
function cmapDiff(cmap_diff) {
    $("#cmapdiff").empty();
    $("#cmapdiff").append(`<h3 class="border-top pt-2 border-dark-subtle">Added and Removed Encoded Glyphs</h3>`);
    if (cmap_diff && (cmap_diff.new || cmap_diff.missing)) {
        if (cmap_diff.new) {
            $("#cmapdiff").append(`<h4>Added Glyphs</h4>`);
            let added = $("<div>");
            for (let glyph of cmap_diff.new) {
                addAGlyph(glyph, added);
            }
            $("#cmapdiff").append(added);
        }
        if (cmap_diff.missing) {
            $("#cmapdiff").append(`<h4>Removed Glyphs</h4>`);
            let missing = $("<div>");
            for (let glyph of cmap_diff.missing) {
                addAGlyph(glyph, missing);
            }
            $("#cmapdiff").append(missing);
        }
    }
    else {
        $("#cmapdiff").append(`<p>No changes to encoded glyphs</p>`);
    }
}
function setupAnimation() {
    $("#fonttoggle").click(function () {
        if ($(this).text() == "Old") {
            $(this).text("New");
            $(".font-before").removeClass("font-before").addClass("font-after");
        }
        else {
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
        }
        else {
            $(this).text("Animate");
            clearTimeout(animationHandle);
        }
    });
}



/***/ }),

/***/ "./ts/types.ts":
/*!*********************!*\
  !*** ./ts/types.ts ***!
  \*********************/
/***/ ((__unused_webpack_module, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   isArrayDiff: () => (/* binding */ isArrayDiff),
/* harmony export */   isSimpleDiff: () => (/* binding */ isSimpleDiff),
/* harmony export */   isValue: () => (/* binding */ isValue)
/* harmony export */ });
function isValue(node) {
    return node?.constructor != Object;
}
function isSimpleDiff(node) {
    return Array.isArray(node) && node.length == 2;
}
function isArrayDiff(node) {
    return (node?.constructor == Object &&
        Object.keys(node).every((k) => !isNaN(parseInt(k, 10))));
}


/***/ })

/******/ 	});
/************************************************************************/
/******/ 	// The module cache
/******/ 	var __webpack_module_cache__ = {};
/******/ 	
/******/ 	// The require function
/******/ 	function __webpack_require__(moduleId) {
/******/ 		// Check if module is in cache
/******/ 		var cachedModule = __webpack_module_cache__[moduleId];
/******/ 		if (cachedModule !== undefined) {
/******/ 			return cachedModule.exports;
/******/ 		}
/******/ 		// Create a new module (and put it into the cache)
/******/ 		var module = __webpack_module_cache__[moduleId] = {
/******/ 			// no module.id needed
/******/ 			// no module.loaded needed
/******/ 			exports: {}
/******/ 		};
/******/ 	
/******/ 		// Execute the module function
/******/ 		__webpack_modules__[moduleId](module, module.exports, __webpack_require__);
/******/ 	
/******/ 		// Return the exports of the module
/******/ 		return module.exports;
/******/ 	}
/******/ 	
/************************************************************************/
/******/ 	/* webpack/runtime/define property getters */
/******/ 	(() => {
/******/ 		// define getter functions for harmony exports
/******/ 		__webpack_require__.d = (exports, definition) => {
/******/ 			for(var key in definition) {
/******/ 				if(__webpack_require__.o(definition, key) && !__webpack_require__.o(exports, key)) {
/******/ 					Object.defineProperty(exports, key, { enumerable: true, get: definition[key] });
/******/ 				}
/******/ 			}
/******/ 		};
/******/ 	})();
/******/ 	
/******/ 	/* webpack/runtime/hasOwnProperty shorthand */
/******/ 	(() => {
/******/ 		__webpack_require__.o = (obj, prop) => (Object.prototype.hasOwnProperty.call(obj, prop))
/******/ 	})();
/******/ 	
/******/ 	/* webpack/runtime/make namespace object */
/******/ 	(() => {
/******/ 		// define __esModule on exports
/******/ 		__webpack_require__.r = (exports) => {
/******/ 			if(typeof Symbol !== 'undefined' && Symbol.toStringTag) {
/******/ 				Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });
/******/ 			}
/******/ 			Object.defineProperty(exports, '__esModule', { value: true });
/******/ 		};
/******/ 	})();
/******/ 	
/************************************************************************/
var __webpack_exports__ = {};
// This entry needs to be wrapped in an IIFE because it needs to be isolated against other modules in the chunk.
(() => {
/*!****************************!*\
  !*** ./ts/d3web-static.ts ***!
  \****************************/
__webpack_require__.r(__webpack_exports__);
/* harmony import */ var _shared__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(/*! ./shared */ "./ts/shared.ts");

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
    $("#title").html(`<h2 class="mt-2">${textLocation}</h2>`);
    if (loc.glyphs) {
        loc.glyphs.sort((ga, gb) => new Intl.Collator().compare(ga.string, gb.string));
        $("#main").append("<h3 class='border-top pt-2 border-dark-subtle'>Modified Glyphs</h3>");
        let glyphs = $("<div>");
        for (let glyph of loc.glyphs) {
            (0,_shared__WEBPACK_IMPORTED_MODULE_0__.addAGlyph)(glyph, glyphs);
        }
        $("#main").append(glyphs);
    }
    if (loc.words) {
        $("#main").append("<h3 class='border-top pt-2 border-dark-subtle'>Modified Words</h3>");
        for (let [script, words] of Object.entries(loc.words)) {
            let scriptTitle = $(`<h6>${script}</h6>`);
            $("#main").append(scriptTitle);
            let worddiv = $("<div>");
            for (let word of words) {
                (0,_shared__WEBPACK_IMPORTED_MODULE_0__.addAWord)(word, worddiv);
            }
            $("#main").append(worddiv);
        }
    }
    $('[data-toggle="tooltip"]').tooltip();
}
$(function () {
    if (report["tables"]) {
        (0,_shared__WEBPACK_IMPORTED_MODULE_0__.diffTables)(report);
        (0,_shared__WEBPACK_IMPORTED_MODULE_0__.diffSignificantTables)(report);
        (0,_shared__WEBPACK_IMPORTED_MODULE_0__.diffFeatures)(report);
    }
    if (report["kerns"]) {
        (0,_shared__WEBPACK_IMPORTED_MODULE_0__.diffKerns)(report);
    }
    if (report["languages"]) {
        (0,_shared__WEBPACK_IMPORTED_MODULE_0__.diffLanguages)(report["languages"]);
    }
    (0,_shared__WEBPACK_IMPORTED_MODULE_0__.cmapDiff)(report.cmap_diff);
    $('[data-toggle="tooltip"]').tooltip();
    if (!report["locations"] &&
        !report["cmap_diff"] &&
        !report["tables"] &&
        !report["kerns"]) {
        $("#title").html("<h3>No differences found</h3>");
        return;
    }
    if (report["locations"]) {
        for (var [index, loc] of report["locations"].entries()) {
            var loc_nav = $(`<li class="nav-item">
		<a class="nav-link text-secondary" href="#" data-index="${index}">${loc.location.replaceAll(",", ",\u200b")}</a>
	</li>`);
            $("#locationnav").append(loc_nav);
        }
        $("#locationnav li a").on("click", function (e) {
            $("#locationnav li a").removeClass("active");
            $(this).addClass("active");
            buildLocation_statichtml(report.locations[$(this).data("index")]);
        });
        $("#locationnav li a").eq(0).click();
    }
    document.styleSheets[0].cssRules[0].style.setProperty("src", "url({{ old_filename }})");
    document.styleSheets[0].cssRules[1].style.setProperty("src", "url({{ new_filename }})");
    (0,_shared__WEBPACK_IMPORTED_MODULE_0__.setupAnimation)();
});

})();

/******/ })()
;
//# sourceMappingURL=script.js.map