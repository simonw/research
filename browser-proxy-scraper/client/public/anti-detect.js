/**
 * Conservative anti-detection patches for proxied pages.
 *
 * The goal is not to invent a fake browser from scratch. Instead we keep the
 * host browser's real Chrome/engine version and normalize the highest-signal
 * automation leaks so the JS fingerprint is more internally consistent.
 */
(function () {
  "use strict";

  const debug = !!globalThis.__ANTI_DETECT_DEBUG;
  const nav = navigator;
  const realUserAgent = String(nav.userAgent || "");
  const realAppVersion = String(nav.appVersion || "");
  const realPlatform = String(nav.platform || "");
  const realLanguages = Array.isArray(nav.languages) && nav.languages.length > 0
    ? [...nav.languages]
    : ["en-US", "en"];
  const uaMatch = realUserAgent.match(/Chrome\/(\d+)\.(\d+)\.(\d+)\.(\d+)/);
  const chromeVersion = uaMatch
    ? {
        major: uaMatch[1],
        full: uaMatch.slice(1).join("."),
      }
    : {
        major: "120",
        full: "120.0.0.0",
      };
  const platformInfo = derivePlatform(realUserAgent, realPlatform);
  const canvasSeed = getSessionSeed("anti-detect-canvas-seed");
  const realGlInfo = getRealWebglInfo();
  const defaultProfile = buildDefaultProfile(
    realUserAgent,
    realAppVersion,
    realPlatform,
    realLanguages,
    chromeVersion,
    platformInfo,
    canvasSeed,
    realGlInfo,
  );
  const overrideProfile = globalThis.__ANTI_DETECT_PROFILE_OVERRIDE || {};
  const profile = deepMerge(defaultProfile, overrideProfile);

  globalThis.__ANTI_DETECT_PROFILE = profile;

  patchNavigator(profile);
  patchPermissions(profile);
  patchWindowChrome();
  patchCanvas(profile.canvasSeed);
  patchWebgl(profile.webgl);
  patchWindowMetrics();
  patchLocationAndReferrer();
  stripUvLeak();

  logDebug("active profile", profile);

  function derivePlatform(userAgent, platform) {
    const lowerUa = String(userAgent || "").toLowerCase();
    const lowerPlatform = String(platform || "").toLowerCase();
    if (lowerPlatform.includes("mac") || lowerUa.includes("mac os x")) {
      return {
        platform: "MacIntel",
        uaPlatform: "macOS",
        mobile: false,
        vendor: "Google Inc.",
      };
    }
    if (lowerPlatform.includes("linux") || lowerUa.includes("linux")) {
      return {
        platform: "Linux x86_64",
        uaPlatform: "Linux",
        mobile: false,
        vendor: "Google Inc.",
      };
    }
    return {
      platform: "Win32",
      uaPlatform: "Windows",
      mobile: false,
      vendor: "Google Inc.",
    };
  }

  function getSessionSeed(key) {
    try {
      const existing = sessionStorage.getItem(key);
      if (existing) return Number(existing) || 1;
      const bytes = new Uint32Array(1);
      crypto.getRandomValues(bytes);
      const next = String(bytes[0] || 1);
      sessionStorage.setItem(key, next);
      return Number(next);
    } catch {
      return 1;
    }
  }

  function getRealWebglInfo() {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl") || canvas.getContext("experimental-webgl");
    const info = {
      vendor: "Google Inc.",
      renderer: "",
      maxTextureSize: 16384,
      maxCubeMapTextureSize: 16384,
      maxRenderbufferSize: 16384,
      maxVertexAttribs: 16,
      maxTextureImageUnits: 16,
      maxVertexTextureImageUnits: 16,
      maxCombinedTextureImageUnits: 32,
      maxVertexUniformVectors: 1024,
      maxFragmentUniformVectors: 1024,
      maxViewportDims: [16384, 16384],
    };

    if (!gl) {
      return info;
    }

    const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
    const vendor = debugInfo
      ? gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL)
      : gl.getParameter(gl.VENDOR);
    const renderer = debugInfo
      ? gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL)
      : gl.getParameter(gl.RENDERER);

    info.vendor = sanitizeWebglVendor(String(vendor || ""), platformInfo.uaPlatform);
    info.renderer = sanitizeWebglRenderer(String(renderer || ""), platformInfo.uaPlatform);
    info.maxTextureSize = safeNumber(gl.getParameter(gl.MAX_TEXTURE_SIZE), 16384);
    info.maxCubeMapTextureSize = safeNumber(gl.getParameter(gl.MAX_CUBE_MAP_TEXTURE_SIZE), 16384);
    info.maxRenderbufferSize = safeNumber(gl.getParameter(gl.MAX_RENDERBUFFER_SIZE), 16384);
    info.maxVertexAttribs = safeNumber(gl.getParameter(gl.MAX_VERTEX_ATTRIBS), 16);
    info.maxTextureImageUnits = safeNumber(gl.getParameter(gl.MAX_TEXTURE_IMAGE_UNITS), 16);
    info.maxVertexTextureImageUnits = safeNumber(gl.getParameter(gl.MAX_VERTEX_TEXTURE_IMAGE_UNITS), 16);
    info.maxCombinedTextureImageUnits = safeNumber(gl.getParameter(gl.MAX_COMBINED_TEXTURE_IMAGE_UNITS), 32);
    info.maxVertexUniformVectors = safeNumber(gl.getParameter(gl.MAX_VERTEX_UNIFORM_VECTORS), 1024);
    info.maxFragmentUniformVectors = safeNumber(gl.getParameter(gl.MAX_FRAGMENT_UNIFORM_VECTORS), 1024);

    const viewport = gl.getParameter(gl.MAX_VIEWPORT_DIMS);
    if (viewport && typeof viewport.length === "number") {
      info.maxViewportDims = [
        safeNumber(viewport[0], 16384),
        safeNumber(viewport[1], 16384),
      ];
    }

    return info;
  }

  function safeNumber(value, fallback) {
    const num = Number(value);
    return Number.isFinite(num) && num > 0 ? num : fallback;
  }

  function sanitizeWebglVendor(value, uaPlatform) {
    if (value && !/swiftshader|headless/i.test(value)) {
      return value;
    }
    if (uaPlatform === "macOS") return "Google Inc. (Apple)";
    if (uaPlatform === "Linux") return "Google Inc. (Intel)";
    return "Google Inc. (NVIDIA)";
  }

  function sanitizeWebglRenderer(value, uaPlatform) {
    if (value && !/swiftshader|headless/i.test(value)) {
      return value;
    }
    if (uaPlatform === "macOS") {
      return "ANGLE (Apple, ANGLE Metal Renderer: Apple M1, Unspecified Version)";
    }
    if (uaPlatform === "Linux") {
      return "ANGLE (Intel, Mesa Intel(R) UHD Graphics 620 (KBL GT2), OpenGL 4.6)";
    }
    return "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)";
  }

  function buildDefaultProfile(
    userAgent,
    appVersion,
    platform,
    languages,
    chrome,
    platformMeta,
    seed,
    webglInfo,
  ) {
    const pluginBlueprint = [
      {
        name: "PDF Viewer",
        filename: "internal-pdf-viewer",
        description: "Portable Document Format",
        mimeTypes: [
          { type: "application/pdf", suffixes: "pdf", description: "Portable Document Format" },
          { type: "text/pdf", suffixes: "pdf", description: "Portable Document Format" },
        ],
      },
      {
        name: "Chrome PDF Viewer",
        filename: "mhjfbmdgcfjbbpaeojofohoefgiehjai",
        description: "",
        mimeTypes: [],
      },
      {
        name: "Chromium PDF Viewer",
        filename: "internal-pdf-viewer",
        description: "",
        mimeTypes: [],
      },
    ];

    return {
      userAgent: userAgent,
      appVersion: appVersion,
      vendor: platformMeta.vendor,
      platform: platformMeta.platform || platform,
      hardwareConcurrency: normalizeHardwareConcurrency(nav.hardwareConcurrency),
      deviceMemory: normalizeDeviceMemory(nav.deviceMemory),
      maxTouchPoints: Number.isFinite(nav.maxTouchPoints) ? nav.maxTouchPoints : 0,
      languages,
      pdfViewerEnabled: true,
      plugins: pluginBlueprint,
      mimeTypes: pluginBlueprint.flatMap((plugin) => plugin.mimeTypes),
      uaData: buildUaData(chrome, platformMeta),
      canvasSeed: seed,
      webgl: webglInfo,
    };
  }

  function normalizeHardwareConcurrency(value) {
    const num = Number(value);
    if (!Number.isFinite(num) || num <= 0) return 8;
    if (num < 4) return 4;
    if (num > 16) return 16;
    return num;
  }

  function normalizeDeviceMemory(value) {
    const num = Number(value);
    if (!Number.isFinite(num) || num <= 0) return 8;
    if (num < 4) return 4;
    if (num > 16) return 16;
    return num;
  }

  function buildUaData(chrome, platformMeta) {
    const major = chrome.major;
    return {
      brands: [
        { brand: "Chromium", version: major },
        { brand: "Google Chrome", version: major },
        { brand: "Not.A/Brand", version: "24" },
      ],
      mobile: platformMeta.mobile,
      platform: platformMeta.uaPlatform,
      fullVersionList: [
        { brand: "Chromium", version: chrome.full },
        { brand: "Google Chrome", version: chrome.full },
        { brand: "Not.A/Brand", version: "24.0.0.0" },
      ],
    };
  }

  function deepMerge(base, override) {
    if (!override || typeof override !== "object") return base;
    const output = Array.isArray(base) ? [...base] : { ...base };
    for (const [key, value] of Object.entries(override)) {
      if (
        value &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        output[key] &&
        typeof output[key] === "object" &&
        !Array.isArray(output[key])
      ) {
        output[key] = deepMerge(output[key], value);
      } else {
        output[key] = value;
      }
    }
    return output;
  }

  function makePluginArray(pluginsSpec) {
    const plugins = pluginsSpec.map((pluginSpec, pluginIndex) => {
      const mimeTypes = pluginSpec.mimeTypes.map((mimeSpec) => ({
        type: mimeSpec.type,
        suffixes: mimeSpec.suffixes,
        description: mimeSpec.description,
        enabledPlugin: null,
      }));
      const plugin = {
        name: pluginSpec.name,
        filename: pluginSpec.filename,
        description: pluginSpec.description,
        length: mimeTypes.length,
        item(index) {
          return mimeTypes[index] || null;
        },
        namedItem(name) {
          return mimeTypes.find((entry) => entry.type === name) || null;
        },
      };
      mimeTypes.forEach((mimeType, mimeIndex) => {
        mimeType.enabledPlugin = plugin;
        plugin[mimeIndex] = mimeType;
      });
      plugin.__pluginIndex = pluginIndex;
      return plugin;
    });

    const arrayLike = {};
    plugins.forEach((plugin, index) => {
      arrayLike[index] = plugin;
      arrayLike[plugin.name] = plugin;
    });
    Object.defineProperties(arrayLike, {
      length: { value: plugins.length, enumerable: false },
      item: {
        value(index) {
          return plugins[index] || null;
        },
        enumerable: false,
      },
      namedItem: {
        value(name) {
          return plugins.find((plugin) => plugin.name === name) || null;
        },
        enumerable: false,
      },
      refresh: {
        value() {},
        enumerable: false,
      },
      [Symbol.iterator]: {
        value: function* iterator() {
          yield* plugins;
        },
        enumerable: false,
      },
    });
    return arrayLike;
  }

  function makeMimeTypeArray(mimeTypesSpec, pluginArray) {
    const mimeTypes = mimeTypesSpec.map((mimeSpec) => ({
      type: mimeSpec.type,
      suffixes: mimeSpec.suffixes,
      description: mimeSpec.description,
      enabledPlugin: pluginArray.namedItem("PDF Viewer"),
    }));
    const arrayLike = {};
    mimeTypes.forEach((mimeType, index) => {
      arrayLike[index] = mimeType;
      arrayLike[mimeType.type] = mimeType;
    });
    Object.defineProperties(arrayLike, {
      length: { value: mimeTypes.length, enumerable: false },
      item: {
        value(index) {
          return mimeTypes[index] || null;
        },
        enumerable: false,
      },
      namedItem: {
        value(name) {
          return mimeTypes.find((mimeType) => mimeType.type === name) || null;
        },
        enumerable: false,
      },
      [Symbol.iterator]: {
        value: function* iterator() {
          yield* mimeTypes;
        },
        enumerable: false,
      },
    });
    return arrayLike;
  }

  function defineGetter(target, property, getter) {
    try {
      Object.defineProperty(target, property, {
        configurable: true,
        get: getter,
      });
      return true;
    } catch {
      return false;
    }
  }

  function defineValue(target, property, value) {
    try {
      Object.defineProperty(target, property, {
        configurable: true,
        value,
      });
      return true;
    } catch {
      return false;
    }
  }

  function patchNavigator(activeProfile) {
    const pluginArray = makePluginArray(activeProfile.plugins);
    const mimeTypeArray = makeMimeTypeArray(activeProfile.mimeTypes, pluginArray);
    const uaData = buildUaDataObject(activeProfile.uaData, activeProfile);
    const targets = [nav, Object.getPrototypeOf(nav)].filter(Boolean);
    for (const target of targets) {
      defineGetter(target, "webdriver", () => undefined);
      defineGetter(target, "userAgent", () => activeProfile.userAgent);
      defineGetter(target, "appVersion", () => activeProfile.appVersion);
      defineGetter(target, "vendor", () => activeProfile.vendor);
      defineGetter(target, "platform", () => activeProfile.platform);
      defineGetter(target, "hardwareConcurrency", () => activeProfile.hardwareConcurrency);
      defineGetter(target, "deviceMemory", () => activeProfile.deviceMemory);
      defineGetter(target, "maxTouchPoints", () => activeProfile.maxTouchPoints);
      defineGetter(target, "languages", () => [...activeProfile.languages]);
      defineGetter(target, "plugins", () => pluginArray);
      defineGetter(target, "mimeTypes", () => mimeTypeArray);
      defineGetter(target, "pdfViewerEnabled", () => activeProfile.pdfViewerEnabled);
      if (uaData) {
        defineGetter(target, "userAgentData", () => uaData);
      }
    }
  }

  function buildUaDataObject(uaDataProfile, activeProfile) {
    if (!uaDataProfile) return null;
    return {
      brands: uaDataProfile.brands,
      mobile: uaDataProfile.mobile,
      platform: uaDataProfile.platform,
      toJSON() {
        return {
          brands: this.brands,
          mobile: this.mobile,
          platform: this.platform,
        };
      },
      async getHighEntropyValues(hints) {
        const result = {};
        for (const hint of hints || []) {
          if (hint === "architecture") result.architecture = "x86";
          if (hint === "bitness") result.bitness = "64";
          if (hint === "brands") result.brands = uaDataProfile.brands;
          if (hint === "fullVersionList") result.fullVersionList = uaDataProfile.fullVersionList;
          if (hint === "mobile") result.mobile = uaDataProfile.mobile;
          if (hint === "model") result.model = "";
          if (hint === "platform") result.platform = uaDataProfile.platform;
          if (hint === "platformVersion") result.platformVersion = guessPlatformVersion(activeProfile.platform);
          if (hint === "uaFullVersion") result.uaFullVersion = chromeVersion.full;
          if (hint === "wow64") result.wow64 = false;
        }
        return result;
      },
    };
  }

  function guessPlatformVersion(platform) {
    if (platform === "MacIntel") return "14.0.0";
    if (platform === "Linux x86_64") return "6.0.0";
    return "15.0.0";
  }

  function patchPermissions(activeProfile) {
    const permissionState = Notification.permission === "granted"
      ? "granted"
      : "default";
    try {
      if (typeof Notification !== "undefined") {
        defineGetter(Notification, "permission", () => permissionState);
      }
    } catch {
      // Ignore
    }

    const permissions = nav.permissions;
    const originalQuery = permissions?.query?.bind(permissions);
    if (!originalQuery) return;

    permissions.query = async function query(descriptor) {
      const name = descriptor && descriptor.name;
      if (name === "notifications") {
        return {
          state: permissionState === "default" ? "prompt" : permissionState,
          onchange: null,
        };
      }
      if (name === "clipboard-read" || name === "clipboard-write") {
        return {
          state: "prompt",
          onchange: null,
        };
      }
      return originalQuery(descriptor);
    };
  }

  function patchWindowChrome() {
    if (!window.chrome) {
      defineValue(window, "chrome", {});
    }
    if (!window.chrome.runtime) {
      window.chrome.runtime = {
        connect: undefined,
        sendMessage: undefined,
      };
    }
    if (!window.chrome.app) {
      window.chrome.app = {
        isInstalled: false,
      };
    }
    if (!window.chrome.csi) {
      window.chrome.csi = function csi() {
        return {
          onloadT: performance.now(),
          startE: Date.now() - Math.round(performance.now()),
          pageT: Math.round(performance.now()),
          tran: 15,
        };
      };
    }
    if (!window.chrome.loadTimes) {
      window.chrome.loadTimes = function loadTimes() {
        return {
          commitLoadTime: 0,
          connectionInfo: "h2",
          finishDocumentLoadTime: 0,
          finishLoadTime: 0,
          firstPaintAfterLoadTime: 0,
          firstPaintTime: 0,
          navigationType: "Other",
          npnNegotiatedProtocol: "h2",
          requestTime: 0,
          startLoadTime: 0,
          wasAlternateProtocolAvailable: false,
          wasFetchedViaSpdy: true,
          wasNpnNegotiated: true,
        };
      };
    }
  }

  function patchCanvas(seed) {
    const contextProto = window.CanvasRenderingContext2D && window.CanvasRenderingContext2D.prototype;
    const canvasProto = window.HTMLCanvasElement && window.HTMLCanvasElement.prototype;
    if (!contextProto || !canvasProto) return;

    const originalGetImageData = contextProto.getImageData;
    const originalToDataURL = canvasProto.toDataURL;
    const originalToBlob = canvasProto.toBlob;

    contextProto.getImageData = function patchedGetImageData(...args) {
      const imageData = originalGetImageData.apply(this, args);
      return perturbImageData(imageData, seed);
    };

    canvasProto.toDataURL = function patchedToDataURL(...args) {
      const clone = cloneCanvasWithNoise(this, seed);
      return originalToDataURL.apply(clone || this, args);
    };

    canvasProto.toBlob = function patchedToBlob(...args) {
      const clone = cloneCanvasWithNoise(this, seed);
      return originalToBlob.apply(clone || this, args);
    };
  }

  function cloneCanvasWithNoise(canvas, seed) {
    try {
      if (!canvas || !canvas.width || !canvas.height) return null;
      const clone = document.createElement("canvas");
      clone.width = canvas.width;
      clone.height = canvas.height;
      const ctx = clone.getContext("2d", { willReadFrequently: true });
      if (!ctx) return null;
      ctx.drawImage(canvas, 0, 0);
      const imageData = ctx.getImageData(0, 0, clone.width, clone.height);
      ctx.putImageData(perturbImageData(imageData, seed), 0, 0);
      return clone;
    } catch {
      return null;
    }
  }

  function perturbImageData(imageData, seed) {
    if (!imageData || !imageData.data || imageData.data.length === 0) {
      return imageData;
    }
    const data = new Uint8ClampedArray(imageData.data);
    const stride = Math.max(64, Math.floor(data.length / 128));
    for (let index = 0; index < data.length; index += stride) {
      const delta = ((seed + index) % 3) - 1;
      data[index] = clampByte(data[index] + delta);
      if (index + 1 < data.length) data[index + 1] = clampByte(data[index + 1] - delta);
      if (index + 2 < data.length) data[index + 2] = clampByte(data[index + 2] + (delta === 0 ? 1 : delta));
    }
    return new ImageData(data, imageData.width, imageData.height);
  }

  function clampByte(value) {
    return Math.max(0, Math.min(255, value));
  }

  function patchWebgl(webglProfile) {
    patchWebglContext(window.WebGLRenderingContext && window.WebGLRenderingContext.prototype, webglProfile);
    patchWebglContext(window.WebGL2RenderingContext && window.WebGL2RenderingContext.prototype, webglProfile);
  }

  function patchWebglContext(proto, webglProfile) {
    if (!proto) return;
    const originalGetExtension = proto.getExtension;
    const originalGetParameter = proto.getParameter;
    const debugExtension = {
      UNMASKED_VENDOR_WEBGL: 0x9245,
      UNMASKED_RENDERER_WEBGL: 0x9246,
    };

    proto.getExtension = function patchedGetExtension(name) {
      if (name === "WEBGL_debug_renderer_info") {
        return debugExtension;
      }
      return originalGetExtension.apply(this, arguments);
    };

    proto.getParameter = function patchedGetParameter(parameter) {
      if (parameter === debugExtension.UNMASKED_VENDOR_WEBGL) {
        return webglProfile.vendor;
      }
      if (parameter === debugExtension.UNMASKED_RENDERER_WEBGL) {
        return webglProfile.renderer;
      }

      const value = originalGetParameter.apply(this, arguments);
      switch (parameter) {
        case this.MAX_TEXTURE_SIZE:
          return Math.max(Number(value) || 0, webglProfile.maxTextureSize);
        case this.MAX_CUBE_MAP_TEXTURE_SIZE:
          return Math.max(Number(value) || 0, webglProfile.maxCubeMapTextureSize);
        case this.MAX_RENDERBUFFER_SIZE:
          return Math.max(Number(value) || 0, webglProfile.maxRenderbufferSize);
        case this.MAX_VERTEX_ATTRIBS:
          return Math.max(Number(value) || 0, webglProfile.maxVertexAttribs);
        case this.MAX_TEXTURE_IMAGE_UNITS:
          return Math.max(Number(value) || 0, webglProfile.maxTextureImageUnits);
        case this.MAX_VERTEX_TEXTURE_IMAGE_UNITS:
          return Math.max(Number(value) || 0, webglProfile.maxVertexTextureImageUnits);
        case this.MAX_COMBINED_TEXTURE_IMAGE_UNITS:
          return Math.max(Number(value) || 0, webglProfile.maxCombinedTextureImageUnits);
        case this.MAX_VERTEX_UNIFORM_VECTORS:
          return Math.max(Number(value) || 0, webglProfile.maxVertexUniformVectors);
        case this.MAX_FRAGMENT_UNIFORM_VECTORS:
          return Math.max(Number(value) || 0, webglProfile.maxFragmentUniformVectors);
        case this.MAX_VIEWPORT_DIMS:
          return new Int32Array(webglProfile.maxViewportDims);
        default:
          return value;
      }
    };
  }

  function patchWindowMetrics() {
    const outerWidth = Number(window.outerWidth);
    const outerHeight = Number(window.outerHeight);
    const safeOuterWidth = outerWidth > window.innerWidth ? outerWidth : window.innerWidth + 12;
    const safeOuterHeight = outerHeight > window.innerHeight ? outerHeight : window.innerHeight + 88;

    defineGetter(window, "outerWidth", () => safeOuterWidth);
    defineGetter(window, "outerHeight", () => safeOuterHeight);
  }

  function patchLocationAndReferrer() {
    try {
      // Find the active target origin
      const targetOrigin = window.__antiDetectTarget ? window.__antiDetectTarget.origin
        : window.__uv$location ? window.__uv$location.origin
        : window.location.origin;

      // Ensure location.fakeAncestorOrigins exists for our AST rewriter
      if (typeof Location !== "undefined") {
        defineGetter(Location.prototype, "fakeAncestorOrigins", () => {
          const arr = [targetOrigin];
          arr.item = (i) => arr[i];
          arr.contains = (s) => arr.includes(s);
          return arr;
        });
      } else {
        defineGetter(window.location, "fakeAncestorOrigins", () => {
          const arr = [targetOrigin];
          arr.item = (i) => arr[i];
          arr.contains = (s) => arr.includes(s);
          return arr;
        });
      }

      // Ensure document.fakeReferrer exists for our AST rewriter 
      if (typeof Document !== "undefined") {
         defineGetter(Document.prototype, "fakeReferrer", () => {
            return targetOrigin + "/";
         });
      } else {
         defineGetter(document, "fakeReferrer", () => {
            return targetOrigin + "/";
         });
      }

    } catch (e) {
      logDebug("Failed to patch location/referrer", e);
    }
  }

  function stripUvLeak() {
    try {
      if (typeof __uv$config !== "undefined") {
        delete self.__uv$config;
      }
    } catch {
      // Ignore
    }
  }

  function logDebug(...args) {
    if (debug) {
      console.log("[anti-detect]", ...args);
    }
  }
})();
