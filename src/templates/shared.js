function renderTableDiff(node, toplevel) {
	var wrapper = $("<div> </div>");
	if (!node) {
		return wrapper
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
		display.append(renderTableDiff(value, false).children());
		if (display.children(".node").length > 0) {
			display.addClass("closed")
		}
		wrapper.append(display)
	}
	return wrapper

}


function addAGlyph(glyph, where) {
    let title = "";
    if (glyph.name) {
        title = "name: "+glyph.name;
    }
    let cp = "<br>U+"+glyph.string.charCodeAt(0).toString(16).padStart(4, '0').toUpperCase();
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
		<span data-toggle="tooltip" data-html="true" data-title="before: <pre>${diff.buffer_a}</pre><br>after: <pre>${diff.buffer_b}</pre><br>percent: ${diff.percent}">
		${diff.word}
		</span>
		</div>
	`);
}


export { renderTableDiff, addAGlyph, addAWord }