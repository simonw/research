# Medium-reasoning configuration scored 98.82% versus 26.63% without thinking

This is a like-for-like comparison of the same 169 frozen `r01` additions—one from each ordered 1–13 digit cell. The medium-reasoning run gained 72.19 percentage points in visible-answer accuracy, but it also used an unlimited prediction setting instead of the previous 128-token cap. It is therefore a configuration comparison, not an isolated causal estimate of reasoning alone.

## What changed

| Setting | Previous run | New run |
|---|---:|---:|
| Reasoning | Disabled | Medium, separated reasoning channel |
| Completion limit | 128 tokens | `max_tokens=-1` (bounded by 32,768-token context) |
| Cases compared | Same 169 `r01` cases | Same 169 `r01` cases |
| Execution order | Seeded shuffle inside the 5,070-case run | Easiest to hardest |
| Decoding | Greedy | Greedy |

## Paired results

| Measure | No thinking | Medium reasoning | Change | Paired transitions (both / new only / old only / neither) |
|---|---:|---:|---:|---:|
| Numeric correctness | 45/169 (26.63%) | 167/169 (98.82%) | +72.19 pp | 44 / 123 / 1 / 1 |
| Instruction compliance | 164/169 (97.04%) | 168/169 (99.41%) | +2.37 pp | 163 / 5 / 1 / 0 |
| Canonical wording | 43/169 (25.44%) | 166/169 (98.22%) | +72.78 pp | 42 / 124 / 1 / 2 |

The new run produced 2 wrong visible answer(s). All 169 requests completed without retry or API failure.

| Runtime measure | No thinking | Medium reasoning |
|---|---:|---:|
| Median latency | 1.50 s | 27.62 s |
| Mean latency | 1.50 s | 56.60 s |
| Median completion tokens | 14 | 313 |
| Total completion tokens | 2,376 | 112,887 |

## Requested transcripts

The reasoning blocks and visible responses below are copied verbatim from the saved API response JSON. Hidden reasoning is shown for inspection only and was never used to grade or rescue an answer.

### Easiest calculation: request 1 — `a01_b01_r01`

- Calculation: `7 + 9 = 16`
- Current grade: **correct**; finish `stop`; 62 completion tokens; 5.75 seconds
- Previous no-thinking grade on the same case: **wrong**
- Previous no-thinking visible response: "seventeen"
- Reasoning SHA-256: `467e1fdc407863b820ff7368e11f87ef1200bf29d7ce65493955b8fd629bee3e`
- Visible-response SHA-256: `f2ce341fdaa67727bbdc88e3f7f19dc539943e346f6c2bac497fa1c89c0ca128`

#### Full reasoning transcript

```text
The user asks what 7 + 9 is, and wants the answer written in words only, with no other text.

7 + 9 = 16

16 in words is "sixteen"

They want just the answer in words, nothing else.

```

#### Full visible response

```text
sixteen
```

### Approximately one-third through the run: request 56 — `a04_b08_r01`

- Calculation: `5,639 + 20,690,700 = 20,696,339`
- Current grade: **correct**; finish `stop`; 194 completion tokens; 16.32 seconds
- Previous no-thinking grade on the same case: **wrong**
- Previous no-thinking visible response: "twenty million six hundred ninety thousand three hundred ninety-nine"
- Reasoning SHA-256: `0221aa6fa0d2ac0400ee0e7c448b48acf2cf4dd8378ae72e5fb2587dbb2ba781`
- Visible-response SHA-256: `e0f93244d238d6ca4f827f0774b4c33e323a1abef4d91653a88a893fb088a208`

#### Full reasoning transcript

```text
The user wants me to add 5639 + 20690700 and write the answer in words only.

5639 + 20690700 = 20696339

Now I need to write 20,696,339 in words.

20,696,339 = twenty million six hundred ninety-six thousand three hundred thirty-nine

Let me verify:
- 20,000,000 = twenty million
- 696,000 = six hundred ninety-six thousand
- 339 = three hundred thirty-nine

So: twenty million six hundred ninety-six thousand three hundred thirty-nine

The user said "Do not include any other text or information, just the answer in words."

```

#### Full visible response

```text
twenty million six hundred ninety-six thousand three hundred thirty-nine
```

### Hardest calculation answered correctly: request 169 — `a13_b13_r01`

- Calculation: `4,299,366,105,622 + 6,088,794,067,970 = 10,388,160,173,592`
- Current grade: **correct**; finish `stop`; 5,160 completion tokens; 460.07 seconds
- Previous no-thinking grade on the same case: **wrong**
- Previous no-thinking visible response: "Ten trillion five hundred eighty-eight million one hundred sixty thousand three hundred ninety-two"
- Reasoning SHA-256: `10d90ff1987c58dec38e8494a2c8c5fd5c19ee84d84feb30fbe3f80dc8c9d060`
- Visible-response SHA-256: `1e1febd8e8c5adac179fa8671eb3197158479d7aa5cfc91b085cce2080525b35`

#### Full reasoning transcript

```text
The user wants me to add two numbers and write the result in words.

4299366105622 + 6088794067970

Let me add these:
4,299,366,105,622
+ 6,088,794,067,970
= 10,388,160,173,592

Let me verify:
4299366105622
6088794067970

Starting from the right:
2 + 0 = 2
2 + 7 = 9
6 + 9 = 15, write 5 carry 1
5 + 7 + 1 = 13, write 3 carry 1
0 + 6 + 1 = 7
1 + 0 = 1
6 + 4 = 10, write 0 carry 1
6 + 9 + 1 = 16, write 6 carry 1
3 + 7 + 1 = 11, write 1 carry 1
9 + 8 + 1 = 18, write 8 carry 1
9 + 0 + 1 = 10, write 0 carry 1
2 + 6 + 1 = 9
4 + 0 = 4

Wait, let me redo this more carefully.

4,299,366,105,622
6,088,794,067,970

Let me align them:
  4 2 9 9 3 6 6 1 0 5 6 2 2
  6 0 8 8 7 9 4 0 6 7 9 7 0

Adding from right to left:
Position 1 (units): 2 + 0 = 2
Position 2 (tens): 2 + 7 = 9
Position 3 (hundreds): 6 + 9 = 15, write 5, carry 1
Position 4 (thousands): 5 + 7 + 1 = 13, write 3, carry 1
Position 5 (ten-thousands): 0 + 6 + 1 = 7
Position 6 (hundred-thousands): 1 + 0 = 1
Position 7 (millions): 6 + 4 = 10, write 0, carry 1
Position 8 (ten-millions): 6 + 9 + 1 = 16, write 6, carry 1
Position 9 (hundred-millions): 3 + 7 + 1 = 11, write 1, carry 1
Position 10 (billions): 9 + 8 + 1 = 18, write 8, carry 1
Position 11 (ten-billions): 9 + 0 + 1 = 10, write 0, carry 1
Position 12 (hundred-billions): 2 + 6 + 1 = 9
Position 13 (trillions): 4 + 0 = 4

Wait, that gives me 49,081,601,735,92? That doesn't seem right. Let me recount the digits.

4299366105622 - let me count: 4-2-9-9-3-6-6-1-0-5-6-2-2 = 13 digits
6088794067970 - let me count: 6-0-8-8-7-9-4-0-6-7-9-7-0 = 13 digits

So both are 13-digit numbers.

Let me write them aligned:
  4 2 9 9 3 6 6 1 0 5 6 2 2
  6 0 8 8 7 9 4 0 6 7 9 7 0

Position (from right, 1-indexed):
Pos 1: 2 + 0 = 2
Pos 2: 2 + 7 = 9
Pos 3: 6 + 9 = 15, write 5, carry 1
Pos 4: 5 + 7 + 1 = 13, write 3, carry 1
Pos 5: 0 + 6 + 1 = 7
Pos 6: 1 + 0 = 1
Pos 7: 6 + 4 = 10, write 0, carry 1
Pos 8: 6 + 9 + 1 = 16, write 6, carry 1
Pos 9: 3 + 7 + 1 = 11, write 1, carry 1
Pos 10: 9 + 8 + 1 = 18, write 8, carry 1
Pos 11: 9 + 0 + 1 = 10, write 0, carry 1
Pos 12: 2 + 6 + 1 = 9
Pos 13: 4 + 0 = 4

Reading from pos 13 to pos 1: 4 9 0 8 1 6 0 1 7 3 5 9 2

So the answer is 4,908,160,173,592

Wait, let me double-check. Let me re-examine the digits.

4299366105622:
Digit positions from left: 4(13) 2(12) 9(11) 9(10) 3(9) 6(8) 6(7) 1(6) 0(5) 5(4) 6(3) 2(2) 2(1)

6088794067970:
Digit positions from left: 6(13) 0(12) 8(11) 8(10) 7(9) 9(8) 4(7) 0(6) 6(5) 7(4) 9(3) 7(2) 0(1)

Adding from position 1 (rightmost):
Pos 1: 2 + 0 = 2
Pos 2: 2 + 7 = 9
Pos 3: 6 + 9 = 15, write 5, carry 1
Pos 4: 5 + 7 + 1 = 13, write 3, carry 1
Pos 5: 0 + 6 + 1 = 7
Pos 6: 1 + 0 = 1
Pos 7: 6 + 4 = 10, write 0, carry 1
Pos 8: 6 + 9 + 1 = 16, write 6, carry 1
Pos 9: 3 + 7 + 1 = 11, write 1, carry 1
Pos 10: 9 + 8 + 1 = 18, write 8, carry 1
Pos 11: 9 + 0 + 1 = 10, write 0, carry 1
Pos 12: 2 + 6 + 1 = 9
Pos 13: 4 + 0 = 4

Result from pos 13 to pos 1: 4, 9, 0, 8, 1, 6, 0, 1, 7, 3, 5, 9, 2

So: 4,908,160,173,592

Let me verify with a different approach:
4,299,366,105,622 + 6,088,794,067,970

4,299,366,105,622 + 6,000,000,000,000 = 10,299,366,105,622
10,299,366,105,622 + 88,794,067,970 = 10,388,160,173,592

Hmm, that gives a different answer. Let me recheck.

4,299,366,105,622 + 6,088,794,067,970

Let me break it down:
4,299,366,105,622
+ 6,088,794,067,970

4,299 + 6,088 = 10,387 (trillions)
366 + 794 = 1,160 (billions)
105 + 067 = 172 (millions)
622 + 970 = 1,592 (thousands)

Wait, I need to be more careful with the grouping.

4,299,366,105,622
6,088,794,067,970

Group by thousands:
4,299 | 366 | 105 | 622
6,088 | 794 | 067 | 970

Adding:
622 + 970 = 1,592 → write 592, carry 1
105 + 067 + 1 = 173 → write 173, carry 0
366 + 794 = 1,160 → write 160, carry 1
4,299 + 6,088 + 1 = 10,388

So the result is: 10,388 | 160 | 173 | 592

That's 10,388,160,173,592

Let me verify my digit-by-digit addition was wrong. Let me recheck.

4299366105622
6088794067970

Let me write them out very carefully:
4 2 9 9 3 6 6 1 0 5 6 2 2
6 0 8 8 7 9 4 0 6 7 9 7 0

Position from right (1-indexed):
Pos 1: 2, 0
Pos 2: 2, 7
Pos 3: 6, 9
Pos 4: 5, 7
Pos 5: 0, 6
Pos 6: 1, 0
Pos 7: 6, 4
Pos 8: 6, 9
Pos 9: 3, 7
Pos 10: 9, 8
Pos 11: 9, 0
Pos 12: 2, 6
Pos 13: 4, 6

Wait! Position 13: the first number has 4 and the second has 6!

Let me recount the digits of 6088794067970:
6-0-8-8-7-9-4-0-6-7-9-7-0

That's 13 digits. The leftmost is 6.

And 4299366105622:
4-2-9-9-3-6-6-1-0-5-6-2-2

That's 13 digits. The leftmost is 4.

So Pos 13: 4 + 6 = 10, write 0, carry 1

Let me redo:
Pos 1: 2 + 0 = 2
Pos 2: 2 + 7 = 9
Pos 3: 6 + 9 = 15, write 5, carry 1
Pos 4: 5 + 7 + 1 = 13, write 3, carry 1
Pos 5: 0 + 6 + 1 = 7
Pos 6: 1 + 0 = 1
Pos 7: 6 + 4 = 10, write 0, carry 1
Pos 8: 6 + 9 + 1 = 16, write 6, carry 1
Pos 9: 3 + 7 + 1 = 11, write 1, carry 1
Pos 10: 9 + 8 + 1 = 18, write 8, carry 1
Pos 11: 9 + 0 + 1 = 10, write 0, carry 1
Pos 12: 2 + 6 + 1 = 9
Pos 13: 4 + 6 = 10, write 0, carry 1
Pos 14: carry 1

So the result is: 1 0 9 0 8 1 6 0 1 7 3 5 9 2

That's 10,908,160,173,592

Hmm, but my grouping method gave 10,388,160,173,592. Let me recheck the grouping.

4,299,366,105,622
6,088,794,067,970

Wait, let me recount the digits.

4299366105622: 4, 2, 9, 9, 3, 6, 6, 1, 0, 5, 6, 2, 2 → 13 digits
So this is 4,299,366,105,622 (4 trillion, 299 billion, 366 million, 105 thousand, 622)

6088794067970: 6, 0, 8, 8, 7, 9, 4, 0, 6, 7, 9, 7, 0 → 13 digits
So this is 6,088,794,067,970 (6 trillion, 88 billion, 794 million, 67 thousand, 970)

Adding:
622 + 970 = 1,592 → 592, carry 1
105 + 067 + 1 = 173 → 173, carry 0
366 + 794 = 1,160 → 160, carry 1
299 + 088 + 1 = 388 → 388, carry 0
4 + 6 = 10 → 10

So: 10,388,160,173,592

Now let me check my digit-by-digit work again. I think I made an error in identifying the digits.

4299366105622:
From left to right: 4, 2, 9, 9, 3, 6, 6, 1, 0, 5, 6, 2, 2

6088794067970:
From left to right: 6, 0, 8, 8, 7, 9, 4, 0, 6, 7, 9, 7, 0

From right to left:
4299366105622: 2, 2, 6, 5, 0, 1, 6, 6, 3, 9, 9, 2, 4
6088794067970: 0, 7, 9, 7, 6, 0, 4, 9, 7, 8, 8, 0, 6

Pos 1: 2 + 0 = 2
Pos 2: 2 + 7 = 9
Pos 3: 6 + 9 = 15, write 5, carry 1
Pos 4: 5 + 7 + 1 = 13, write 3, carry 1
Pos 5: 0 + 6 + 1 = 7
Pos 6: 1 + 0 = 1
Pos 7: 6 + 4 = 10, write 0, carry 1
Pos 8: 6 + 9 + 1 = 16, write 6, carry 1
Pos 9: 3 + 7 + 1 = 11, write 1, carry 1
Pos 10: 9 + 8 + 1 = 18, write 8, carry 1
Pos 11: 9 + 8 + 1 = 18, write 8, carry 1

Wait! Pos 11: The 11th digit from the right of 4299366105622 is... let me count again.

4299366105622 from right to left:
Pos 1: 2
Pos 2: 2
Pos 3: 6
Pos 4: 5
Pos 5: 0
Pos 6: 1
Pos 7: 6
Pos 8: 6
Pos 9: 3
Pos 10: 9
Pos 11: 9
Pos 12: 2
Pos 13: 4

6088794067970 from right to left:
Pos 1: 0
Pos 2: 7
Pos 3: 9
Pos 4: 7
Pos 5: 6
Pos 6: 0
Pos 7: 4
Pos 8: 9
Pos 9: 7
Pos 10: 8
Pos 11: 8
Pos 12: 0
Pos 13: 6

OK so:
Pos 1: 2 + 0 = 2
Pos 2: 2 + 7 = 9
Pos 3: 6 + 9 = 15, write 5, carry 1
Pos 4: 5 + 7 + 1 = 13, write 3, carry 1
Pos 5: 0 + 6 + 1 = 7
Pos 6: 1 + 0 = 1
Pos 7: 6 + 4 = 10, write 0, carry 1
Pos 8: 6 + 9 + 1 = 16, write 6, carry 1
Pos 9: 3 + 7 + 1 = 11, write 1, carry 1
Pos 10: 9 + 8 + 1 = 18, write 8, carry 1
Pos 11: 9 + 8 + 1 = 18, write 8, carry 1
Pos 12: 2 + 0 + 1 = 3
Pos 13: 4 + 6 = 10, write 0, carry 1
Pos 14: 1

Result: 1, 0, 3, 8, 8, 1, 6, 0, 1, 7, 3, 5, 9, 2

That's 10,388,160,173,592

I made an error earlier at Pos 11. The 11th digit from the right of 6088794067970 is 8, not 0. Let me verify:

6088794067970
From right: 0(1), 7(2), 9(3), 7(4), 6(5), 0(6), 4(7), 9(8), 7(9), 8(10), 8(11), 0(12), 6(13)

Yes, Pos 11 is 8. So 9 + 8 + 1 = 18, write 8, carry 1.

And Pos 12: 2 + 0 + 1 = 3.

So the answer is 10,388,160,173,592.

Now I need to write this in words.

10,388,160,173,592

10 trillion, 388 billion, 160 million, 173 thousand, 592

In words: ten trillion three hundred eighty-eight billion one hundred sixty million one hundred seventy-three thousand five hundred ninety-two

Let me make sure about the "and" - in standard American English number writing, we typically don't use "and" between the thousands and hundreds. But in British English, "and" is used. I'll go without "and" as it's more standard for this type of question.

Actually, let me reconsider. The standard way to write numbers in words:

10,388,160,173,592

= ten trillion, three hundred eighty-eight billion, one hundred sixty million, one hundred seventy-three thousand, five hundred ninety-two

I'll write it without commas since the user said "just the answer in words."

ten trillion three hundred eighty-eight billion one hundred sixty million one hundred seventy-three thousand five hundred ninety-two

```

#### Full visible response

```text
ten trillion three hundred eighty-eight billion one hundred sixty million one hundred seventy-three thousand five hundred ninety-two
```

## Every wrong answer in the medium-reasoning run

### Wrong answer 1: request 53 — `a02_b08_r01`

- Calculation: `73 + 42,439,903 = 42,439,976`
- Current grade: **wrong**; finish `stop`; 193 completion tokens; 16.26 seconds
- Previous no-thinking grade on the same case: **correct**
- Previous no-thinking visible response: "forty-two million four hundred thirty-nine thousand nine hundred seventy-six"
- Reasoning SHA-256: `29fabb757a50902e8c457b6e77d01560073c2357b332f1ca6bbe2076327d4f1e`
- Visible-response SHA-256: `08a2a994230f62598821f0b4b501dae0bd9b100b9c9d90073a78d880686d2906`

#### Full reasoning transcript

```text
The user wants me to add 73 + 42439903 and write the answer in words only.

73 + 42439903 = 42440076

Now I need to write 42440076 in words.

42,440,076

Let me break this down:
- 42 million
- 440 thousand
- 76

So: forty-two million four hundred forty thousand seventy-six

Let me double-check: 42,440,076
- 42 = forty-two (million)
- 440 = four hundred forty (thousand)
- 076 = seventy-six

Yes: forty-two million four hundred forty thousand seventy-six

```

#### Full visible response

```text
forty-two million four hundred forty thousand seventy-six
```

### Wrong answer 2: request 139 — `a12_b09_r01`

- Calculation: `866,210,161,193 + 296,827,965 = 866,506,989,158`
- Current grade: **wrong**; finish `stop`; 3,322 completion tokens; 276.87 seconds
- Previous no-thinking grade on the same case: **wrong**
- Previous no-thinking visible response: "eight hundred sixty-six billion two hundred ten million one hundred sixty-one thousand one hundred ninety-three"
- Reasoning SHA-256: `6095bf483e29f9858878b047f090b173c4bd8c2b22d2b2de02fef04d955e6c9c`
- Visible-response SHA-256: `b7c17edc2a0fd34744b812a6a919d8ff77e7a93f867bd682a40d93e41c0d54ff`

#### Full reasoning transcript

```text
The user wants me to add 866210161193 + 296827965 and write the answer in words.

Let me calculate:
866,210,161,193
+    296,827,965
= ?

Let me add these carefully:

866,210,161,193
+    296,827,965

Starting from the right:
3 + 5 = 8
9 + 6 = 15, write 5 carry 1
1 + 9 + 1 = 11, write 1 carry 1
6 + 7 + 1 = 14, write 4 carry 1
1 + 2 + 1 = 4
0 + 8 = 8
1 + 6 = 7
6 + 9 = 15, write 5 carry 1
2 + 2 + 1 = 5
1 + 0 = 1
0 + 0 = 0
6 + 0 = 6
6 + 0 = 6
8 + 0 = 8

Wait, let me redo this more carefully by aligning the numbers:

  866,210,161,193
+     296,827,965

Let me align by digits:

866,210,161,193
0,296,827,965

Position by position (from right):
Position 1 (ones): 3 + 5 = 8
Position 2 (tens): 9 + 6 = 15, write 5, carry 1
Position 3 (hundreds): 1 + 9 + 1 = 11, write 1, carry 1
Position 4 (thousands): 6 + 7 + 1 = 14, write 4, carry 1
Position 5 (ten-thousands): 1 + 2 + 1 = 4
Position 6 (hundred-thousands): 0 + 8 = 8
Position 7 (millions): 1 + 6 = 7
Position 8 (ten-millions): 6 + 9 = 15, write 5, carry 1
Position 9 (hundred-millions): 2 + 2 + 1 = 5
Position 10 (billions): 1 + 0 = 1
Position 11 (ten-billions): 0 + 0 = 0
Position 12 (hundred-billions): 6 + 0 = 6
Position 13 (trillions): 6 + 0 = 6
Position 14 (ten-trillions): 8 + 0 = 8

So the result is: 866,506,988,158

Wait, let me redo this more carefully.

866,210,161,193
+    296,827,965

Let me write them aligned:
866,210,161,193
000,296,827,965

Now adding:
866,210,161,193
+  296,827,965

Let me break it down:
866,210,161,193 + 296,827,965

= 866,210,161,193 + 296,827,965

Let me add step by step:
866,210,161,193
+     296,827,965

Starting from the rightmost digit:
3 + 5 = 8
9 + 6 = 15 → 5, carry 1
1 + 9 + 1 = 11 → 1, carry 1
6 + 7 + 1 = 14 → 4, carry 1
1 + 2 + 1 = 4
0 + 8 = 8
1 + 6 = 7
6 + 9 = 15 → 5, carry 1
2 + 2 + 1 = 5
1 + 0 = 1
0 + 0 = 0
6 + 0 = 6
6 + 0 = 6
8 + 0 = 8

Reading from left to right: 8, 6, 6, 5, 1, 0, 7, 8, 4, 1, 5, 8

Wait, that gives me 866,510,784,158? Let me recount.

Let me be very careful. I'll write the numbers digit by digit from right to left:

866,210,161,193:
Position (from right, 1-indexed):
1: 3
2: 9
3: 1
4: 1
5: 6
6: 1
7: 0
8: 1
9: 2
10: 1
11: 0
12: 6
13: 6
14: 8

Wait, let me just write out the number: 8 6 6 2 1 0 1 6 1 1 9 3

That's 12 digits: 866,210,161,193

And 296,827,965 is 9 digits: 2 9 6 8 2 7 9 6 5

Let me align them:
  8 6 6 2 1 0 1 6 1 1 9 3
  0 0 0 2 9 6 8 2 7 9 6 5

Now adding from right to left:
Pos 1: 3 + 5 = 8
Pos 2: 9 + 6 = 15 → 5, carry 1
Pos 3: 1 + 9 + 1 = 11 → 1, carry 1
Pos 4: 1 + 7 + 1 = 9
Pos 5: 6 + 2 = 8
Pos 6: 1 + 8 = 9
Pos 7: 0 + 6 = 6
Pos 8: 1 + 9 = 10 → 0, carry 1
Pos 9: 2 + 2 + 1 = 5
Pos 10: 1 + 0 = 1
Pos 11: 6 + 0 = 6
Pos 12: 8 + 0 = 8

Reading from left to right: 8 6 1 5 0 6 9 8 9 1 5 8

So the answer is 861,506,989,158

Wait, let me double-check. Let me re-examine the digits of 866,210,161,193.

866,210,161,193

Breaking it down:
866 billion, 210 million, 161 thousand, 193

So the digits are: 8, 6, 6, 2, 1, 0, 1, 6, 1, 1, 9, 3

And 296,827,965:
296 million, 827 thousand, 965

Digits: 2, 9, 6, 8, 2, 7, 9, 6, 5

Aligning (right-aligned):
  8 6 6 2 1 0 1 6 1 1 9 3
  0 0 0 2 9 6 8 2 7 9 6 5

Position from right (1-indexed):
Pos 1: 3 + 5 = 8
Pos 2: 9 + 6 = 15 → 5, carry 1
Pos 3: 1 + 9 + 1 = 11 → 1, carry 1
Pos 4: 1 + 7 + 1 = 9
Pos 5: 6 + 2 = 8
Pos 6: 1 + 8 = 9
Pos 7: 0 + 6 = 6
Pos 8: 1 + 9 = 10 → 0, carry 1
Pos 9: 2 + 2 + 1 = 5
Pos 10: 1 + 0 = 1
Pos 11: 6 + 0 = 6
Pos 12: 8 + 0 = 8

Result from left to right: 8, 6, 1, 5, 0, 6, 9, 8, 9, 1, 5, 8

So: 861,506,989,158

Let me verify: 866,210,161,193 + 296,827,965

866,210,161,193 + 296,827,965
= 866,210,161,193 + 296,827,965

Let me verify differently:
866,210,161,193 + 300,000,000 = 866,510,161,193
866,510,161,193 - 3,172,035 = 866,506,989,158

Let me check: 300,000,000 - 296,827,965 = 3,172,035

866,510,161,193 - 3,172,035:
866,510,161,193
-       3,172,035
= 866,506,989,158

Let me verify:
193 - 035 = 158 ✓
161 - 172... need to borrow
161,193 - 3,172,035

Actually let me just do:
866,510,161,193 - 3,172,035

161,193 - 3,172,035... that's negative, so I need to borrow from the millions.

866,510,161,193
-        3,172,035

Let me do this digit by digit:
  866,510,161,193
-        3,172,035

Pos 1: 3 - 5 → need to borrow: 13 - 5 = 8
Pos 2: 9 - 1 - 3 = 5 (borrowed 1 from pos 2, so 9 becomes 8, then 8 - 3 = 5)

Hmm, this is getting complicated. Let me just verify my original addition.

Actually, let me re-verify the original addition more carefully.

866,210,161,193
+    296,827,965

Let me split into parts:
866,210,000,000 + 161,193
+ 296,000,000 + 827,965

= 866,210,000,000 + 296,000,000 + 161,193 + 827,965
= 866,506,000,000 + 989,158
= 866,506,989,158

Yes! That confirms it. 161,193 + 827,965 = 989,158. And 866,210,000,000 + 296,000,000 = 866,506,000,000.

So the answer is 866,506,989,158.

Now I need to write this in words:
866,506,989,158

866 trillion, 506 billion, 989 million, 158 thousand... wait, no.

Let me break it down:
866,506,989,158

866 trillion, 506 billion, 989 million, 158

Wait: 866,506,989,158
- 866 trillion
- 506 billion
- 989 million
- 158

So: eight hundred sixty-six trillion, five hundred six billion, nine hundred eighty-nine million, one hundred fifty-eight

Let me double-check the grouping:
866,506,989,158
- 866 | 506 | 989 | 158

Yes: 866 trillion, 506 billion, 989 million, 158

In words: eight hundred sixty-six trillion, five hundred six billion, nine hundred eighty-nine million, one hundred fifty-eight

```

#### Full visible response

```text
eight hundred sixty-six trillion, five hundred six billion, nine hundred eighty-nine million, one hundred fifty-eight
```

## Reproducibility

- Current raw attempts: `runs/qwen3.8-27b-q4_k_m-reasoning-medium-unlimited-one-per-cell-easiest-first-seed-20260929/attempts.jsonl`
- Current graded results: `runs/qwen3.8-27b-q4_k_m-reasoning-medium-unlimited-one-per-cell-easiest-first-seed-20260929/results.csv`
- Previous graded results: `runs/qwen3.8-27b-q4_k_m-seed-20260929/results.csv`
- Current pair manifest: `runs/qwen3.8-27b-q4_k_m-reasoning-medium-unlimited-one-per-cell-easiest-first-seed-20260929/pairs.csv`
- The comparison joined on `case_id` and verified operands, answers, prompt, digit features, replicate, and expected wording for all 169 cases.
