// Sample obfuscated JavaScript with various import patterns

// ES6 imports with obfuscated module names
import { _0x1a2b as foo } from "./mod_0x3c4d";
import _0x5e6f from "./util_0x7g8h";
import * as _0x9i0j from "./lib_0xabcd";
import { alpha, _0xbeta, gamma as _0xgamma } from "./helpers_0xefgh";

// Dynamic imports
const mod1 = await import("./dynamic_0x1234");
const mod2 = import('./lazy_0x5678');

// CommonJS require patterns
const oldModule = require("./legacy_0xaaaa");
const { extract } = require("./utils_0xbbbb");
const partial = require("./partial_0xcccc").subModule;

// Re-exports
export { something } from "./reexport_0xdddd";
export * from "./reexport_all_0xeeee";
export { _0x1111 as renamed } from "./renamed_0xffff";

// Side-effect imports
import "./sideeffect_0x0000";

// Multi-line imports
import {
  longName1,
  longName2,
  _0xlong3 as alias3
} from "./multiline_0x1111";

// String variations - single vs double quotes
import singleQuote from './single_0x2222';
import doubleQuote from "./double_0x3333";

// Template literal (edge case - not typically valid but might appear in bundles)
// const tpl = require(`./template_${someVar}`);

// Webpack/bundler specific patterns
__webpack_require__("./webpack_0x4444");
__webpack_require__.r(exports);
__webpack_require__.d(exports, "named", function() { return named; });

// Example of obfuscated function that creates requires
function _0x9876() {
  return require("./internal_0x5555");
}

// AMD-style define (less common but possible)
define(["./amd_0x6666", "./amd_0x7777"], function(a, b) {
  return a + b;
});

console.log("Module loaded");
