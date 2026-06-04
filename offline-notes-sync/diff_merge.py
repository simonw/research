"""
Character-Level Diff and Three-Way Merge Algorithm

This module implements a text merging algorithm similar to what Apple Notes
and other collaborative editing apps use. The approach:

1. For simple cases (one side unchanged, both same change), return immediately
2. Use operation-based diffing to identify insertion/deletion positions
3. Apply non-overlapping changes from both sides
4. Mark overlapping changes as conflicts

The key insight is that we can auto-merge concurrent edits if they
affect different character positions in the original text.
"""

from dataclasses import dataclass
from enum import Enum
from typing import Optional


class OpType(Enum):
    INSERT = 'insert'
    DELETE = 'delete'
    RETAIN = 'retain'


@dataclass
class Operation:
    """Represents an edit operation at a specific position."""
    type: OpType
    position: int  # Position in the base text
    text: str  # For INSERT: text to insert; For DELETE: text being deleted


def compute_operations(base: str, modified: str) -> list[Operation]:
    """
    Compute the operations needed to transform base into modified.
    Uses a simple approach based on common prefix/suffix detection
    and treating the middle as a replace operation.
    """
    if base == modified:
        return []

    # Find longest common prefix
    prefix_len = 0
    min_len = min(len(base), len(modified))
    while prefix_len < min_len and base[prefix_len] == modified[prefix_len]:
        prefix_len += 1

    # Find longest common suffix (not overlapping with prefix)
    suffix_len = 0
    max_suffix = min(len(base), len(modified)) - prefix_len
    while suffix_len < max_suffix:
        if base[-(suffix_len + 1)] != modified[-(suffix_len + 1)]:
            break
        suffix_len += 1

    # The middle part differs
    base_end = len(base) - suffix_len
    mod_end = len(modified) - suffix_len

    deleted = base[prefix_len:base_end]
    inserted = modified[prefix_len:mod_end]

    ops = []
    if deleted:
        ops.append(Operation(OpType.DELETE, prefix_len, deleted))
    if inserted:
        ops.append(Operation(OpType.INSERT, prefix_len, inserted))

    return ops


def operations_overlap(ops1: list[Operation], ops2: list[Operation], base_len: int) -> bool:
    """
    Check if two sets of operations affect overlapping regions of the base text.
    """
    # Build affected ranges for each set
    def get_affected_range(ops: list[Operation]) -> tuple[int, int]:
        if not ops:
            return (base_len, 0)  # No affected range

        min_pos = base_len
        max_pos = 0
        for op in ops:
            if op.type == OpType.DELETE:
                min_pos = min(min_pos, op.position)
                max_pos = max(max_pos, op.position + len(op.text))
            elif op.type == OpType.INSERT:
                min_pos = min(min_pos, op.position)
                max_pos = max(max_pos, op.position)

        return (min_pos, max_pos)

    r1_start, r1_end = get_affected_range(ops1)
    r2_start, r2_end = get_affected_range(ops2)

    # Check for overlap
    return not (r1_end <= r2_start or r2_end <= r1_start)


def apply_operations(base: str, ops: list[Operation]) -> str:
    """Apply a list of operations to the base string."""
    if not ops:
        return base

    result = base

    # Sort by position (descending) to apply from end to start
    # This prevents position shifts from affecting other operations
    sorted_ops = sorted(ops, key=lambda o: o.position, reverse=True)

    for op in sorted_ops:
        pos = op.position
        if op.type == OpType.DELETE:
            result = result[:pos] + result[pos + len(op.text):]
        elif op.type == OpType.INSERT:
            result = result[:pos] + op.text + result[pos:]

    return result


def merge_operations(base: str, local_ops: list[Operation], remote_ops: list[Operation]) -> tuple[str, bool]:
    """
    Merge two sets of operations on the same base text.
    Returns (merged_result, had_conflict).
    """
    if not local_ops and not remote_ops:
        return base, False

    if not local_ops:
        return apply_operations(base, remote_ops), False

    if not remote_ops:
        return apply_operations(base, local_ops), False

    # Check if operations affect the same region
    if operations_overlap(local_ops, remote_ops, len(base)):
        # Overlapping changes - this is a conflict
        # But first, check if the changes result in the same text
        local_result = apply_operations(base, local_ops)
        remote_result = apply_operations(base, remote_ops)

        if local_result == remote_result:
            return local_result, False

        # True conflict - create conflict markers
        return f"<<<<<<< LOCAL\n{local_result}\n=======\n{remote_result}\n>>>>>>> REMOTE", True

    # Non-overlapping - we can apply both
    # Need to transform operations to account for position shifts
    # Apply local ops first, then transform and apply remote ops

    # Sort all operations by position
    all_ops_sorted = sorted(local_ops + remote_ops, key=lambda o: o.position, reverse=True)

    # Apply all operations
    return apply_operations(base, all_ops_sorted), False


def merge_texts(base: str, local: str, remote: str) -> tuple[str, bool]:
    """
    Main entry point for merging two versions of text.

    Args:
        base: The common ancestor text
        local: The local version
        remote: The remote version

    Returns:
        (merged_text, had_conflict)
    """
    # Handle None/empty
    base = base or ''
    local = local or ''
    remote = remote or ''

    # Trivial cases
    if local == remote:
        return local, False
    if base == local:
        return remote, False
    if base == remote:
        return local, False

    # Check if multi-line - use line-based merge for better results
    if '\n' in base or '\n' in local or '\n' in remote:
        return merge_multiline(base, local, remote)

    # Compute operations for each side
    local_ops = compute_operations(base, local)
    remote_ops = compute_operations(base, remote)

    return merge_operations(base, local_ops, remote_ops)


def merge_multiline(base: str, local: str, remote: str) -> tuple[str, bool]:
    """
    Merge multi-line text using line-based approach with character-level
    merge for individual line conflicts.
    """
    base_lines = base.split('\n')
    local_lines = local.split('\n')
    remote_lines = remote.split('\n')

    # Use diff to find matching lines
    base_local_matches = find_line_matches(base_lines, local_lines)
    base_remote_matches = find_line_matches(base_lines, remote_lines)

    result_lines = []
    has_conflict = False

    i = j = k = 0

    while i < len(base_lines) or j < len(local_lines) or k < len(remote_lines):
        base_line = base_lines[i] if i < len(base_lines) else None
        local_line = local_lines[j] if j < len(local_lines) else None
        remote_line = remote_lines[k] if k < len(remote_lines) else None

        # All three match
        if base_line is not None and base_line == local_line == remote_line:
            result_lines.append(base_line)
            i += 1
            j += 1
            k += 1
            continue

        # Only local changed this line
        if base_line == remote_line and local_line != base_line:
            if local_line is not None:
                result_lines.append(local_line)
            j += 1
            if base_line is not None:
                i += 1
                k += 1
            continue

        # Only remote changed this line
        if base_line == local_line and remote_line != base_line:
            if remote_line is not None:
                result_lines.append(remote_line)
            k += 1
            if base_line is not None:
                i += 1
                j += 1
            continue

        # Both changed to same thing
        if local_line == remote_line:
            if local_line is not None:
                result_lines.append(local_line)
            j += 1
            k += 1
            if base_line is not None:
                i += 1
            continue

        # Both changed differently - try to merge the lines
        if base_line is not None and local_line is not None and remote_line is not None:
            # Use character-level merge for this line
            merged_line, line_conflict = merge_single_line(base_line, local_line, remote_line)
            if not line_conflict:
                result_lines.append(merged_line)
                i += 1
                j += 1
                k += 1
                continue

        # Cannot auto-merge - create conflict
        has_conflict = True
        result_lines.append("<<<<<<< LOCAL")

        # Collect local's version
        local_conflict_lines = []
        while j < len(local_lines):
            if i < len(base_lines) and local_lines[j] == base_lines[i]:
                break
            local_conflict_lines.append(local_lines[j])
            j += 1

        result_lines.extend(local_conflict_lines)
        result_lines.append("=======")

        # Collect remote's version
        while k < len(remote_lines):
            if i < len(base_lines) and remote_lines[k] == base_lines[i]:
                break
            result_lines.append(remote_lines[k])
            k += 1

        result_lines.append(">>>>>>> REMOTE")

        if i < len(base_lines):
            i += 1

    return '\n'.join(result_lines), has_conflict


def merge_single_line(base: str, local: str, remote: str) -> tuple[str, bool]:
    """
    Merge a single line using character-level operations.
    """
    # Trivial cases
    if local == remote:
        return local, False
    if base == local:
        return remote, False
    if base == remote:
        return local, False

    local_ops = compute_operations(base, local)
    remote_ops = compute_operations(base, remote)

    return merge_operations(base, local_ops, remote_ops)


def find_line_matches(old_lines: list[str], new_lines: list[str]) -> list[tuple[int, int]]:
    """
    Find matching lines between old and new using LCS.
    Returns list of (old_idx, new_idx) pairs.
    """
    m, n = len(old_lines), len(new_lines)
    dp = [[0] * (n + 1) for _ in range(m + 1)]

    for i in range(1, m + 1):
        for j in range(1, n + 1):
            if old_lines[i-1] == new_lines[j-1]:
                dp[i][j] = dp[i-1][j-1] + 1
            else:
                dp[i][j] = max(dp[i-1][j], dp[i][j-1])

    # Backtrack
    matches = []
    i, j = m, n
    while i > 0 and j > 0:
        if old_lines[i-1] == new_lines[j-1]:
            matches.append((i-1, j-1))
            i -= 1
            j -= 1
        elif dp[i-1][j] > dp[i][j-1]:
            i -= 1
        else:
            j -= 1

    matches.reverse()
    return matches


# Additional exports for compatibility
class EditType(Enum):
    EQUAL = 'equal'
    INSERT = 'insert'
    DELETE = 'delete'


@dataclass
class Edit:
    """Represents a single edit operation."""
    type: EditType
    text: str
    position: int


def compute_diff(old: str, new: str) -> list[Edit]:
    """
    Compute the character-level diff between two strings.
    Returns a list of Edit operations.
    """
    if old == new:
        return [Edit(EditType.EQUAL, old, 0)] if old else []

    edits = []
    ops = compute_operations(old, new)

    # Convert to Edit format
    for op in ops:
        if op.type == OpType.DELETE:
            edits.append(Edit(EditType.DELETE, op.text, op.position))
        elif op.type == OpType.INSERT:
            edits.append(Edit(EditType.INSERT, op.text, op.position))

    return edits


# Test the implementation
if __name__ == '__main__':
    print("=== Test 1: Non-overlapping insertions ===")
    base = "Hello World"
    local = "Hello Beautiful World"  # Insert "Beautiful " at pos 6
    remote = "Hello World!"  # Insert "!" at pos 11
    merged, conflict = merge_texts(base, local, remote)
    print(f"Base: '{base}'")
    print(f"Local: '{local}'")
    print(f"Remote: '{remote}'")
    print(f"Merged: '{merged}'")
    print(f"Conflict: {conflict}")
    print(f"Expected: 'Hello Beautiful World!'")
    print()

    print("=== Test 2: Same edit ===")
    base = "Hello World"
    local = "Hello Universe"
    remote = "Hello Universe"
    merged, conflict = merge_texts(base, local, remote)
    print(f"Base: '{base}'")
    print(f"Local: '{local}'")
    print(f"Remote: '{remote}'")
    print(f"Merged: '{merged}'")
    print(f"Conflict: {conflict}")
    print()

    print("=== Test 3: Only local changed ===")
    base = "Hello World"
    local = "Hello Beautiful World"
    remote = "Hello World"
    merged, conflict = merge_texts(base, local, remote)
    print(f"Base: '{base}'")
    print(f"Local: '{local}'")
    print(f"Remote: '{remote}'")
    print(f"Merged: '{merged}'")
    print(f"Conflict: {conflict}")
    print()

    print("=== Test 4: Multi-line merge (non-conflicting) ===")
    base = "Line 1\nLine 2\nLine 3"
    local = "Line 1\nLine 2 modified\nLine 3"
    remote = "Line 1\nLine 2\nLine 3 changed"
    merged, conflict = merge_texts(base, local, remote)
    print(f"Base:\n{base}")
    print(f"Local:\n{local}")
    print(f"Remote:\n{remote}")
    print(f"Merged:\n{merged}")
    print(f"Conflict: {conflict}")
    print()

    print("=== Test 5: Prefix/suffix edits ===")
    base = "The quick brown fox"
    local = "A quick brown fox"  # Changed prefix
    remote = "The quick brown dog"  # Changed suffix
    merged, conflict = merge_texts(base, local, remote)
    print(f"Base: '{base}'")
    print(f"Local: '{local}'")
    print(f"Remote: '{remote}'")
    print(f"Merged: '{merged}'")
    print(f"Conflict: {conflict}")
    print(f"Expected: 'A quick brown dog'")
    print()

    print("=== Test 6: Overlapping edits (conflict) ===")
    base = "Hello World"
    local = "Hello Earth"  # Changed "World" to "Earth"
    remote = "Hello Mars"  # Changed "World" to "Mars"
    merged, conflict = merge_texts(base, local, remote)
    print(f"Base: '{base}'")
    print(f"Local: '{local}'")
    print(f"Remote: '{remote}'")
    print(f"Merged: '{merged}'")
    print(f"Conflict: {conflict}")
    print(f"Expected: conflict markers")
