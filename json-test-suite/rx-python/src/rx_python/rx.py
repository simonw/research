"""
Python implementation of the REXC (rx) encoder/decoder.

This is a port of the TypeScript implementation at https://github.com/creationix/rx
"""

from __future__ import annotations

import math
import re
from typing import Any

# ── Sentinel for undefined ──

class _Undefined:
    """Sentinel for JavaScript's undefined value."""
    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __repr__(self):
        return "UNDEFINED"

    def __bool__(self):
        return False

Undefined = _Undefined
UNDEFINED = _Undefined()

# ── Tuning constants ──

INDEX_THRESHOLD = 16
STRING_CHAIN_THRESHOLD = 64
STRING_CHAIN_DELIMITER = "/"
KEY_COMPLEXITY_THRESHOLD = 100

# ── Base64 numeric system ──
# Numbers are written big-endian with the most significant digit on the left.
# There is no padding, not even for zero, which is an empty string.

B64_CHARS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-_"

# char-code -> digit-value (256 = invalid)
_b64_decode_table = [0xFF] * 256
# digit-value -> char-code
_b64_encode_table = [0] * 64

for _i in range(64):
    _code = ord(B64_CHARS[_i])
    _b64_decode_table[_code] = _i
    _b64_encode_table[_i] = _code


def is_b64(byte: int) -> bool:
    return _b64_decode_table[byte] != 0xFF


def b64_stringify(num: int) -> str:
    if num < 0:
        raise ValueError(f"Cannot stringify {num} as base64")
    if num == 0:
        return ""
    result = []
    while num > 0:
        result.append(B64_CHARS[num % 64])
        num //= 64
    return "".join(reversed(result))


def b64_parse(s: str) -> int:
    result = 0
    for ch in s:
        digit = _b64_decode_table[ord(ch)]
        if digit == 0xFF:
            raise ValueError(f"Invalid base64 character: {ch}")
        result = result * 64 + digit
    return result


def b64_read(data: bytes, left: int, right: int) -> int:
    result = 0
    for i in range(left, right):
        digit = _b64_decode_table[data[i]]
        if digit == 0xFF:
            raise ValueError(f"Invalid base64 character code: {data[i]}")
        result = result * 64 + digit
    return result


def b64_sizeof(num: int) -> int:
    if num < 0:
        raise ValueError(f"Cannot calculate size of {num} as base64")
    if num == 0:
        return 0
    return math.ceil(math.log(num + 1) / math.log(64))


def b64_write(data: bytearray, left: int, right: int, num: int) -> None:
    offset = right - 1
    while offset >= left:
        data[offset] = _b64_encode_table[num % 64]
        num //= 64
        offset -= 1
    if num > 0:
        raise ValueError(f"Cannot write {num} as base64")


# ── Zigzag encoding ──

def to_zigzag(num: int) -> int:
    if -0x80000000 <= num <= 0x7FFFFFFF:
        # 32-bit path: use unsigned right shift equivalent
        return ((num << 1) ^ (num >> 31)) & 0xFFFFFFFF
    return num * -2 - 1 if num < 0 else num * 2


def from_zigzag(num: int) -> int:
    if num <= 0xFFFFFFFF:
        # 32-bit path
        result = (num >> 1) ^ -(num & 1)
        # Ensure it's treated as signed 32-bit
        if result >= 0x80000000:
            result -= 0x100000000
        return result
    return num // 2 if num % 2 == 0 else (num + 1) // -2


# ── Number helpers ──

def _trim_zeroes(s: str) -> tuple[int, int]:
    trimmed = s.rstrip("0")
    return int(trimmed), len(s) - len(trimmed)


def split_number(val: float) -> tuple[int, int]:
    if isinstance(val, int) or (isinstance(val, float) and val == int(val) and math.isfinite(val)):
        ival = int(val)
        if abs(ival) < 10:
            return ival, 0
        if abs(ival) < 9.999999999999999e20:
            return _trim_zeroes(str(ival))

    dec_match = re.match(r"^([-+]?\d+)(?:\.(\d+))?$", f"{val:.14f}")
    if dec_match:
        integer_part = dec_match.group(1) or ""
        decimal_part = dec_match.group(2) or ""
        b1 = int(integer_part + decimal_part)
        e1 = -len(decimal_part) if decimal_part else 0
        if e1 == 0:
            return b1, 0
        b2, e2 = split_number(b1)
        return b2, e1 + e2

    sci_match = re.match(r"^([+-]?\d+)(?:\.(\d+))?(?:e([+-]?\d+))$", f"{val:.14e}")
    if sci_match:
        int_part = sci_match.group(1) or ""
        frac_part = sci_match.group(2) or ""
        exp_part = sci_match.group(3) or "0"
        e1 = -len(frac_part) if frac_part else 0
        e2 = int(exp_part)
        b1, e3 = _trim_zeroes(int_part + frac_part)
        return b1, e1 + e2 + e3

    raise ValueError(f"Invalid number format: {val}")


# ── UTF-8 sort ──

def utf8_sort_key(s: str) -> bytes:
    return s.encode("utf-8")


# ── Identity key for pointer dedup ──

def make_key(val: Any, ttl: int = KEY_COMPLEXITY_THRESHOLD) -> Any:
    result = _walk_key(val, [ttl])
    return result if result is not None else id(val)


def _walk_key(val: Any, ttl: list[int]) -> str | None:
    ttl[0] -= 1
    if ttl[0] <= 0:
        return None
    if val is None:
        return "null"
    if val is True:
        return "true"
    if val is False:
        return "false"
    if val is UNDEFINED:
        return "undefined"
    if isinstance(val, str):
        return repr(val)
    if isinstance(val, (int, float)):
        return str(val)
    if isinstance(val, list):
        parts = []
        for item in val:
            k = _walk_key(item, ttl)
            if k is None:
                return None
            parts.append(k)
        return "[" + ",".join(parts) + "]"
    if isinstance(val, dict):
        parts = []
        for ek, ev in val.items():
            kk = _walk_key(ek, ttl)
            if kk is None:
                return None
            vk = _walk_key(ev, ttl)
            if vk is None:
                return None
            parts.append(f"{kk}:{vk}")
        return "{" + ",".join(parts) + "}"
    return None


# ── Cursor ──

class Cursor:
    __slots__ = ("data", "left", "right", "tag", "val", "ix_width", "ix_count", "schema")

    def __init__(self, data: bytes):
        self.data = data
        self.left = 0
        self.right = len(data)
        self.tag = "null"
        self.val: Any = 0
        self.ix_width = 0
        self.ix_count = 0
        self.schema = 0


def _peek_tag(c: Cursor) -> int:
    data = c.data
    offset = c.right
    offset -= 1
    while offset >= 0 and is_b64(data[offset]):
        offset -= 1
    if offset < 0:
        raise SyntaxError("peekTag: no tag found")
    c.left = offset
    return data[offset]


def _unpack_index(c: Cursor, data: bytes, left: int, right: int) -> None:
    packed = b64_read(data, left, right)
    c.ix_width = (packed & 0b111) + 1
    c.ix_count = packed >> 3


def read(c: Cursor) -> str:
    data = c.data
    c.ix_width = 0
    c.ix_count = 0
    c.schema = 0

    tag = _peek_tag(c)
    left = c.left

    if tag == 0x27:  # ' — ref or builtin
        name_len = c.right - left - 1
        if name_len == 1:
            ch = data[left + 1]
            if ch == 0x74: c.tag = "true"; c.val = 0; return c.tag
            if ch == 0x66: c.tag = "false"; c.val = 0; return c.tag
            if ch == 0x6E: c.tag = "null"; c.val = 0; return c.tag
            if ch == 0x75: c.tag = "undef"; c.val = 0; return c.tag
        elif name_len == 3:
            a, b, d = data[left + 1], data[left + 2], data[left + 3]
            if a == 0x69 and b == 0x6E and d == 0x66:
                c.tag = "float"; c.val = float("inf"); return c.tag
            if a == 0x6E and b == 0x69 and d == 0x66:
                c.tag = "float"; c.val = float("-inf"); return c.tag
            if a == 0x6E and b == 0x61 and d == 0x6E:
                c.tag = "float"; c.val = float("nan"); return c.tag
        c.val = name_len
        c.tag = "ref"
        return c.tag

    b64 = b64_read(data, left + 1, c.right)

    if tag == 0x2C:  # , — string
        c.left = left - b64
        c.val = b64
        c.tag = "str"
        return c.tag

    if tag == 0x2B:  # + — integer
        c.val = from_zigzag(b64)
        c.tag = "int"
        return c.tag

    if tag == 0x2A:  # * — float (exponent)
        exp = from_zigzag(b64)
        saved_right = c.right
        c.right = left
        read(c)
        c.val = float(f"{c.val}e{exp}")
        c.right = saved_right
        c.tag = "float"
        return c.tag

    if tag == 0x3A:  # : — object
        content = left
        c.left = left - b64
        if content > c.left:
            # Parse optional schema then optional index
            _k = Cursor(data)
            _k.right = content
            inner_tag = _peek_tag(_k)
            # Schema: ' or ^
            if inner_tag == 0x27 or inner_tag == 0x5E:
                is_schema = True
                if inner_tag == 0x5E:
                    target = _k.left - b64_read(data, _k.left + 1, content)
                    _s = Cursor(data)
                    _s.right = target
                    target_tag = _peek_tag(_s)
                    is_schema = target_tag == 0x3B or target_tag == 0x3A
                if is_schema:
                    c.schema = content
                    content = _k.left
            # Index: #
            if content > c.left:
                _k.right = content
                inner_tag = _peek_tag(_k)
                if inner_tag == 0x23:
                    _unpack_index(c, data, _k.left + 1, content)
                    content = _k.left - c.ix_width * c.ix_count
        c.val = content
        c.tag = "object"
        return c.tag

    if tag == 0x3B:  # ; — array
        content = left
        c.left = left - b64
        if content > c.left:
            _k = Cursor(data)
            _k.data = data
            _k.right = content
            ix_tag = _peek_tag(_k)
            if ix_tag == 0x23:  # #
                _unpack_index(c, data, _k.left + 1, content)
                content = _k.left - c.ix_width * c.ix_count
        c.val = content
        c.tag = "array"
        return c.tag

    if tag == 0x5E:  # ^ — pointer
        c.val = left - b64
        c.tag = "ptr"
        return c.tag

    if tag == 0x2E:  # . — chain
        c.left = left - b64
        c.val = left
        c.tag = "chain"
        return c.tag

    raise SyntaxError(f"Unknown tag: {chr(tag)}")


# ── String handling ──

def _str_start(c: Cursor) -> int:
    return c.left + (1 if c.tag == "ref" else 0)


def read_str(c: Cursor) -> str:
    start = _str_start(c)
    return bytes(c.data[start:start + c.val]).decode("utf-8")


def resolve_str(c: Cursor) -> str:
    saved = (c.left, c.right, c.tag, c.val)
    result = _resolve_str(c)
    c.left, c.right, c.tag, c.val = saved
    return result


def _resolve_str(c: Cursor) -> str:
    while c.tag == "ptr":
        c.right = c.val
        read(c)
    if c.tag == "str":
        return read_str(c)
    if c.tag == "chain":
        parts = []
        right = c.val
        left = c.left
        while right > left:
            c.right = right
            read(c)
            right = c.left
            parts.append(_resolve_str(c))
        return "".join(parts)
    raise TypeError(f"resolveStr: expected str, ptr, or chain, got {c.tag}")


# ── Container access ──

def seek_child(c: Cursor, container: Cursor, index: int) -> None:
    if container.ix_width == 0:
        raise ValueError("seekChild requires an indexed container")
    if index < 0 or index >= container.ix_count:
        raise IndexError(f"seekChild: index {index} out of range [0, {container.ix_count})")
    ix_base = container.val
    ix_width = container.ix_width
    entry_left = ix_base + index * ix_width
    delta = b64_read(container.data, entry_left, entry_left + ix_width)
    c.data = container.data
    c.right = ix_base - delta
    read(c)


def collect_children(container: Cursor) -> list[int]:
    offsets = []
    cc = Cursor(container.data)
    right = container.val
    end = container.left
    while right > end:
        offsets.append(right)
        cc.right = right
        read(cc)
        right = cc.left
    return offsets


# ── High-level decode ──

def _open_context(buffer: bytes, refs: dict | None = None):
    scratch = Cursor(buffer)

    def wrap(c: Cursor) -> Any:
        while c.tag == "ptr":
            c.right = c.val
            read(c)
        if c.tag == "ref":
            if refs:
                name = read_str(c)
                if name in refs:
                    return refs[name]
            return UNDEFINED
        tag = c.tag
        if tag == "int" or tag == "float":
            return c.val
        if tag == "str":
            return read_str(c)
        if tag == "chain":
            return resolve_str(c)
        if tag == "true":
            return True
        if tag == "false":
            return False
        if tag == "null":
            return None
        if tag == "undef":
            return UNDEFINED
        if tag == "array":
            return _read_array(c)
        if tag == "object":
            return _read_object(c)
        raise ValueError(f"Unknown tag: {tag}")

    def _resolve_key_str(c: Cursor) -> str:
        saved = (c.left, c.right, c.tag, c.val)
        while c.tag == "ptr":
            c.right = c.val
            read(c)
        if c.tag == "ref" and refs:
            name = read_str(c)
            val = refs.get(name)
            if isinstance(val, str):
                result = val
            else:
                result = resolve_str(c)
        else:
            result = resolve_str(c)
        c.left, c.right, c.tag, c.val = saved
        return result

    def _read_array(container: Cursor) -> list:
        result = []
        right = container.val
        end = container.left
        while right > end:
            child = Cursor(buffer)
            child.right = right
            read(child)
            node_left = child.left  # save before wrap modifies cursor
            result.append(wrap(child))
            right = node_left
        return result

    def _read_object(container: Cursor) -> dict:
        result = {}
        data = container.data
        right = container.val
        end = container.left

        if container.schema != 0:
            # Schema object
            sc = Cursor(data)
            sc.right = container.schema
            read(sc)
            while sc.tag == "ptr":
                sc.right = sc.val
                read(sc)

            # Handle ref schemas
            if sc.tag == "ref" and refs:
                ref_val = refs.get(read_str(sc))
                if isinstance(ref_val, list):
                    key_strings = ref_val
                elif isinstance(ref_val, dict):
                    key_strings = list(ref_val.keys())
                else:
                    key_strings = []
                val_right = container.val
                for name in key_strings:
                    vc = Cursor(data)
                    vc.right = val_right
                    read(vc)
                    node_left = vc.left
                    result[name] = wrap(vc)
                    val_right = node_left
                return result

            # Inline schema
            kc = Cursor(data)
            val_right = container.val

            if sc.tag == "object":
                key_right = sc.val
                key_end = sc.left
                while key_right > key_end and val_right > end:
                    kc.right = key_right
                    read(kc)
                    name = _resolve_key_str(kc)
                    # Skip schema value
                    sc2 = Cursor(data)
                    sc2.right = kc.left
                    read(sc2)
                    key_right = sc2.left

                    vc = Cursor(data)
                    vc.right = val_right
                    read(vc)
                    node_left = vc.left
                    result[name] = wrap(vc)
                    val_right = node_left
            elif sc.tag == "array":
                key_right = sc.val
                key_end = sc.left
                while key_right > key_end and val_right > end:
                    kc.right = key_right
                    read(kc)
                    name = _resolve_key_str(kc)
                    key_right = kc.left

                    vc = Cursor(data)
                    vc.right = val_right
                    read(vc)
                    node_left = vc.left
                    result[name] = wrap(vc)
                    val_right = node_left
            return result

        # No schema: interleaved key/value pairs
        while right > end:
            kc = Cursor(data)
            kc.right = right
            read(kc)
            key_name = _resolve_key_str(kc)

            vc = Cursor(data)
            vc.right = kc.left
            read(vc)
            node_left = vc.left
            result[key_name] = wrap(vc)
            right = node_left

        return result

    def resolve(right: int) -> Any:
        scratch.data = buffer
        scratch.right = right
        read(scratch)
        return wrap(scratch)

    root = resolve(len(buffer))
    return root


def open_buffer(buffer: bytes, refs: dict | None = None) -> Any:
    return _open_context(buffer, refs)


def decode(input_bytes: bytes, refs: dict | None = None) -> Any:
    return open_buffer(input_bytes, refs)


def parse(input_str: str, refs: dict | None = None) -> Any:
    return decode(input_str.encode("utf-8"), refs)


# ── Encoder ──

def _write_unsigned(tag: str, value: int) -> str:
    return f"{tag}{b64_stringify(value)}"


def _write_signed(tag: str, value: int) -> str:
    return f"{tag}{b64_stringify(to_zigzag(value))}"


def encode(value: Any, options: dict | None = None) -> bytes:
    return stringify(value, options).encode("utf-8")


def stringify(root_value: Any, options: dict | None = None) -> str:
    opts = options or {}
    index_threshold = opts.get("indexThreshold", INDEX_THRESHOLD)
    chain_threshold = opts.get("stringChainThreshold", STRING_CHAIN_THRESHOLD)
    chain_delimiter = opts.get("stringChainDelimiter", STRING_CHAIN_DELIMITER)

    # Build refs map: value -> ref name
    refs_dict = opts.get("refs", {})
    refs_map: dict[Any, str] = {}
    for key, val in refs_dict.items():
        refs_map[make_key(val)] = key

    seen_offsets: dict[Any, int] = {}
    schema_offsets: dict[Any, int | str] = {}
    seen_costs: dict[Any, int] = {}

    # Pre-scan refs for schema keys
    for key, val in refs_dict.items():
        if isinstance(val, (list, dict)):
            schema_key = make_key(list(val.keys()) if isinstance(val, dict) else val)
            schema_offsets[schema_key] = key

    parts: list[str] = []
    byte_length = 0

    # Pre-scan for reused path prefixes
    duplicate_prefixes: set[str] = set()
    seen_prefixes: set[str] = set()

    def scan_prefixes(value: Any) -> None:
        if isinstance(value, str) and len(value) > chain_threshold and chain_delimiter and chain_delimiter in value[1:]:
            if value not in seen_prefixes:
                offset = 0
                while offset < len(value):
                    nxt = value.find(chain_delimiter, offset + 1)
                    if nxt == -1:
                        break
                    prefix = value[:nxt]
                    if prefix in seen_prefixes:
                        duplicate_prefixes.add(prefix)
                    else:
                        seen_prefixes.add(prefix)
                    offset = nxt
        elif isinstance(value, list):
            for item in value:
                scan_prefixes(item)
        elif isinstance(value, dict):
            for k, v in value.items():
                scan_prefixes(k)
                scan_prefixes(v)

    scan_prefixes(root_value)

    def push_string(s: str) -> int:
        nonlocal byte_length
        parts.append(s)
        byte_length += len(s.encode("utf-8"))
        return byte_length

    def write_any(value: Any) -> int:
        nonlocal byte_length
        key = make_key(value)
        ref_key = refs_map.get(key)
        if ref_key is not None:
            return push_string(f"'{ref_key}")
        seen_offset = seen_offsets.get(key)
        if seen_offset is not None:
            delta = byte_length - seen_offset
            seen_cost = seen_costs.get(key, 0)
            pointer_cost = math.ceil(math.log(delta + 1) / math.log(64)) + 1
            if pointer_cost < seen_cost:
                return push_string(_write_unsigned("^", delta))
        before = byte_length
        ret = write_any_inner(value)
        seen_offsets[key] = byte_length
        seen_costs[key] = byte_length - before
        return ret

    def write_any_inner(value: Any) -> int:
        if value is UNDEFINED:
            return push_string("'u")
        if value is True:
            return push_string("'t")
        if value is False:
            return push_string("'f")
        if value is None:
            return push_string("'n")
        if isinstance(value, str):
            return write_string(value)
        if isinstance(value, float):
            return write_number(value)
        if isinstance(value, int):
            return write_number(value)
        if isinstance(value, list):
            return write_array(value)
        if isinstance(value, dict):
            return write_object(value)
        raise TypeError(f"Unsupported value type: {type(value)}")

    def write_string(value: str) -> int:
        nonlocal byte_length
        if chain_delimiter and chain_delimiter in value:
            head = None
            tail = None
            offset = len(value)
            while offset > 0:
                offset = value.rfind(chain_delimiter, 0, offset)
                if offset <= 0:
                    break
                prefix = value[:offset]
                if prefix in duplicate_prefixes:
                    head = prefix
                    tail = value[offset:]
                    break
            if head and tail:
                before = byte_length
                write_any(tail)
                write_any(head)
                return push_string(_write_unsigned(".", byte_length - before))

        utf8 = value.encode("utf-8")
        parts.append(value)
        byte_length += len(utf8)
        return push_string(_write_unsigned(",", len(utf8)))

    def write_number(value: float | int) -> int:
        if isinstance(value, float) and math.isnan(value):
            return push_string("'nan")
        if value == float("inf"):
            return push_string("'inf")
        if value == float("-inf"):
            return push_string("'nif")
        base, exp = split_number(value)
        if exp >= 0 and exp < 5 and isinstance(base, int) and abs(int(value)) <= 2**53 - 1:
            ival = int(value)
            return push_string(_write_signed("+", ival))
        push_string(_write_signed("+", base))
        return push_string(_write_signed("*", exp))

    def write_array(value: list) -> int:
        start = byte_length
        write_values(value)
        return push_string(_write_unsigned(";", byte_length - start))

    def write_values(values: list) -> None:
        nonlocal byte_length
        length = len(values)
        offsets = [0] * length if length > index_threshold else None
        for i in range(length - 1, -1, -1):
            write_any(values[i])
            if offsets is not None:
                offsets[i] = byte_length
        if offsets is not None:
            last_offset = offsets[-1]
            max_delta = byte_length - last_offset
            width = max(1, math.ceil(math.log(max_delta + 1) / math.log(64)))
            pointers = "".join(
                b64_stringify(byte_length - o).zfill(width) for o in offsets
            )
            push_string(pointers)
            push_string(_write_unsigned("#", (length << 3) | (width - 1)))

    def write_object(value: dict) -> int:
        nonlocal byte_length
        keys = list(value.keys())
        length = len(keys)
        if length == 0:
            return push_string(":")

        keys_key = make_key(keys)
        schema_target = schema_offsets.get(keys_key)
        if schema_target is None:
            schema_target = seen_offsets.get(keys_key)
        if schema_target is not None:
            return write_schema_object(value, schema_target)

        before = byte_length
        offsets: dict[str, int] | None = {} if length > index_threshold else None
        last_offset: int | None = None
        entries = list(value.items())
        for i in range(len(entries) - 1, -1, -1):
            key, val = entries[i]
            write_any(val)
            write_any(key)
            if offsets is not None:
                offsets[key] = byte_length
                if last_offset is None:
                    last_offset = byte_length

        if offsets is not None and last_offset is not None:
            max_delta = byte_length - last_offset
            width = max(1, math.ceil(math.log(max_delta + 1) / math.log(64)))
            sorted_entries = sorted(offsets.items(), key=lambda x: utf8_sort_key(x[0]))
            pointers = "".join(
                b64_stringify(byte_length - o).zfill(width) for _, o in sorted_entries
            )
            push_string(pointers)
            push_string(_write_unsigned("#", (length << 3) | (width - 1)))

        ret = push_string(_write_unsigned(":", byte_length - before))
        schema_offsets[keys_key] = byte_length
        return ret

    def write_schema_object(value: dict, target: str | int) -> int:
        nonlocal byte_length
        before = byte_length
        write_values(list(value.values()))
        if isinstance(target, str):
            push_string(f"'{target}")
        else:
            push_string(_write_unsigned("^", byte_length - target))
        return push_string(_write_unsigned(":", byte_length - before))

    write_any(root_value)
    return "".join(parts)
