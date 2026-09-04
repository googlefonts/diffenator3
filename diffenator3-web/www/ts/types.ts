import type {
  Difference,
  CmapDiff,
  GlyphDiff,
  LanguageDiff,
  LocationResult,
  SignatureSummary,
} from "./api";
export type {
  GlyphDiff,
  CmapDiff,
  Report,
  LocationResult,
  Difference,
  EncodedGlyph,
  LanguageDiff,
  SignatureSummary,
} from "./api";

export type Value = string | number | boolean;
export type SimpleDiff = [Value, Value];
type ArrayDiff = { [key: number]: Diff };
type TooManyDiffs = { error: string };
export type ObjectDiff = { [key: string]: Diff | null | TooManyDiffs };
export type Diff = SimpleDiff | ArrayDiff | ObjectDiff | null;

export function isValue(node: Diff | Value): node is Value {
  return node?.constructor != Object;
}
export function isSimpleDiff(node: Diff | Value): node is SimpleDiff {
  return Array.isArray(node) && node.length == 2;
}
export function isArrayDiff(node: Diff | Value): node is ArrayDiff {
  return (
    node?.constructor == Object &&
    Object.keys(node).every((k) => !isNaN(parseInt(k, 10)))
  );
}

export type WordDiffs = Record<string, Difference[]>;

export type Location = Record<string, number>;
export type InstancePosition = [string, Location];
export type AxesMessage = {
  type: "axes";
  axes: Record<string, [number, number, number]>;
  instances: InstancePosition[];
};

type WordDiffMessage = { type: "words"; words: WordDiffs; token: number };
type CmapDiffMessage = {
  type: "cmap_diff";
  cmap_diff: CmapDiff;
};
type ReadyMessage = { type: "ready" };
type TablesMessage = { type: "tables"; tables: Record<string, Diff> };
type LanguagesMessage = {
  type: "languages";
  languages: Record<string, LanguageDiff>;
};
export type ModifiedGlyphsMessage = {
  type: "modified_glyphs";
  modified_glyphs: GlyphDiff[];
  token: number;
};
type KernDiffMessage = { type: "kerns"; kerns: Record<string, Diff> };
/** Result of the `use_auto_by_default` wasm call: whether to start in auto mode. */
export type AutoDefaultMessage = { type: "auto_default"; auto: boolean };
/** Auto mode: `diff_all` streams these in order -- the interesting locations
 * first (to seed the location nav), then the glyph diffs, then the word diffs,
 * each as soon as it is available. */
export type DiffLocationsMessage = {
  type: "diff_locations";
  locations: LocationResult[];
};
export type DiffGlyphsMessage = {
  type: "diff_glyphs";
  locations: LocationResult[];
};
/** Auto mode: on-demand word diffs for one location, requested when the user
 * clicks that location in the nav (words are too slow to compute eagerly). */
export type AutoWordsMessage = {
  type: "auto_words";
  location: string;
  words: WordDiffs;
};
/** Auto mode: the human-readable difference summary, posted back first. */
export type DiffSummaryMessage = {
  type: "diff_summary";
  summary: SignatureSummary;
};

export type ReceivedMessage =
  | ReadyMessage
  | WordDiffMessage
  | AxesMessage
  | TablesMessage
  | KernDiffMessage
  | ModifiedGlyphsMessage
  | CmapDiffMessage
  | LanguagesMessage
  | AutoDefaultMessage
  | DiffSummaryMessage
  | DiffLocationsMessage
  | DiffGlyphsMessage
  | AutoWordsMessage;

export interface ValueRecord {
  x?: number | Record<string, number>;
  y?: number | Record<string, number>;
  x_placement?: number | Record<string, number>;
  y_placement?: number | Record<string, number>;
}

export type SimpleCommand = "tables" | "kerns" | "cmap_diff" | "languages";
export type SentMessage =
  | {
      command: SimpleCommand | "axes" | "auto_default";
      beforeFont: Uint8Array<ArrayBufferLike>;
      afterFont: Uint8Array<ArrayBufferLike>;
    }
  | {
      command: "modified_glyphs";
      beforeFont: Uint8Array<ArrayBufferLike>;
      afterFont: Uint8Array<ArrayBufferLike>;
      location: string;
      /** Correlates glyph/word responses with the location they were requested for. */
      token: number;
    }
  | {
      command: "words";
      beforeFont: Uint8Array<ArrayBufferLike>;
      afterFont: Uint8Array<ArrayBufferLike>;
      customWords: string[];
      location: string;
      token: number;
    }
  | {
      command: "diff_all";
      beforeFont: Uint8Array<ArrayBufferLike>;
      afterFont: Uint8Array<ArrayBufferLike>;
      customWords: string[];
    }
  | {
      command: "auto_words";
      location: string;
    };
