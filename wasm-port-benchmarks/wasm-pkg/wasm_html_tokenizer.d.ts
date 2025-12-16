/* tslint:disable */
/* eslint-disable */

export class WasmTokenizer {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Count tokens without returning them (for benchmarking)
   */
  count_tokens(): number;
  constructor(html: string);
  /**
   * Tokenize the entire input and return tokens as JSON
   */
  tokenize(): any;
}

/**
 * Count tokens without returning them (for benchmarking)
 */
export function count_tokens(html: string): number;

/**
 * Tokenize HTML and return tokens as JSON (simple wrapper function)
 */
export function tokenize_html(html: string): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wasmtokenizer_free: (a: number, b: number) => void;
  readonly count_tokens: (a: number, b: number) => number;
  readonly tokenize_html: (a: number, b: number) => number;
  readonly wasmtokenizer_count_tokens: (a: number) => number;
  readonly wasmtokenizer_new: (a: number, b: number) => number;
  readonly wasmtokenizer_tokenize: (a: number) => number;
  readonly __wbindgen_export: (a: number, b: number) => number;
  readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
