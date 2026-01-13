# String Redaction Library

A Python library for detecting potential secrets in text using statistical analysis of character patterns that deviate from normal English words.

## Overview

This library scans text for strings that are statistically unlikely to be real English words. It detects:
- API keys and tokens
- Random alphanumeric strings
- Base64-encoded data
- MD5/SHA hashes
- Other secret-like patterns

The detection is based on vowel/consonant ratio analysis, digit distribution, and character pattern scoring - without requiring pattern matching for specific secret formats.

## Installation

```bash
pip install pyyaml  # Required dependency
```

## Usage

```python
from redactor import detect_secrets

text = "The API key is xK9mNpQrStVwXyZb and the password is hunter2"
secrets = detect_secrets(text)

for secret in secrets:
    print(f"Found: {secret['value']} at positions {secret['start']}-{secret['end']}")
```

### Output Format

Returns a list of dictionaries, each containing:
- `value`: The detected secret string
- `start`: Starting index in the input text
- `end`: Ending index (exclusive) in the input text

## Detection Algorithm

The library uses a scoring system to identify potential secrets:

### Positive Indicators (increase secret score)
- **Low vowel ratio (<30%)**: Suggests random consonant strings
- **High vowel ratio (>55%)**: Unusual for English
- **Mixed digits with letters**: Strong indicator of generated tokens
- **Long consonant sequences (>5)**: Rare in natural English

### Negative Indicators (decrease secret score)
- **CamelCase pattern**: Likely programming identifier
- **Common English endings**: -tion, -ment, -ness, -able, etc.
- **Common English prefixes**: un-, re-, pre-, dis-, etc.
- **Simple word patterns**: All lowercase/uppercase with no digits

A token is flagged as a potential secret if its score reaches the threshold of 2.

## Test-Driven Development

The library was built using TDD with YAML-based test cases for cross-language portability.

### Running Tests

```bash
python test_runner.py
```

### Test Case Format

Tests are defined in `tests.yaml`:

```yaml
test_cases:
  - name: "test_name"
    input: "text to scan"
    expected:
      - value: "detected_secret"
        start: 0
        end: 15
    description: "What this test verifies"
```

## Examples

### Detected as Secrets
```
xK9mNpQrStVwXyZb        # Random alphanumeric
zxcvbnmqwrtyp           # Consonant-heavy
d41d8cd98f00b204e9800998ecf8427e  # MD5 hash
aeiouaeiouaei           # Vowel-heavy nonsense
```

### NOT Detected (Normal English/Code)
```
extraordinary           # Normal English word
strengths               # English word with consonant cluster
getUserInformation      # CamelCase identifier
calculate_total         # Snake_case identifier
```

## Limitations

1. **Well-crafted secrets**: Secrets with natural-looking vowel ratios (~40%) may not be detected
2. **Short secrets**: Only detects strings >8 characters
3. **Context-free**: Does not use surrounding context (e.g., "password=" prefix)
4. **English-tuned**: Parameters optimized for English; other languages may need adjustment

## Files

- `redactor.py` - Main library implementation
- `test_runner.py` - YAML-based test runner
- `tests.yaml` - Cross-language test cases (29 tests)
- `notes.md` - Development notes and TDD process log

## Cross-Language Portability

The `tests.yaml` file is designed to be language-agnostic. To implement in another language:

1. Parse the YAML test cases
2. Implement `detect_secrets(text)` function
3. Run tests and verify all pass

The test format ensures consistent behavior across implementations.

## Algorithm Details

### Vowel Ratio
Normal English text has ~38-42% vowels. Deviations suggest random strings:
- <15%: Very suspicious (+3 score for non-simple words)
- 15-25%: Suspicious (+2)
- 25-30%: Slightly suspicious (+1)
- 55-70%: Slightly unusual (+1)
- >70%: Very unusual (+3)

### Digit Mixing
Letters mixed with digits (10-90% digits) suggests generated tokens (+2)

### Consonant Sequences
Long consonant sequences are rare in English:
- >6 consonants: +2 score
- >5 consonants: +1 score (except for simple words)

### Score Threshold
Tokens scoring ≥2 are flagged as potential secrets.

