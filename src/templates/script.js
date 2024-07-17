
	POSITION = "old"
	fontToggle = document.getElementById("font-toggle")
	function switchFonts() {
	boxTitles = document.getElementsByClassName("box-title")
	items = document.getElementsByClassName("box-text");
	  
	if (POSITION === "old") {
	  POSITION = "new"
	  for (item of boxTitles) {
	    item.textContent = item.textContent.replace("old", "new")
	    fontToggle.textContent = "new"
	  }
	  for (item of items) {
		item.className = item.className.replace("old", "new")
	  }

	  } else {
	  POSITION = "old"
	  for (item of boxTitles) {
	    item.textContent = item.textContent.replace("new", "old")
	    fontToggle.textContent = "old"
	  }
	  for (item of items) {
		item.className = item.className.replace("new", "old")
	  }
	}
	}
	if (fontToggle !== null) {
		fontToggle.addEventListener("click", switchFonts);
	}

// apply optional ot feats

function buildFeatureList() {
	var features = [
		'aalt', 'c2sc', 'calt', 'case', 'cpsp', 'dlig', 'dnom',
		'fina', 'frac', 'init', 'kern', 'liga', 'lnum', 'numr', 'onum',
		'ordn', 'pnum', 'salt', 'sinf', 'smcp', 'sups',
		'swsh', 'titl', 'tnum', 'zero', 'ss01', 'ss02',
		'ss03', 'ss04', 'ss05', 'ss06', 'ss07', 'ss08',
		'ss09', 'ss10', 'ss11', 'ss12', 'ss13', 'ss14',
		'ss15', 'ss16', 'ss17', 'ss18', 'ss19', 'ss20'
	]
	
	var otPanel = document.getElementById("ot-panel")
	for (i=0; i<features.length; i++) {
		var feat = features[i]
		
		var row = document.createElement("div");

		var item = document.createElement("input");
		item.type = "checkbox"
		item.value = feat
		item.classList.add("ot-checkbox")
		var label = document.createElement("label")
		label.textContent = feat

		row.appendChild(item)
		row.appendChild(label)

		otPanel.appendChild(row)
	}
}

buildFeatureList()


function enableFeatures() {
	var checkboxes = document.getElementsByClassName("ot-checkbox")
	var res = []
	var disableKerning = false
	for (i=0; i<checkboxes.length; i++) {
		if (checkboxes[i].checked === true) {
			var checkbox = checkboxes[i]
			res.push('"' + checkbox.value + '"')

			if (checkbox.value === "kern") {
				disableKerning = true
			}
		}
	}	

	var boxText = document.getElementsByClassName("box-text")
	var otString = res.join(", ")
	for (j=0; j<boxText.length; j++) {
		boxText[j].style.fontFeatureSettings = otString
		if (disableKerning === true) {
			boxText[j].style.fontKerning = "none"
		}
	}
}

var otButton = document.getElementById("ot-button")
otButton.addEventListener("click", function() {
    otPanel = document.getElementById("ot-panel")
    if (otPanel.style.display !== "block") {
        otPanel.style.display = "block"
    } else {
        otPanel.style.display = "none"
    }
})
var checkboxes = document.getElementsByClassName("ot-checkbox")
for (i=0; i<checkboxes.length; i++) {
	cb = checkboxes[i]
	cb.addEventListener("click", enableFeatures)
}	