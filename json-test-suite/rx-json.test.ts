import { describe, expect, test } from "vitest";
import {
  b64Stringify,
  b64Parse,
  toZigZag,
  fromZigZag,
  stringify,
  parse,
  encode,
  open,
  splitNumber,
} from "./rx";
import { readFileSync } from "fs";
import { resolve } from "path";

const suite = JSON.parse(
  readFileSync("/home/user/research/json-test-suite/rx-tests.json", "utf-8"),
);

/** Decode a JSON test value, handling __special wrappers */
function decodeValue(v: unknown): unknown {
  if (v !== null && typeof v === "object" && !Array.isArray(v)) {
    const obj = v as Record<string, unknown>;
    if ("__special" in obj) {
      switch (obj.__special) {
        case "NaN": return NaN;
        case "Infinity": return Infinity;
        case "-Infinity": return -Infinity;
        case "undefined": return undefined;
      }
    }
    // Recurse into plain objects
    const result: Record<string, unknown> = {};
    for (const [key, val] of Object.entries(obj)) {
      result[key] = decodeValue(val);
    }
    return result;
  }
  if (Array.isArray(v)) {
    return v.map(decodeValue);
  }
  return v;
}

/** Deep compare that handles NaN, undefined, and proxy objects */
function deepEqual(actual: unknown, expected: unknown): void {
  if (expected !== null && typeof expected === "object" && !Array.isArray(expected)) {
    const expObj = expected as Record<string, unknown>;
    expect(typeof actual).toBe("object");
    expect(actual).not.toBeNull();
    const actObj = actual as Record<string, unknown>;
    const expKeys = Object.keys(expObj).sort();
    const actKeys = Object.keys(actObj).sort();
    expect(actKeys).toEqual(expKeys);
    for (const key of expKeys) {
      deepEqual(actObj[key], expObj[key]);
    }
    return;
  }
  if (Array.isArray(expected)) {
    expect(Array.isArray(actual) || (typeof actual === "object" && actual !== null && "length" in actual)).toBe(true);
    const actArr = [...(actual as any[])];
    expect(actArr.length).toBe(expected.length);
    for (let i = 0; i < expected.length; i++) {
      deepEqual(actArr[i], expected[i]);
    }
    return;
  }
  if (typeof expected === "number" && isNaN(expected)) {
    expect(actual).toBeNaN();
    return;
  }
  expect(actual).toBe(expected);
}

// ── b64 stringify ──

describe("JSON suite: b64_stringify", () => {
  for (const t of suite.b64_stringify.tests) {
    test(`b64Stringify(${t.input}) = "${t.expected}"`, () => {
      expect(b64Stringify(t.input)).toBe(t.expected);
    });
  }
});

// ── b64 parse ──

describe("JSON suite: b64_parse", () => {
  for (const t of suite.b64_parse.tests) {
    test(`b64Parse("${t.input}") = ${t.expected}`, () => {
      expect(b64Parse(t.input)).toBe(t.expected);
    });
  }
});

// ── zigzag encode ──

describe("JSON suite: zigzag_encode", () => {
  for (const t of suite.zigzag_encode.tests) {
    test(`toZigZag(${t.input}) = ${t.expected}`, () => {
      expect(toZigZag(t.input)).toBe(t.expected);
    });
  }
});

// ── zigzag decode ──

describe("JSON suite: zigzag_decode", () => {
  for (const t of suite.zigzag_decode.tests) {
    test(`fromZigZag(${t.input}) = ${t.expected}`, () => {
      expect(fromZigZag(t.input)).toBe(t.expected);
    });
  }
});

// ── stringify ──

describe("JSON suite: stringify", () => {
  for (const t of suite.stringify.tests) {
    test(t.name, () => {
      const input = decodeValue(t.input);
      const opts = t.options ?? {};
      expect(stringify(input, opts)).toBe(t.expected);
    });
  }
});

// ── parse ──

describe("JSON suite: parse", () => {
  for (const t of suite.parse.tests) {
    test(t.name, () => {
      const result = parse(t.input);
      const expected = decodeValue(t.expected);
      deepEqual(result, expected);
    });
  }
});

// ── roundtrip ──

describe("JSON suite: roundtrip", () => {
  for (const t of suite.roundtrip.tests) {
    test(t.name, () => {
      const value = decodeValue(t.value);
      const opts = t.options ?? {};
      const buf = encode(value, opts);
      const result = open(buf, opts?.refs);
      deepEqual(result, value);
    });
  }
});

// ── split number ──

describe("JSON suite: split_number", () => {
  for (const t of suite.split_number.tests) {
    test(`splitNumber(${t.input}) = [${t.base}, ${t.exponent}]`, () => {
      const [base, exp] = splitNumber(t.input);
      expect(base).toBe(t.base);
      expect(exp).toBe(t.exponent);
    });
  }
});
