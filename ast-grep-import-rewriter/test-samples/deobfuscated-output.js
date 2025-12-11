// Sample obfuscated JavaScript with various import patterns

// ES6 imports with obfuscated module names
import { _0x1a2b as foo } from "./modules/user-auth";
import _0x5e6f from "./utils/helpers";
import * as _0x9i0j from "./lib/core";
import { alpha, _0xbeta, gamma as _0xgamma } from "./utils/common-helpers";

// Dynamic imports
const mod1 = await import("./modules/lazy-loader");
const mod2 = import('./modules/deferred-init');

// CommonJS require patterns
const oldModule = require("./legacy/compat-layer");
const { extract } = require("./utils/extraction");
const partial = require("./partial/submodule").subModule;

// Re-exports
export { something } from "./exports/main";
export * from "./exports/all";
export { _0x1111 as renamed } from "./exports/renamed";

// Side-effect imports
import "./init/setup";

// Multi-line imports
import {
  longName1,
  longName2,
  _0xlong3 as alias3
} from "./modules/multi-component";

// String variations - single vs double quotes
import singleQuote from './single-module';
import doubleQuote from "./double-module";

// Template literal (edge case - not typically valid but might appear in bundles)
// const tpl = require(`./template_${someVar}`);

// Webpack/bundler specific patterns
__webpack_require__("./webpack/bundle");
__webpack_require__.r(exports);
__webpack_require__.d(exports, "named", function() { return named; });

// Example of obfuscated function that creates requires
function _0x9876() {
  return require("./internal/private");
}

// AMD-style define (less common but possible)
define(["./amd/module-a", "./amd/module-b"], function(a, b) {
  return a + b;
});

console.log("Module loaded");
