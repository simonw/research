/* tslint:disable */
/* eslint-disable */

export function init(): void;

/**
 * Parse `html` and re-serialise it to a normalised form (implicit
 * <html>/<head>/<body> inserted, attributes quoted, entities
 * escaped). This is what you'd ship an end-user as the
 * "sanitiser / canonicaliser" output.
 */
export function normalise(html: string): string;

/**
 * Parse an HTML document and return a compact indented tree dump
 * that shows how html5ever actually interpreted the markup. This
 * makes the quirks-mode / implicit-tags behaviour visible, which is
 * what makes this interesting compared to a naive text formatter.
 */
export function parse_tree(html: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly init: () => void;
    readonly normalise: (a: number, b: number, c: number) => void;
    readonly parse_tree: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
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
