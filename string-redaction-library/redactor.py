#!/usr/bin/env python3
"""
String Redaction Library

Detects potential secrets in text based on statistical analysis
of character patterns that deviate from normal English words.
"""

from typing import List, Dict
import re


# Vowels including y as sometimes vowel
VOWELS = set("aeiouAEIOU")
CONSONANTS = set("bcdfghjklmnpqrstvwxyzBCDFGHJKLMNPQRSTVWXYZ")

# Minimum length for a string to be considered a potential secret
MIN_SECRET_LENGTH = 9


def get_vowel_ratio(text: str) -> float:
    """Calculate the ratio of vowels to total letters in text."""
    letters = [c for c in text if c.isalpha()]
    if not letters:
        return 0.0
    vowel_count = sum(1 for c in letters if c in VOWELS)
    return vowel_count / len(letters)


def get_digit_ratio(text: str) -> float:
    """Calculate the ratio of digits to total alphanumeric characters."""
    alnum = [c for c in text if c.isalnum()]
    if not alnum:
        return 0.0
    digit_count = sum(1 for c in text if c.isdigit())
    return digit_count / len(alnum)


def has_mixed_case(text: str) -> bool:
    """Check if text has both upper and lower case letters."""
    has_upper = any(c.isupper() for c in text)
    has_lower = any(c.islower() for c in text)
    return has_upper and has_lower


def max_consonant_sequence(text: str) -> int:
    """Find the longest sequence of consecutive consonants."""
    max_seq = 0
    current_seq = 0
    for c in text.lower():
        if c in "bcdfghjklmnpqrstvwxyz":
            current_seq += 1
            max_seq = max(max_seq, current_seq)
        else:
            current_seq = 0
    return max_seq


def max_same_type_sequence(text: str) -> int:
    """Find longest sequence of same character type (vowels or consonants)."""
    max_seq = 0
    current_seq = 0
    last_type = None

    for c in text.lower():
        if c.isalpha():
            is_vowel = c in "aeiou"
            if last_type == is_vowel:
                current_seq += 1
            else:
                current_seq = 1
                last_type = is_vowel
            max_seq = max(max_seq, current_seq)
        else:
            current_seq = 0
            last_type = None

    return max_seq


def is_likely_secret(token: str) -> bool:
    """
    Determine if a token is likely a secret based on statistical patterns.

    Returns True if the token appears to be a secret (non-English-like).
    """
    # Must be longer than minimum length
    if len(token) <= 8:
        return False

    # Pure numbers are not secrets in our context
    if token.isdigit():
        return False

    # Get letter-only portion for analysis
    letters_only = "".join(c for c in token if c.isalpha())

    # If very few letters, check digit mixing pattern
    if len(letters_only) < 3:
        # Mixed alphanumeric with few letters - likely a secret
        if len(token) > 8 and any(c.isdigit() for c in token):
            return True
        return False

    vowel_ratio = get_vowel_ratio(token)
    digit_ratio = get_digit_ratio(token)
    mixed_case = has_mixed_case(token)
    max_consonants = max_consonant_sequence(token)
    max_same = max_same_type_sequence(token)

    # Check if this looks like a simple English word
    # (all lowercase or all uppercase, no digits, reasonable length)
    is_simple_word = (
        (token.islower() or token.isupper()) and
        digit_ratio == 0 and
        len(token) <= 12
    )

    # Scoring system - accumulate "secret-like" indicators
    secret_score = 0

    # Vowel ratio analysis
    # Normal English: ~38-42% vowels
    # Very low vowels (< 20%) suggests random consonant strings
    if vowel_ratio < 0.15:
        # Less penalty for simple words - English has words like "strengths"
        secret_score += 1 if is_simple_word else 3
    elif vowel_ratio < 0.25:
        secret_score += 1 if is_simple_word else 2
    elif vowel_ratio < 0.30:
        secret_score += 0 if is_simple_word else 1

    # Very high vowels (> 60%) is also unusual
    if vowel_ratio > 0.70:
        secret_score += 3
    elif vowel_ratio > 0.55:
        secret_score += 1

    # Mixed digits with letters is a strong indicator
    if digit_ratio > 0.1 and digit_ratio < 0.9:
        secret_score += 2

    # Mixed case (camelCase excluded later) can indicate secrets
    if mixed_case and digit_ratio > 0:
        secret_score += 1

    # Long consonant sequences (> 4) are unusual in English
    # "strengths" has 5 consonants but that's rare
    if max_consonants > 6:
        secret_score += 2
    elif max_consonants > 5:
        # Don't penalize simple words as much - "strengths" etc
        secret_score += 0 if is_simple_word else 1

    # Very long same-type sequences
    if max_same > 6:
        secret_score += 2

    # Check for camelCase pattern (likely code, not secret)
    # Pattern: lowercase followed by uppercase, multiple times
    camel_transitions = len(re.findall(r"[a-z][A-Z]", token))
    if camel_transitions >= 1 and digit_ratio == 0:
        # Likely camelCase code identifier
        secret_score -= 2

    # Check for common English word patterns
    # Ending patterns common in English
    lower_token = token.lower()
    english_endings = ["tion", "sion", "ment", "ness", "able", "ible", "ful", "less", "ing", "ous", "ive", "ant", "ent", "ths"]
    for ending in english_endings:
        if lower_token.endswith(ending):
            secret_score -= 2
            break

    # Common prefixes
    english_prefixes = ["un", "re", "pre", "dis", "mis", "over", "under", "out", "sub", "super", "inter", "trans", "str"]
    for prefix in english_prefixes:
        if lower_token.startswith(prefix) and len(lower_token) > len(prefix) + 3:
            secret_score -= 1
            break

    # Threshold for determining if it's a secret
    return secret_score >= 2


def extract_tokens(text: str) -> List[Dict]:
    """
    Extract alphanumeric tokens from text with their positions.

    Returns list of dicts with 'value', 'start', 'end' keys.
    """
    tokens = []
    # Match sequences of alphanumeric characters and underscores
    # But we'll primarily analyze the alphanumeric parts
    pattern = r"[a-zA-Z0-9_]+"

    for match in re.finditer(pattern, text):
        token = match.group()
        # Skip if it has leading/trailing underscores for the value
        # but keep position accurate
        start = match.start()
        end = match.end()

        tokens.append({
            "value": token,
            "start": start,
            "end": end
        })

    return tokens


def detect_secrets(text: str) -> List[Dict[str, any]]:
    """
    Scan text for potential secrets.

    A potential secret is a string longer than 8 characters that
    appears statistically unlikely to be a real English word based
    on consonant/vowel patterns.

    Args:
        text: The input text to scan

    Returns:
        List of dicts with keys:
        - value: the detected secret string
        - start: starting index in the text
        - end: ending index in the text (exclusive)
    """
    if not text:
        return []

    tokens = extract_tokens(text)
    secrets = []

    for token_info in tokens:
        token = token_info["value"]

        # Skip tokens that are too short
        if len(token) <= 8:
            continue

        # Remove underscores for analysis but keep them in value
        clean_token = token.replace("_", "")

        if len(clean_token) > 8 and is_likely_secret(clean_token):
            secrets.append({
                "value": token,
                "start": token_info["start"],
                "end": token_info["end"]
            })

    return secrets
