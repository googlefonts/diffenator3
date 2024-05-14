import Worker from "worker-loader!./webworker.js";
const diffWorker = new Worker();

jQuery.fn.shake = function (interval, distance, times) {
	interval = typeof interval == "undefined" ? 100 : interval;
	distance = typeof distance == "undefined" ? 10 : distance;
	times = typeof times == "undefined" ? 3 : times;
	var jTarget = $(this);
	jTarget.css('position', 'relative');
	for (var iter = 0; iter < (times + 1); iter++) {
		jTarget.animate({
			left: ((iter % 2 == 0 ? distance : distance * -1))
		}, interval);
	}
	return jTarget.animate({
		left: 0
	}, interval);
}

class Diffenator {
	constructor() {
		this.beforeFont = null;
		this.afterFont = null;
	}

	get beforeCssStyle() {
		return document.styleSheets[0].cssRules[0].style
	}
	get afterCssStyle() {
		return document.styleSheets[0].cssRules[1].style
	}

	setVariationStyle(variations) {
		let rule = document.styleSheets[0].cssRules[2].style
		rule.setProperty("font-variation-settings", variations)
	}


	dropFile(files, element) {
		if (!files[0].name.match(/\.[ot]tf$/i)) {
			$(element).shake()
			return;
		}
		var style;
		if (element.id == "fontbefore") {
			style = this.beforeCssStyle;
			$(element).find("h2").addClass("font-before")
		} else {
			style = this.afterCssStyle;
			$(element).find("h2").addClass("font-after")
		}
		window.thing = files[0]
		$(element).find("h2").text(files[0].name);
		style.setProperty("src", "url(" + URL.createObjectURL(files[0]) + ")")
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


	renderTableDiff(node, toplevel) {
		var wrapper = $("<div> </div>");
		if (!node) {
			return wrapper
		}
		if (Array.isArray(node)) {
			var before = $("<span/>");
			before.addClass("attr-before");
			before.html(" " + node[0] + " ");
			var after = $("<span/>");
			after.addClass("attr-after");
			after.append(this.renderTableDiff(node[1], true).children());
			wrapper.append(before);
			wrapper.append(after);
			return wrapper
		}
		if (node.constructor != Object) {
			var thing = $("<span/>");
			thing.html(node);
			wrapper.append(thing);
			return wrapper
		}
		for (const [key, value] of Object.entries(node)) {
			var display = $("<div/>");
			display.addClass("node")
			if (!toplevel) {
				display.hide()
			}
			display.append(key);
			display.append(this.renderTableDiff(value, false).children());
			wrapper.append(display)
		}
		return wrapper

	}

	setVariations() {
		let cssSetting = $("#axes input").map(function () {
			return `"${this.id.replace("axis-", "")}" ${this.value}`
		}).get().join(", ");
		this.setVariationStyle(cssSetting);
		this.updateGlyphs();
	}

	setupAxes(message) {
		$("#axes").empty();
		console.log(message)
		let {axes, instances} = message;
		for (var [tag, limits] of Object.entries(axes)) {
			console.log(tag,limits)
			let [axis_min, axis_def, axis_max] = limits;
			let axis = $(`<div class="axis">
				${tag}
				<input type="range" min="${axis_min}" max="${axis_max}" value="${axis_def}" class="slider" id="axis-${tag}">
			`);
			$("#axes").append(axis);
			axis.on("input", this.setVariations.bind(this))
			axis.on("change", this.updateWords.bind(this))
		}
		if (Object.keys(instances).length > 0) {
			let select = $("<select id='instance-select'></select>")
			for (var [name, location] of instances) {
				console.log(location)
				let location_str = Object.entries(location).map(([k,v]) => `${k}=${v}`).join(",")
				let option = $(`<option value="${location_str}">${name}</option>`)
				select.append(option)
			}
			select.on("change", function () {
				let location = $(this).val();
				let parts = location.split(",");
				for (let [i, part] of parts.entries()) {
					let [tag, value] = part.split("=");
					console.log(tag, value)
					$(`#axis-${tag}`).val(value)
				}
				$("#axes input").trigger("input");
				$("#axes input").trigger("change");
			})
			$("#axes").append(select)
		
		}
	}

	progress_callback(message) {
		// console.log("Got json ", message)
		if ("type" in message && message.type == "ready") {
			$("#bigLoadingModal").hide()
			$("#startModal").show()
		} else if (message.type == "axes") {
			this.setupAxes(message) // Contains axes and named instances
		} else if (message.type == "tables") {
			// console.log("Hiding spinner")
			$("#spinnerModal").hide();
			let table_diff = message.tables;
			$("#difftable").empty();
			$("#difftable").append(this.renderTableDiff({"tables":table_diff}, true).children())
		} else if (message.type == "glyphs") {
			$("#spinnerModal").hide();
			let glyph_diff = message.glyphs;
			this.renderGlyphDiff(glyph_diff);
			$(".node").on("click", function (event) { $(this).children().toggle(); event.stopPropagation() })
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
		return $("#axes input").map(function () {
			return `${this.id.replace("axis-", "")}=${this.value}`
		}).get().join(",")
	}


	letsDoThis() {
		$("#startModal").hide();
		$("#spinnerModal").show();
		diffWorker.postMessage({ command: "axes", beforeFont: this.beforeFont, afterFont: this.afterFont });
		diffWorker.postMessage({ command: "tables", beforeFont: this.beforeFont, afterFont: this.afterFont });
		this.updateGlyphs();
		this.updateWords();
	}

	updateGlyphs() {
		let location = this.variationLocation();
		diffWorker.postMessage({ command: "glyphs", beforeFont: this.beforeFont, afterFont: this.afterFont, location });
	}

	updateWords() {
		$("#wordspinner").show();
		$("#worddiff").empty();
		let location = this.variationLocation();
		diffWorker.postMessage({ command: "words", beforeFont: this.beforeFont, afterFont: this.afterFont, location });
	}

	addAGlyph(glyph, where) {
		where.append(`
			<div class="cell-word font-before">
		    <span data-toggle="tooltip" data-html="true" data-title="name: ${glyph.name}<br>unicode: ${glyph.unicode}">
	        ${glyph.string}
	        </span>
			</div>
		`);
	}


	addAWord(diff, where) {
		where.append(`
			<div class="cell-word font-before">
		    <span data-toggle="tooltip" data-html="true" data-title="before: <pre>${diff.buffer_a}</pre><br>after: <pre>${diff.buffer_b}</pre><br>percent: ${diff.percent}">
	        ${diff.word}
	        </span>
			</div>
		`);
	}
	renderGlyphDiff(glyph_diff) {
		$("#glyphdiff").empty();
		for (var [key, glyphs] of Object.entries(glyph_diff)) {
			let title = key.charAt(0).toUpperCase() + key.slice(1);
			if (glyphs.length > 0) {
				let that = this;
				$("#glyphdiff").append($(`<h2>${title} glyphs</h2>`));
				let place = $('<div class="glyphgrid"/>')
				$("#glyphdiff").append(place);
				glyphs.forEach((glyph) => {
					that.addAGlyph(glyph, place)
				})
			}
		}
		$('[data-toggle="tooltip"]').tooltip()
	}


	renderWordDiff(script, diffs) {
		$("#worddiff").append($(`<h2>${script} words</h2>`));
		let place = $('<div class="wordgrid"/>')
		$("#worddiff").append(place);
		diffs.forEach((glyph) => {
			this.addAWord(glyph, place)
		})
		$('[data-toggle="tooltip"]').tooltip()
	}

}

$(function () {
	window.diffenator = new Diffenator();
	diffWorker.onmessage = (e) => window.diffenator.progress_callback(e.data);
	$("#bigLoadingModal").show()

	$('.fontdrop').on(
		'dragover dragenter',
		function (e) {
			e.preventDefault();
			e.stopPropagation();
			$(this).addClass("dragging");
		}
	)
	$('.fontdrop').on(
		'dragleave dragend',
		function (e) {
			$(this).removeClass("dragging");
		}
	);

	$('.fontdrop').on(
		'drop',
		function (e) {
			$(this).removeClass("dragging");
			if (e.originalEvent.dataTransfer && e.originalEvent.dataTransfer.files.length) {
				e.preventDefault();
				e.stopPropagation();
				diffenator.dropFile(e.originalEvent.dataTransfer.files, this);
			}
		}
	);

	$("#fonttoggle").click(function () {
		if ($(this).text() == "Old") {
			$(this).text("New");
			$(".font-before").removeClass("font-before").addClass("font-after");
		} else {
			$(this).text("Old");
			$(".font-after").removeClass("font-after").addClass("font-before");
		}
	})

})
