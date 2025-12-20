/* tslint:disable */
/* eslint-disable */

export function main(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly main: () => void;
  readonly wasm_bindgen__convert__closures_____invoke__h0230e50cfa92633d: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h1feb17ac7454ddfa: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hbc694b6bf507aa7c: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h54cc1cbbebead552: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h1b0963836600a57c: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h1c720e866c21e083: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__ha3d84879192fad52: (a: number, b: number) => [number, number];
  readonly wasm_bindgen__convert__closures_____invoke__hec0a73c96c1808bc: (a: number, b: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __externref_table_dealloc: (a: number) => void;
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
