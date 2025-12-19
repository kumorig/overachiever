/* tslint:disable */
/* eslint-disable */

export function main(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly main: () => void;
  readonly wasm_bindgen__convert__closures_____invoke__h366949e1894c37ce: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h25f16fb129038ac0: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h8a4be4bdd04e51e4: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h35e1215ad8ba72c3: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h9242f69a4ca8d15f: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hbc694b6bf507aa7c: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h54cc1cbbebead552: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h036a814d33e3e6b7: (a: number, b: number) => [number, number];
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
