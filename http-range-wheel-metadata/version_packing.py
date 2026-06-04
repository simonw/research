#!/usr/bin/env python3
"""
Compact PEP 440 version representation using bit packing.

This replicates the mechanism used by uv (https://github.com/astral-sh/uv)
to pack version numbers into a single u64 (8 bytes) for fast comparison.

The bit layout in repr (u64):
- Bits 48-63 (16 bits): First release segment (u16, max 65535)
- Bits 40-47 (8 bits): Second release segment (u8, max 255)
- Bits 32-39 (8 bits): Third release segment (u8, max 255)
- Bits 24-31 (8 bits): Fourth release segment (u8, max 255)
- Bits 20-23 (4 bits): Suffix kind (dev, alpha, beta, rc, none, post, etc.)
- Bits 0-19 (20 bits): Suffix version number (max 1048575)

Over 90% of PyPI versions fit in this compact representation!
Versions that don't fit fall back to a "full" representation.

Benefits:
- Version comparison is a single integer comparison
- Hashing is fast (hash the integer)
- Memory efficient (16 bytes vs variable size strings)
- Cache friendly
"""

import re
from dataclasses import dataclass
from enum import IntEnum
from typing import Optional, Tuple, Union


class SuffixKind(IntEnum):
    """Suffix kinds ordered for correct version comparison."""
    MIN = 0      # Special: sorts before everything (for lower bounds)
    DEV = 1      # Development release: 1.0.dev0
    ALPHA = 2    # Alpha pre-release: 1.0a0
    BETA = 3     # Beta pre-release: 1.0b0
    RC = 4       # Release candidate: 1.0rc0
    NONE = 5     # Final release: 1.0
    LOCAL = 6    # Local version: 1.0+local (not supported in small repr)
    POST = 7     # Post release: 1.0.post1
    MAX = 8      # Special: sorts after everything (for upper bounds)


@dataclass
class Prerelease:
    kind: str  # 'a', 'b', 'rc'
    number: int


@dataclass
class VersionFull:
    """Full version representation for versions that don't fit in u64."""
    epoch: int
    release: Tuple[int, ...]
    pre: Optional[Prerelease]
    post: Optional[int]
    dev: Optional[int]
    local: Optional[str]


class VersionSmall:
    """
    Compact version representation packed into a u64.

    This mirrors uv's VersionSmall struct from version.rs.
    """

    # Masks and bit positions
    SUFFIX_RELEASE_MASK = 0xFFFF_FFFF_FF00_0000
    SUFFIX_VERSION_MASK = 0x000F_FFFF
    SUFFIX_VERSION_BIT_LEN = 20
    SUFFIX_KIND_MASK = 0b1111

    def __init__(self, repr_val: int = 0, length: int = 0):
        # Default to NONE suffix
        if repr_val == 0:
            repr_val = SuffixKind.NONE << self.SUFFIX_VERSION_BIT_LEN
        self.repr = repr_val
        self.len = length

    @classmethod
    def from_release(cls, segments: Tuple[int, ...]) -> Optional['VersionSmall']:
        """
        Create a VersionSmall from release segments.
        Returns None if segments don't fit.
        """
        if len(segments) > 4:
            return None
        if len(segments) == 0:
            return cls()

        # First segment must fit in u16
        if segments[0] > 0xFFFF:
            return None

        # Other segments must fit in u8
        for seg in segments[1:]:
            if seg > 0xFF:
                return None

        small = cls()
        small.len = 0

        for seg in segments:
            if not small.push_release(seg):
                return None

        return small

    def push_release(self, n: int) -> bool:
        """Add a release segment. Returns False if it doesn't fit."""
        if self.len == 0:
            if n > 0xFFFF:
                return False
            self.repr |= n << 48
            self.len = 1
            return True
        else:
            if n > 0xFF:
                return False
            if self.len >= 4:
                return False
            shift = 48 - (self.len * 8)
            self.repr |= n << shift
            self.len += 1
            return True

    @property
    def release(self) -> Tuple[int, ...]:
        """Extract release segments from packed representation."""
        if self.len == 0:
            return ()
        elif self.len == 1:
            return ((self.repr >> 48) & 0xFFFF,)
        elif self.len == 2:
            return (
                (self.repr >> 48) & 0xFFFF,
                (self.repr >> 40) & 0xFF,
            )
        elif self.len == 3:
            return (
                (self.repr >> 48) & 0xFFFF,
                (self.repr >> 40) & 0xFF,
                (self.repr >> 32) & 0xFF,
            )
        else:  # len == 4
            return (
                (self.repr >> 48) & 0xFFFF,
                (self.repr >> 40) & 0xFF,
                (self.repr >> 32) & 0xFF,
                (self.repr >> 24) & 0xFF,
            )

    @property
    def suffix_kind(self) -> SuffixKind:
        """Get the suffix kind from the packed representation."""
        return SuffixKind((self.repr >> self.SUFFIX_VERSION_BIT_LEN) & self.SUFFIX_KIND_MASK)

    @suffix_kind.setter
    def suffix_kind(self, kind: SuffixKind):
        """Set the suffix kind in the packed representation."""
        # Clear existing suffix kind bits
        self.repr &= ~(self.SUFFIX_KIND_MASK << self.SUFFIX_VERSION_BIT_LEN)
        # Set new suffix kind
        self.repr |= (kind & self.SUFFIX_KIND_MASK) << self.SUFFIX_VERSION_BIT_LEN

    @property
    def suffix_version(self) -> int:
        """Get the suffix version number."""
        return self.repr & self.SUFFIX_VERSION_MASK

    @suffix_version.setter
    def suffix_version(self, version: int):
        """Set the suffix version number."""
        if version > self.SUFFIX_VERSION_MASK:
            raise ValueError(f"Suffix version {version} exceeds maximum {self.SUFFIX_VERSION_MASK}")
        self.repr &= ~self.SUFFIX_VERSION_MASK
        self.repr |= version & self.SUFFIX_VERSION_MASK

    @property
    def pre(self) -> Optional[Prerelease]:
        """Get pre-release info if present."""
        kind = self.suffix_kind
        if kind == SuffixKind.ALPHA:
            return Prerelease('a', self.suffix_version)
        elif kind == SuffixKind.BETA:
            return Prerelease('b', self.suffix_version)
        elif kind == SuffixKind.RC:
            return Prerelease('rc', self.suffix_version)
        return None

    def set_pre(self, pre: Optional[Prerelease]) -> bool:
        """Set pre-release. Returns False if current suffix conflicts."""
        current = self.suffix_kind
        if current not in (SuffixKind.NONE, SuffixKind.ALPHA, SuffixKind.BETA, SuffixKind.RC):
            return pre is None

        if pre is None:
            self.suffix_kind = SuffixKind.NONE
            return True

        if pre.number > self.SUFFIX_VERSION_MASK:
            return False

        if pre.kind == 'a':
            self.suffix_kind = SuffixKind.ALPHA
        elif pre.kind == 'b':
            self.suffix_kind = SuffixKind.BETA
        elif pre.kind == 'rc':
            self.suffix_kind = SuffixKind.RC
        else:
            return False

        self.suffix_version = pre.number
        return True

    @property
    def post(self) -> Optional[int]:
        """Get post-release number if present."""
        if self.suffix_kind == SuffixKind.POST:
            return self.suffix_version
        return None

    def set_post(self, post: Optional[int]) -> bool:
        """Set post-release. Returns False if current suffix conflicts."""
        current = self.suffix_kind
        if current not in (SuffixKind.NONE, SuffixKind.POST):
            return post is None

        if post is None:
            self.suffix_kind = SuffixKind.NONE
            return True

        if post > self.SUFFIX_VERSION_MASK:
            return False

        self.suffix_kind = SuffixKind.POST
        self.suffix_version = post
        return True

    @property
    def dev(self) -> Optional[int]:
        """Get dev-release number if present."""
        if self.suffix_kind == SuffixKind.DEV:
            return self.suffix_version
        return None

    def set_dev(self, dev: Optional[int]) -> bool:
        """Set dev-release. Returns False if current suffix conflicts."""
        current = self.suffix_kind
        if current not in (SuffixKind.NONE, SuffixKind.DEV):
            return dev is None

        if dev is None:
            self.suffix_kind = SuffixKind.NONE
            return True

        if dev > self.SUFFIX_VERSION_MASK:
            return False

        self.suffix_kind = SuffixKind.DEV
        self.suffix_version = dev
        return True

    def __repr__(self) -> str:
        return f"VersionSmall(repr=0x{self.repr:016x}, len={self.len})"

    def __str__(self) -> str:
        """Convert to PEP 440 string representation."""
        parts = ['.'.join(str(x) for x in self.release)]

        pre = self.pre
        if pre:
            parts.append(f"{pre.kind}{pre.number}")

        post = self.post
        if post is not None:
            parts.append(f".post{post}")

        dev = self.dev
        if dev is not None:
            parts.append(f".dev{dev}")

        return ''.join(parts)

    def __eq__(self, other: 'VersionSmall') -> bool:
        return self.repr == other.repr and self.len == other.len

    def __lt__(self, other: 'VersionSmall') -> bool:
        # Compare release segments first, then suffix
        # The repr is designed so that integer comparison works correctly!
        # This is the key optimization.
        return self.repr < other.repr

    def __le__(self, other: 'VersionSmall') -> bool:
        return self.repr <= other.repr

    def __gt__(self, other: 'VersionSmall') -> bool:
        return self.repr > other.repr

    def __ge__(self, other: 'VersionSmall') -> bool:
        return self.repr >= other.repr

    def __hash__(self) -> int:
        return hash((self.repr, self.len))


class Version:
    """
    PEP 440 version with automatic small/full representation.

    Uses the compact u64 representation when possible, falls back
    to full representation for complex versions.
    """

    # Regex for parsing PEP 440 versions
    VERSION_RE = re.compile(
        r'^'
        r'(?:(?P<epoch>[0-9]+)!)?'                    # epoch
        r'(?P<release>[0-9]+(?:\.[0-9]+)*)'           # release
        r'(?:(?P<pre_type>a|b|rc)(?P<pre_num>[0-9]+))?'  # pre
        r'(?:\.post(?P<post>[0-9]+))?'                # post
        r'(?:\.dev(?P<dev>[0-9]+))?'                  # dev
        r'(?:\+(?P<local>[a-zA-Z0-9._]+))?'           # local
        r'$'
    )

    def __init__(self, version_str: str):
        self._str = version_str
        self._small: Optional[VersionSmall] = None
        self._full: Optional[VersionFull] = None

        self._parse(version_str)

    def _parse(self, version_str: str):
        match = self.VERSION_RE.match(version_str)
        if not match:
            raise ValueError(f"Invalid version: {version_str}")

        epoch = int(match.group('epoch') or 0)
        release = tuple(int(x) for x in match.group('release').split('.'))
        pre = None
        if match.group('pre_type'):
            pre = Prerelease(match.group('pre_type'), int(match.group('pre_num')))
        post = int(match.group('post')) if match.group('post') else None
        dev = int(match.group('dev')) if match.group('dev') else None
        local = match.group('local')

        # Try to fit in small representation
        can_be_small = (
            epoch == 0 and
            len(release) <= 4 and
            local is None and
            # Only one suffix allowed in small repr
            sum([pre is not None, post is not None, dev is not None]) <= 1
        )

        if can_be_small:
            small = VersionSmall.from_release(release)
            if small is not None:
                success = True
                if pre is not None:
                    success = small.set_pre(pre)
                elif post is not None:
                    success = small.set_post(post)
                elif dev is not None:
                    success = small.set_dev(dev)

                if success:
                    self._small = small
                    return

        # Fall back to full representation
        self._full = VersionFull(
            epoch=epoch,
            release=release,
            pre=pre,
            post=post,
            dev=dev,
            local=local,
        )

    @property
    def is_small(self) -> bool:
        """Returns True if using compact representation."""
        return self._small is not None

    @property
    def release(self) -> Tuple[int, ...]:
        if self._small:
            return self._small.release
        return self._full.release

    @property
    def pre(self) -> Optional[Prerelease]:
        if self._small:
            return self._small.pre
        return self._full.pre

    @property
    def post(self) -> Optional[int]:
        if self._small:
            return self._small.post
        return self._full.post

    @property
    def dev(self) -> Optional[int]:
        if self._small:
            return self._small.dev
        return self._full.dev

    def __repr__(self) -> str:
        if self._small:
            return f"Version({self._str!r}, small={self._small})"
        return f"Version({self._str!r}, full)"

    def __str__(self) -> str:
        return self._str

    def __eq__(self, other: 'Version') -> bool:
        # If both are small, use fast integer comparison
        if self._small and other._small:
            return self._small == other._small
        # Fall back to string comparison
        return str(self) == str(other)

    def __lt__(self, other: 'Version') -> bool:
        # If both are small, use fast integer comparison
        if self._small and other._small:
            return self._small < other._small
        # Fall back to tuple comparison
        return self._cmp_key() < other._cmp_key()

    def _cmp_key(self):
        """Comparison key for full representation."""
        # This implements PEP 440 ordering
        pre_key = (0, self.pre.kind, self.pre.number) if self.pre else (1,)
        post_key = (1, self.post) if self.post is not None else (0,)
        dev_key = (0, self.dev) if self.dev is not None else (1,)
        return (self.release, pre_key, post_key, dev_key)

    def __hash__(self) -> int:
        if self._small:
            return hash(self._small)
        return hash(self._str)


def explain_packing(version_str: str):
    """Show how a version is packed into a u64."""
    v = Version(version_str)

    print(f"\nVersion: {version_str}")
    print(f"Representation: {'Small (u64)' if v.is_small else 'Full'}")

    if v._small:
        s = v._small
        print(f"\nBit Layout (u64 = 0x{s.repr:016x}):")
        print("  ┌─────────────────────────────────────────────────────────────────┐")
        print("  │ 63-48 (16b)  47-40 (8b)  39-32 (8b)  31-24 (8b)  23-20  19-0    │")
        print("  │ Release[0]   Release[1]  Release[2]  Release[3]  Kind   Version │")
        print("  └─────────────────────────────────────────────────────────────────┘")
        print()

        # Extract each field
        r0 = (s.repr >> 48) & 0xFFFF
        r1 = (s.repr >> 40) & 0xFF
        r2 = (s.repr >> 32) & 0xFF
        r3 = (s.repr >> 24) & 0xFF
        suffix_kind = (s.repr >> 20) & 0xF
        suffix_ver = s.repr & 0xFFFFF

        print(f"  Release segments: {r0}.{r1}.{r2}.{r3} (using {s.len} segments)")
        print(f"  Suffix kind: {suffix_kind} ({SuffixKind(suffix_kind).name})")
        print(f"  Suffix version: {suffix_ver}")
        print()

        # Binary representation
        binary = format(s.repr, '064b')
        print("  Binary breakdown:")
        print(f"    {binary[:16]} | {binary[16:24]} | {binary[24:32]} | {binary[32:40]} | {binary[40:44]} | {binary[44:]}")
        print(f"    {'Release[0]':^16} | {'R[1]':^8} | {'R[2]':^8} | {'R[3]':^8} | Kind | {'Suffix Version':^20}")
    else:
        print(f"\n  Epoch: {v._full.epoch}")
        print(f"  Release: {v._full.release}")
        print(f"  Pre: {v._full.pre}")
        print(f"  Post: {v._full.post}")
        print(f"  Dev: {v._full.dev}")
        print(f"  Local: {v._full.local}")
        print("\n  Cannot fit in u64 due to:")
        if v._full.epoch != 0:
            print("    - Non-zero epoch")
        if len(v._full.release) > 4:
            print("    - More than 4 release segments")
        if v._full.local:
            print("    - Local version component")
        if sum([v._full.pre is not None, v._full.post is not None, v._full.dev is not None]) > 1:
            print("    - Multiple suffixes (pre + post, post + dev, etc.)")


def demo_comparison():
    """Demonstrate fast version comparison."""
    versions = [
        "1.0.0",
        "1.0.0a1",
        "1.0.0b1",
        "1.0.0rc1",
        "1.0.0.post1",
        "1.0.1",
        "2.0.0.dev1",
        "2.0.0",
    ]

    print("\n" + "=" * 60)
    print("Version Comparison Demo")
    print("=" * 60)

    parsed = [Version(v) for v in versions]

    print("\nOriginal order:")
    for v in parsed:
        print(f"  {v} (repr=0x{v._small.repr:016x})")

    sorted_versions = sorted(parsed)

    print("\nSorted order (by integer comparison of packed u64):")
    for v in sorted_versions:
        print(f"  {v} (repr=0x{v._small.repr:016x})")

    # Show that sorting is just integer comparison
    print("\nNote: The hex values increase monotonically because the bit layout")
    print("is designed so that integer comparison gives correct PEP 440 ordering!")


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description='Demonstrate compact PEP 440 version packing',
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument('versions', nargs='*', help='Version strings to analyze')
    parser.add_argument('--demo', action='store_true', help='Run comparison demo')

    args = parser.parse_args()

    if args.demo or not args.versions:
        demo_comparison()

    for v in args.versions:
        explain_packing(v)


if __name__ == '__main__':
    main()
