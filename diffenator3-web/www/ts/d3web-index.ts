const diffWorker = new Worker(new URL("./webworker", import.meta.url));

import {
  cmapDiff,
  setupAnimation,
  diffTables,
  diffKerns,
  diffFeatures,
  diffLanguages,
  diffSignificantTables,
  renderGlyphs,
  renderWords,
  setVariationStyle,
  locationLabel,
  diffSignatureSummary,
} from "./shared";

import type {
  AxesMessage,
  ReceivedMessage,
  CmapDiff,
  Difference,
  GlyphDiff,
  Location,
  LocationResult,
  InstancePosition,
  SentMessage,
} from "./types";

declare global {
  interface JQuery {
    shake: (interval?: number, distance?: number, times?: number) => JQuery;
  }
}

jQuery.fn.shake = function (
  interval?: number,
  distance?: number,
  times?: number,
) {
  interval = typeof interval == "undefined" ? 100 : interval;
  distance = typeof distance == "undefined" ? 10 : distance;
  times = typeof times == "undefined" ? 3 : times;
  var jTarget = $(this);
  jTarget.css("position", "relative");
  for (var iter = 0; iter < times + 1; iter++) {
    jTarget.animate(
      {
        left: iter % 2 == 0 ? distance : distance * -1,
      },
      interval,
    );
  }
  return jTarget.animate(
    {
      left: 0,
    },
    interval,
  );
};

/** Serialise a location map to `tag=value,tag=value`. */
function instanceLocationString(location: Location): string {
  return Object.entries(location)
    .map(([k, v]) => `${k}=${v}`)
    .join(",");
}

/** Parse a `tag=value,tag=value` string into a location map. */
function instanceLocationFromString(str: string): Location {
  let loc: Location = {};
  for (let part of str.split(",")) {
    if (!part) continue;
    let [tag, value] = part.split("=");
    let parsed = parseFloat(value as string);
    if (tag && !isNaN(parsed)) loc[tag] = parsed;
  }
  return loc;
}

class Diffenator {
  beforeFont: Uint8Array | null;
  afterFont: Uint8Array | null;
  customWords: string[];

  /** Axes + named instances from the `axes()` wasm call. */
  axes: AxesMessage | null = null;
  /** True when the static-diff (signature) mode is active. */
  autoMode = false;

  // Auto mode streams results from `diff_all`: the interesting locations come
  // back first (seeding the nav), then the glyph diffs, then the word diffs.
  // Glyphs/words are accumulated per location so the selected view can be
  // re-rendered as each batch arrives.
  private autoLocations: LocationResult[] = [];
  private autoGlyphs = new Map<string, GlyphDiff[]>();
  private autoWords = new Map<string, Record<string, Difference[]>>();
  private autoGlyphsArrived = false;
  private autoWordsArrived = false;
  private selectedLocation: string | null = null;

  // Non-auto mode is on-demand: glyph/word responses carry a token that is
  // bumped whenever the requested location changes, so responses for an old
  // location are discarded.
  private diffToken = 0;
  private diffLocation: string | null = null;
  private lastSetupLocation: string | null = null;
  private glyphTimer: number | undefined;
  private initialized = false;
  private axesReady = false;
  private autoReady = false;

  constructor() {
    this.beforeFont = null;
    this.afterFont = null;
    this.customWords = [];
  }

  get beforeCssStyle() {
    return (document.styleSheets[0]!.cssRules[0]! as CSSStyleRule).style;
  }
  get afterCssStyle() {
    return (document.styleSheets[0]!.cssRules[1]! as CSSStyleRule).style;
  }

  dropFile(files: FileList, element: HTMLElement) {
    let file = files[0]!;
    if (!file.name.match(/\.[ot]tf$/i)) {
      $(element).shake();
      return;
    }
    var style;
    if (element.id == "fontbefore") {
      style = this.beforeCssStyle;
      $(element).find("h2").addClass("font-before");
    } else {
      style = this.afterCssStyle;
      $(element).find("h2").addClass("font-after");
    }
    $(element).find("h2").text(file.name);
    style.setProperty("src", "url(" + URL.createObjectURL(file) + ")");
    var reader = new FileReader();
    let that = this;
    reader.onload = function (e) {
      if (!this.result) return;
      let u8 = new Uint8Array(this.result as ArrayBuffer);
      if (element.id == "fontbefore") {
        that.beforeFont = u8;
      } else {
        that.afterFont = u8;
      }
      if (that.beforeFont && that.afterFont) {
        that.letsDoThis();
      }
    };
    reader.readAsArrayBuffer(file);
  }

  dropWordlist(files: FileList) {
    var reader = new FileReader();
    let that = this;
    reader.onload = function (e) {
      let contents = (e.target?.result as string) || "";
      that.customWords = contents
        .split("\n")
        .map(function (line) {
          return line.trim();
        })
        .filter(function (line) {
          return line.length > 0 && !line.startsWith("#");
        });
      $("#wordlistlabel").text(
        `${that.customWords.length} words loaded (you can drop more)`,
      );
    };
    reader.readAsText(files[0]!);
  }

  // ---- Axes panel ---------------------------------------------------------

  get instances(): InstancePosition[] {
    return this.axes?.instances ?? [];
  }

  setupAxes(message: AxesMessage) {
    this.axes = message;
    this.axesReady = true;
    $("#axes").empty();
    for (var [tag, limits] of Object.entries(message.axes)) {
      let [axis_min, axis_def, axis_max] = limits;
      let axis = $(`<div class="axis">
        <span class="axis-tag">${tag}</span>
        <input type="range" min="${axis_min}" max="${axis_max}" value="${axis_def}" class="slider" id="axis-${tag}">
      </div>`);
      $("#axes").append(axis);
      axis.on("input", this.onAxisInput.bind(this));
      axis.on("change", this.onAxisChange.bind(this));
    }
    if (message.instances.length > 0) {
      let select = $<HTMLSelectElement>(
        "<select id='instance-select'></select>",
      );
      for (var [name, location] of message.instances) {
        let location_str = instanceLocationString(location);
        let option = $(`<option value="${location_str}">${name}</option>`);
        select.append(option);
      }
      select.on("change", () => {
        this.selectInstance(instanceLocationFromString(select.val() as string));
      });
      $("#axes").append(select);
    }
    this.maybeInit();
  }

  sliderCoords(): Location {
    let coords: Location = {};
    $<HTMLInputElement>("#axes input[type=range]").each((_, el) => {
      coords[el.id.replace("axis-", "")] = parseFloat(el.value);
    });
    return coords;
  }

  variationLocation(): string {
    return instanceLocationString(this.sliderCoords());
  }

  applySliderVariation() {
    setVariationStyle(this.sliderCoords());
  }

  onAxisInput() {
    this.applySliderVariation();
    if (this.autoMode) return;
    if (this.glyphTimer) clearTimeout(this.glyphTimer);
    this.glyphTimer = window.setTimeout(() => {
      this.glyphTimer = undefined;
      this.requestGlyphs();
    }, 250);
  }

  onAxisChange() {
    if (this.autoMode) return;
    if (this.glyphTimer) {
      clearTimeout(this.glyphTimer);
      this.glyphTimer = undefined;
    }
    this.requestWords();
  }

  selectInstance(location: Location) {
    if (Object.keys(location).length === 0 && this.axes) {
      // "Default" pill: reset every slider to its axis default position.
      for (let [tag, limits] of Object.entries(this.axes.axes)) {
        $(`#axis-${tag}`).val(String(limits[1]));
      }
    } else {
      for (let [tag, value] of Object.entries(location)) {
        $(`#axis-${tag}`).val(String(value));
      }
    }
    this.applySliderVariation();
    if (this.autoMode) return;
    if (this.glyphTimer) {
      clearTimeout(this.glyphTimer);
      this.glyphTimer = undefined;
    }
    this.requestDiff();
  }

  // ---- Mode management ----------------------------------------------------

  requestDiffAll() {
    this.resetAutoState();
    this.showMainLoading("Analysing designspace...");
    diffWorker.postMessage({
      command: "diff_all",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
      customWords: this.customWords,
    } as SentMessage);
  }

  setAutoMode(auto: boolean) {
    if (this.autoMode === auto) return;
    this.autoMode = auto;
    $("#automode").prop("checked", auto);
    // Before init, maybeInit() will pick up the new mode once both the axes
    // and auto_default responses have arrived.
    if (!this.initialized) return;
    if (auto) {
      $("#axes").addClass("disabled");
      this.requestDiffAll();
    } else {
      $("#axes").removeClass("disabled");
      this.seedInstanceNav();
      this.requestDiff();
    }
  }

  /** Runs once both the axes and auto_default responses have arrived. */
  private maybeInit() {
    if (this.initialized || !this.axesReady || !this.autoReady) return;
    this.initialized = true;
    if (this.autoMode) {
      $("#axes").addClass("disabled");
      this.requestDiffAll();
    } else {
      this.seedInstanceNav();
      this.requestDiff();
    }
  }

  // ---- Location nav -------------------------------------------------------

  populateLocationNav(items: { label: string; location: string }[]) {
    $("#locationnav").empty();
    for (let item of items) {
      let pill = $(`<li class="nav-item">
        <a class="nav-link text-secondary" href="#" data-location="${encodeURIComponent(
          item.location,
        )}">${item.label.replaceAll(",", ",\u200b")}</a>
      </li>`);
      $("#locationnav").append(pill);
    }
    $("#locationnav li a").on("click", (e) => {
      e.preventDefault();
      let loc = decodeURIComponent(
        $(e.currentTarget).data("location") as string,
      );
      this.selectLocation(loc);
    });
  }

  /** Non-auto mode: the location nav lists the font's named instances. */
  seedInstanceNav() {
    let items = [{ label: "Default", location: "" }];
    for (let [name, location] of this.instances) {
      items.push({
        label: name,
        location: instanceLocationString(location),
      });
    }
    this.populateLocationNav(items);
  }

  /** Auto mode: the location nav is seeded from the signature's locations. */
  seedAutoNav() {
    this.populateLocationNav(
      this.autoLocations.map((l) => ({
        label: l.location,
        location: l.location,
      })),
    );
  }

  activateNavPill(loc: string) {
    $("#locationnav li a").removeClass("active");
    $(`#locationnav li a[data-location="${encodeURIComponent(loc)}"]`).addClass(
      "active",
    );
  }

  selectLocation(loc: string) {
    if (this.autoMode) {
      this.renderAutoLocation(loc);
    } else {
      this.selectInstance(instanceLocationFromString(loc));
    }
  }

  /** Auto mode: render a location from whatever glyph/word data has arrived. */
  private renderAutoLocation(loc: string) {
    this.selectedLocation = loc;
    this.activateNavPill(loc);
    let entry = this.autoLocations.find((l) => l.location === loc);
    let coords = entry?.coords ?? {};
    setVariationStyle(coords);
    $("#main").empty();
    $("#main").append(`<h2 class="mt-2">${locationLabel(coords)}</h2>`);
    let glyphsDiv = $('<div class="glyph-section"/>');
    let wordsDiv = $('<div class="word-section"/>');
    $("#main").append(glyphsDiv);
    $("#main").append(wordsDiv);
    if (this.autoGlyphsArrived) {
      renderGlyphs(this.autoGlyphs.get(loc), glyphsDiv);
    } else {
      glyphsDiv.append(
        `<h3 class="border-top pt-2 border-dark-subtle">Modified Glyphs</h3>
        <div class="diff-spinner"><div class="spinner-border" role="status"></div></div>`,
      );
    }
    if (this.autoWordsArrived) {
      renderWords(this.autoWords.get(loc), wordsDiv);
    } else {
      wordsDiv.append(
        `<h3 class="border-top pt-2 border-dark-subtle">Modified Words</h3>
        <div class="diff-spinner"><div class="spinner-border" role="status"></div></div>`,
      );
    }
    $('[data-bs-toggle="tooltip"]').tooltip();
  }

  /** Re-render the selected auto location when a glyph/word batch arrives. */
  private refreshAutoView() {
    if (this.autoMode && this.selectedLocation) {
      this.renderAutoLocation(this.selectedLocation);
    }
  }

  /** Show a small loading indicator in the main area. */
  private showMainLoading(text = "Loading...") {
    $("#main").empty();
    $("#main").append(
      `<div class="diff-spinner"><p>${text}</p><div class="spinner-border" role="status"></div></div>`,
    );
  }

  /** Clear the accumulated auto-mode streaming state before a fresh diff_all. */
  private resetAutoState() {
    this.autoLocations = [];
    this.autoGlyphs.clear();
    this.autoWords.clear();
    this.autoGlyphsArrived = false;
    this.autoWordsArrived = false;
    this.selectedLocation = null;
  }

  /**
   * Once both glyph and word diffs have arrived, prune the location nav down
   * to the locations that actually rendered differently (the signature can
   * flag locations whose changes don't manifest in the rendered output).
   */
  private filterAutoNav() {
    if (!this.autoGlyphsArrived || !this.autoWordsArrived) return;
    let keep = new Set<string>();
    for (let [loc, glyphs] of this.autoGlyphs) {
      if (glyphs.length > 0) keep.add(loc);
    }
    for (let [loc, words] of this.autoWords) {
      if (Object.keys(words).length > 0) keep.add(loc);
    }
    let filtered = this.autoLocations.filter((l) => keep.has(l.location));
    if (filtered.length === 0) return;
    this.autoLocations = filtered;
    this.populateLocationNav(
      filtered.map((l) => ({ label: l.location, location: l.location })),
    );
    if (this.selectedLocation) {
      if (filtered.some((l) => l.location === this.selectedLocation)) {
        this.activateNavPill(this.selectedLocation);
      } else {
        this.selectLocation(filtered[0]!.location);
      }
    }
  }

  // ---- On-demand (non-auto) diff ------------------------------------------

  /** Bump the token whenever the requested location changes. */
  private tokenFor(loc: string): number {
    if (loc !== this.diffLocation) {
      this.diffLocation = loc;
      this.diffToken += 1;
    }
    return this.diffToken;
  }

  /** Build the #main skeleton once per location; glyphs/words fill it in. */
  private beginLocation(loc: string): number {
    const token = this.tokenFor(loc);
    if (this.lastSetupLocation !== loc) {
      this.lastSetupLocation = loc;
      $("#main").empty();
      $("#main").append(
        `<h2 class="mt-2">${locationLabel(this.sliderCoords())}</h2>`,
      );
      $("#main").append(
        `<div id="mainglyphs"><div class="diff-spinner"><div class="spinner-border" role="status"></div></div></div>`,
      );
      $("#main").append(
        `<div id="mainwords"><div class="diff-spinner"><div class="spinner-border" role="status"></div></div></div>`,
      );
    }
    return token;
  }

  requestGlyphs() {
    const loc = this.variationLocation();
    const token = this.beginLocation(loc);
    diffWorker.postMessage({
      command: "modified_glyphs",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
      location: loc,
      token,
    } as SentMessage);
  }

  requestWords() {
    const loc = this.variationLocation();
    const token = this.beginLocation(loc);
    diffWorker.postMessage({
      command: "words",
      beforeFont: this.beforeFont,
      afterFont: this.afterFont,
      customWords: this.customWords,
      location: loc,
      token,
    } as SentMessage);
  }

  requestDiff() {
    this.requestGlyphs();
    this.requestWords();
  }

  // ---- Message handling ---------------------------------------------------

  progress_callback(message: ReceivedMessage) {
    if ("type" in message && message.type == "ready") {
      $("#bigLoadingModal").hide();
      $("#startModal").show();
    } else if (message.type == "axes") {
      this.setupAxes(message);
    } else if (message.type == "auto_default") {
      this.autoMode = message.auto;
      this.autoReady = true;
      $("#automode").prop("checked", message.auto);
      if (message.auto) $("#axes").addClass("disabled");
      this.maybeInit();
    } else if (message.type == "diff_summary") {
      diffSignatureSummary(message.summary, $("#signaturesummary"));
    } else if (message.type == "diff_locations") {
      this.autoLocations = message.locations;
      this.seedAutoNav();
      if (this.autoLocations.length > 0) {
        this.selectLocation(this.autoLocations[0]!.location);
      } else {
        $("#main").empty();
        $("#main").append(`<h2 class="mt-2">No differences found</h2>`);
      }
    } else if (message.type == "diff_glyphs") {
      this.autoGlyphsArrived = true;
      for (let entry of message.locations) {
        this.autoGlyphs.set(entry.location, entry.glyphs ?? []);
      }
      this.refreshAutoView();
      if (this.autoWordsArrived) this.filterAutoNav();
    } else if (message.type == "diff_words") {
      this.autoWordsArrived = true;
      for (let entry of message.locations) {
        this.autoWords.set(entry.location, entry.words ?? {});
      }
      this.refreshAutoView();
      if (this.autoGlyphsArrived) this.filterAutoNav();
    } else if (message.type == "tables") {
      diffTables(message);
      diffFeatures(message);
      // @ts-ignore
      window["tables"] = message;
      diffSignificantTables(message);
    } else if (message.type == "kerns") {
      diffKerns(message);
    } else if (message.type == "cmap_diff") {
      this.renderCmapDiff(message.cmap_diff);
    } else if (message.type == "languages") {
      diffLanguages(message.languages);
    } else if (message.type == "modified_glyphs") {
      if (message.token !== this.diffToken) return;
      renderGlyphs(message.modified_glyphs, $("#mainglyphs"));
      $('[data-bs-toggle="tooltip"]').tooltip();
    } else if (message.type == "words") {
      if (message.token !== this.diffToken) return;
      renderWords(message.words, $("#mainwords"));
      $('[data-bs-toggle="tooltip"]').tooltip();
    } else {
      console.log("Unknown message", message);
    }
  }

  renderCmapDiff(cmap_diff: CmapDiff) {
    $("#cmapdiff").empty();
    cmapDiff(cmap_diff);
    $('[data-bs-toggle="tooltip"]').tooltip();
  }

  letsDoThis() {
    $("#startModal").hide();
    // No full-screen spinner: show the results page immediately with a small
    // loading indicator; the per-section spinners take over as data arrives.
    this.showMainLoading("Loading...");
    for (let command of [
      "axes",
      "tables",
      "kerns",
      "cmap_diff",
      "languages",
      "auto_default",
    ]) {
      console.log("Sending command", command);
      diffWorker.postMessage({
        command,
        beforeFont: this.beforeFont!,
        afterFont: this.afterFont!,
      } as SentMessage);
    }
  }
}

$(function () {
  let diffenator = new Diffenator();
  diffWorker.onmessage = (e) => diffenator.progress_callback(e.data);
  $("#bigLoadingModal").show();

  $(".drop").on("dragover dragenter", function (e) {
    e.preventDefault();
    e.stopPropagation();
    $(this).addClass("dragging");
  });
  $(".drop").on("dragleave dragend", function (e) {
    $(this).removeClass("dragging");
  });

  $(".fontdrop").on("drop", function (e) {
    $(this).removeClass("dragging");
    if (
      e.originalEvent!.dataTransfer &&
      e.originalEvent!.dataTransfer.files.length
    ) {
      e.preventDefault();
      e.stopPropagation();
      diffenator.dropFile(e.originalEvent!.dataTransfer.files, this);
    }
  });

  $("#worddrop").on("drop", function (e) {
    $(this).removeClass("dragging");
    if (
      e.originalEvent!.dataTransfer &&
      e.originalEvent!.dataTransfer.files.length
    ) {
      e.preventDefault();
      e.stopPropagation();
      diffenator.dropWordlist(e.originalEvent!.dataTransfer.files);
    }
  });

  $("#automode").on("change", function () {
    diffenator.setAutoMode($(this).is(":checked"));
  });

  setupAnimation();

  $("body").tooltip({
    selector: '[data-toggle="tooltip"]',
  });
});
