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
  /**
   * Synchronous named-font lookup for ^A@/^CW. Receives a validated uppercase
   * device:path (default device R:), once per resolved name per render.
   * Return quadratic TrueType bytes; null/undefined delegates to bundled ROM
   * fonts. In-job downloads take precedence. Throws propagate as render errors.
   * Bytes are copied; no filesystem access is performed by the package.
   */
  resolveFont?: (name: string) => Uint8Array | null | undefined;
  /** Nonnegative integer byte budgets, up to 4294967295. Other Rust budgets remain. */
  limits?: {
    /** Raw input and expanded formats, each separately. Default 1048576 (1 MiB). */
    inputBytes?: number;
    /** Total decoded inline-font downloads. Default 16777216 (16 MiB). */
    fontBytes?: number;
  };
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

/** Syntax bytes, not Unicode characters. Defaults: ^ (94), ~ (126), comma (44). */
export interface Syntax {
  formatPrefix: number;
  controlPrefix: number;
  delimiter: number;
}

export interface ParseOptions {
  /** Initial syntax for this complete stream. Each byte must be in 0..255. */
  syntax?: Partial<Syntax>;
}

export interface Element {
  kind: 'before_first_command' | 'format_command' | 'control_command' | 'control_character';
  /** Offset in input bytes, including for UTF-8 string input. */
  offset: number;
  /** Owned, unmodified bytes, including whitespace and binary payloads. */
  data: Buffer;
}

export interface ParseResult {
  elements: Element[];
  /** Carry to the next complete stream; this is not incremental chunk parsing. */
  syntax: Syntax;
}

export type ParseErrorKind =
  | 'IncompleteCommand'
  | 'MissingSyntaxCharacter'
  | 'InvalidBinaryHeader'
  | 'InvalidBinaryLength'
  | 'TruncatedBinaryData'
  | 'InvalidEncodedData'
  | 'IncompleteEncodedData';

export class ParseError extends Error {
  constructor(offset: number, kind: ParseErrorKind);
  offset: number;
  kind: ParseErrorKind;
}

/** Lossless command framing, not operand validation. Throws ParseError on framing failure. */
export function parse(input: string | Uint8Array, options?: ParseOptions): ParseResult;
