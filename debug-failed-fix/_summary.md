Debugging investigation into why commit 0dcfad4's fix for cog code rendering didn't work. The fix correctly used string concatenation to avoid `-->` in Python strings, but the explanatory comment itself contained the literal `-->` sequence, which closed the HTML comment early. Solution: rewrote the comment to avoid the problematic character sequence.

- Root cause: Python comment on line 121 contained `"-->"` which HTML parser treats as comment terminator
- Fix: Changed comment wording to avoid the literal sequence
- Lesson: When avoiding character sequences, they must be avoided everywhere in raw text including comments
