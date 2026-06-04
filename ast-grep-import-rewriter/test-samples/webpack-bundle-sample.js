// Simulated webpack bundle with obfuscated module references
// This represents a more realistic obfuscation scenario

(function(modules) {
  // Webpack bootstrap code
  var installedModules = {};

  function __webpack_require__(moduleId) {
    if(installedModules[moduleId]) {
      return installedModules[moduleId].exports;
    }
    var module = installedModules[moduleId] = {
      i: moduleId,
      l: false,
      exports: {}
    };
    modules[moduleId].call(module.exports, module, module.exports, __webpack_require__);
    module.l = true;
    return module.exports;
  }

  return __webpack_require__(__webpack_require__.s = "./src/index_0xa1b2.js");
})({
  "./src/index_0xa1b2.js": function(module, exports, __webpack_require__) {
    "use strict";
    var _utils = __webpack_require__("./src/utils_0xc3d4.js");
    var _api = __webpack_require__("./src/api_0xe5f6.js");
    var _config = __webpack_require__("./config_0xg7h8.json");

    console.log("App initialized");
    _api.fetchData(_config.endpoint);
  },

  "./src/utils_0xc3d4.js": function(module, exports, __webpack_require__) {
    "use strict";
    const lodash = __webpack_require__("lodash");
    const moment = __webpack_require__("moment");

    module.exports = {
      formatDate: function(d) { return moment(d).format(); },
      deepClone: function(o) { return lodash.cloneDeep(o); }
    };
  },

  "./src/api_0xe5f6.js": function(module, exports, __webpack_require__) {
    "use strict";
    const axios = __webpack_require__("axios");
    const helpers = __webpack_require__("./src/helpers_0xi9j0.js");

    module.exports = {
      fetchData: async function(url) {
        const response = await axios.get(url);
        return helpers.transform(response.data);
      }
    };
  },

  "./src/helpers_0xi9j0.js": function(module, exports) {
    "use strict";
    module.exports = {
      transform: function(data) { return data.map(x => x.value); }
    };
  }
});

// Dynamic chunks loaded later
Promise.all([
  import("./chunks/chunk_0xk1l2.js"),
  import("./chunks/chunk_0xm3n4.js")
]).then(([chunk1, chunk2]) => {
  console.log("Chunks loaded");
});
