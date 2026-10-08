import type { Buffer } from 'node:buffer';

export interface Options {
  /** PNG by default. PDF includes all labels unless label is supplied. */
  format?: 'png' | 'svg' | 'pdf';
  /** Matches the Rust default: zd621. */
  profile?: 'specification' | 'zd621' | 'zq610-plus';
  /** Initial canvas dimensions in dots; ZPL and the profile can override them. */
  width?: number;
  height?: number;
  /** Dots per inch. Defaults to the profile's DPI. */
  dpi?: number;
  /** Zero-based label index. PNG/SVG default to 0; PDF defaults to all labels. */
  label?: number;
}

export interface RenderResult {
  /** Owned bytes, including UTF-8 bytes for SVG. */
  data: Buffer;
  format: 'png' | 'svg' | 'pdf';
  /** Actual selected label dimensions; first page for multipage PDF. */
  width: number;
  height: number;
  /** Total number of labels in the source. */
  labels: number;
  warnings: string[];
}

/** Synchronous local rendering. Throws on invalid options or rendering errors. */
export function render(input: string | Uint8Array, options?: Options): RenderResult;
/** Version of the linked Rust renderer, independent of the npm package version. */
export function libraryVersion(): string;
