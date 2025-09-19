import type { Difference, CmapDiff, GlyphDiff } from "./api";
export type {
  GlyphDiff,
  CmapDiff,
  Report,
  LocationResult,
  Difference,
  EncodedGlyph,
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

type WordDiffMessage = { type: "words"; words: WordDiffs };
type Location = Record<string, number>;
type InstancePosition = [string, Location];
type CmapDiffMessage = { type: "new_missing_glyphs"; cmap_diff: CmapDiff };
type ReadyMessage = { type: "ready" };
type TablesMessage = { type: "tables"; tables: Record<string, Diff> };
export type ModifiedGlyphsMessage = {
  type: "modified_glyphs";
  modified_glyphs: GlyphDiff[];
};
type KernDiffMessage = { type: "kerns"; kerns: Record<string, Diff> };

export interface ValueRecord {
  x?: number | Record<string, number>;
  y?: number | Record<string, number>;
  x_placement?: number | Record<string, number>;
  y_placement?: number | Record<string, number>;
}
export type Message =
  | ReadyMessage
  | WordDiffMessage
  | AxesMessage
  | TablesMessage
  | KernDiffMessage
  | ModifiedGlyphsMessage
  | CmapDiffMessage;
export type AxesMessage = {
  type: "axes";
  axes: Record<string, [number, number, number]>;
  instances: InstancePosition[];
};
