import * as wasm from "diffenator3";
wasm.debugging();



jQuery.fn.shake = function(interval, distance, times) {
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
		reader.onload = function(e) {
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

	async wordlists() {
		let scripts_before = new Set(JSON.parse(wasm.supported_scripts(this.beforeFont)));
		let scripts_after = new Set(JSON.parse(wasm.supported_scripts(this.afterFont)));
		console.log("Scripts before", scripts_before);
		console.log("Scripts after", scripts_after);
		// intersection
		let scripts = [...scripts_before].filter(x => scripts_after.has(x));
		return await Promise.all(
			scripts.map(async script => {
				let url = `${document.URL}/wordlists/${script}.txt`;
			  const resp = await fetch(url);
			  return [script, await resp.text()];
		}))
	}


	letsDoThis() {
		$("#startModal").hide()
			let diffs = JSON.parse(wasm.diff(this.beforeFont, this.afterFont));
			let table_diff = diffs["tables"];
			let glyph_diff = diffs["glyphs"];
			$("#difftable").append(this.renderTableDiff(table_diff, true).children())
			$(".node").on("click", function(event){ $(this).children().toggle(); event.stopPropagation() })
			this.renderGlyphDiff(glyph_diff);
		$("#worddiffbutton").click( () => {
			this.wordlists().then( (wl) => {
				$("#worddiff").empty()
				for (let [script, words] of wl) {
					let diffs = JSON.parse(wasm.word_diff(this.beforeFont, this.afterFont, words));
					if (diffs.length > 0) {
						this.renderWordDiff(script, diffs)
					}
				}
			})
		});
	}

	addAGlyph(glyph, where) {
		where.append(`
			<div class="cell-word font-before">
		    <span data-toggle="tooltip" data-html="true" data-title="name: ${ glyph.name }<br>unicode: ${ glyph.unicode }">
	        ${ glyph.string }
	        </span>
			</div>
		`);
	}


	addAWord(diff, where) {
		where.append(`
			<div class="cell-word font-before">
		    <span data-toggle="tooltip" data-html="true" data-title="before: <pre>${ diff.buffer_a }</pre><br>after: <pre>${ diff.buffer_b }</pre><br>percent: ${ diff.percent }">
	        ${ diff.word }
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
				glyphs.forEach( (glyph) => {
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
		diffs.forEach( (glyph) => {
					this.addAWord(glyph, place)
		})
		$('[data-toggle="tooltip"]').tooltip()
	}

}

$(function() {
	window.diffenator = new Diffenator();

	$("#startModal").show()

	$('.fontdrop').on(
		'dragover dragenter',
		function(e) {
			e.preventDefault();
			e.stopPropagation();
			$(this).addClass("dragging");
		}
	)
	$('.fontdrop').on(
		'dragleave dragend',
		function(e) {
			$(this).removeClass("dragging");
		}
	);

	$('.fontdrop').on(
		'drop',
		function(e) {
			$(this).removeClass("dragging");
			if (e.originalEvent.dataTransfer && e.originalEvent.dataTransfer.files.length) {
				e.preventDefault();
				e.stopPropagation();
				diffenator.dropFile(e.originalEvent.dataTransfer.files, this);
			}
		}
	);

	$("#fonttoggle").click(function() {
		if ($(this).text() == "Old") {
			$(this).text("New");
			$(".font-before").removeClass("font-before").addClass("font-after");
		} else {
			$(this).text("Old");
			$(".font-after").removeClass("font-after").addClass("font-before");
		}
	})

})
