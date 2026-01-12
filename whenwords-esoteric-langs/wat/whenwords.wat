;; whenwords library in WebAssembly Text Format (WAT)
;; A low-level stack-based esoteric implementation
;;
;; WAT is the textual representation of WebAssembly.
;; It's a stack-based virtual machine with S-expression syntax.
;; This module exports functions that compute time differences
;; and return codes that JavaScript translates to strings.

(module
  ;; Import JavaScript console for debugging (optional)
  ;; (import "console" "log" (func $log (param i32)))

  ;; Export functions
  (export "timeago_code" (func $timeago_code))
  (export "timeago_value" (func $timeago_value))
  (export "duration_parts" (func $duration_parts))

  ;; Memory for storing results (64KB page)
  (memory (export "memory") 1)

  ;; Global variable to store computed value for timeago
  (global $computed_value (mut i32) (i32.const 0))
  (global $is_future (mut i32) (i32.const 0))

  ;; timeago_code: Returns a code indicating which time string to use
  ;; Codes:
  ;;   0 = "just now"
  ;;   1 = "1 minute ago/in 1 minute"
  ;;   2 = "N minutes ago/in N minutes"
  ;;   3 = "1 hour ago/in 1 hour"
  ;;   4 = "N hours ago/in N hours"
  ;;   5 = "1 day ago/in 1 day"
  ;;   6 = "N days ago/in N days"
  ;;   7 = "1 month ago/in 1 month"
  ;;   8 = "N months ago/in N months"
  ;;   9 = "1 year ago/in 1 year"
  ;;   10 = "N years ago/in N years"
  ;; The computed value (N) is stored in timeago_value
  ;; is_future: 0 = past, 1 = future (stored in high bit of return value)

  (func $abs (param $n i32) (result i32)
    (if (result i32) (i32.lt_s (local.get $n) (i32.const 0))
      (then (i32.sub (i32.const 0) (local.get $n)))
      (else (local.get $n))
    )
  )

  (func $timeago_code (param $timestamp i32) (param $reference i32) (result i32)
    (local $diff i32)
    (local $abs_diff i32)
    (local $code i32)
    (local $value i32)

    ;; Calculate difference: reference - timestamp
    (local.set $diff (i32.sub (local.get $reference) (local.get $timestamp)))
    (local.set $abs_diff (call $abs (local.get $diff)))

    ;; Set is_future flag
    (global.set $is_future
      (if (result i32) (i32.lt_s (local.get $diff) (i32.const 0))
        (then (i32.const 1))
        (else (i32.const 0))
      )
    )

    ;; Just now: 0-44 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 45))
      (then
        (global.set $computed_value (i32.const 0))
        (return (i32.or (i32.const 0) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; 1 minute: 45-89 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 90))
      (then
        (global.set $computed_value (i32.const 1))
        (return (i32.or (i32.const 1) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; N minutes: 90-2699 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 2700))
      (then
        ;; minutes = round((abs_diff + 30) / 60)
        (local.set $value
          (i32.div_s (i32.add (local.get $abs_diff) (i32.const 30)) (i32.const 60))
        )
        (global.set $computed_value (local.get $value))
        (return (i32.or (i32.const 2) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; 1 hour: 2700-5399 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 5400))
      (then
        (global.set $computed_value (i32.const 1))
        (return (i32.or (i32.const 3) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; N hours: 5400-79199 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 79200))
      (then
        ;; hours = round((abs_diff + 1800) / 3600)
        (local.set $value
          (i32.div_s (i32.add (local.get $abs_diff) (i32.const 1800)) (i32.const 3600))
        )
        (global.set $computed_value (local.get $value))
        (if (i32.eq (local.get $value) (i32.const 1))
          (then
            (return (i32.or (i32.const 3) (i32.shl (global.get $is_future) (i32.const 8))))
          )
        )
        (return (i32.or (i32.const 4) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; 1 day: 79200-129599 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 129600))
      (then
        (global.set $computed_value (i32.const 1))
        (return (i32.or (i32.const 5) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; N days: 129600-2246399 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 2246400))
      (then
        ;; days = round((abs_diff + 43200) / 86400)
        (local.set $value
          (i32.div_s (i32.add (local.get $abs_diff) (i32.const 43200)) (i32.const 86400))
        )
        (global.set $computed_value (local.get $value))
        (if (i32.eq (local.get $value) (i32.const 1))
          (then
            (return (i32.or (i32.const 5) (i32.shl (global.get $is_future) (i32.const 8))))
          )
        )
        (return (i32.or (i32.const 6) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; 1 month: 2246400-3887999 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 3888000))
      (then
        (global.set $computed_value (i32.const 1))
        (return (i32.or (i32.const 7) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; N months: 3888000-27647999 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 27648000))
      (then
        ;; months = round((abs_diff + 1296000) / 2592000)
        (local.set $value
          (i32.div_s (i32.add (local.get $abs_diff) (i32.const 1296000)) (i32.const 2592000))
        )
        (global.set $computed_value (local.get $value))
        (if (i32.eq (local.get $value) (i32.const 1))
          (then
            (return (i32.or (i32.const 7) (i32.shl (global.get $is_future) (i32.const 8))))
          )
        )
        (return (i32.or (i32.const 8) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; 1 year: 27648000-47303999 seconds
    (if (i32.lt_s (local.get $abs_diff) (i32.const 47304000))
      (then
        (global.set $computed_value (i32.const 1))
        (return (i32.or (i32.const 9) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )

    ;; N years: 47304000+ seconds
    ;; years = round((abs_diff + 15768000) / 31536000)
    (local.set $value
      (i32.div_s (i32.add (local.get $abs_diff) (i32.const 15768000)) (i32.const 31536000))
    )
    (if (i32.eq (local.get $value) (i32.const 0))
      (then (local.set $value (i32.const 1)))
    )
    (global.set $computed_value (local.get $value))
    (if (i32.eq (local.get $value) (i32.const 1))
      (then
        (return (i32.or (i32.const 9) (i32.shl (global.get $is_future) (i32.const 8))))
      )
    )
    (i32.or (i32.const 10) (i32.shl (global.get $is_future) (i32.const 8)))
  )

  ;; Get the computed value (N in "N minutes ago", etc.)
  (func $timeago_value (result i32)
    (global.get $computed_value)
  )

  ;; Duration: Compute parts and store in memory
  ;; Returns number of units computed
  ;; Memory layout at offset 0:
  ;;   0-3: years
  ;;   4-7: months
  ;;   8-11: days
  ;;   12-15: hours
  ;;   16-19: minutes
  ;;   20-23: seconds

  (func $duration_parts (param $seconds i32) (param $max_units i32) (result i32)
    (local $remaining i32)
    (local $units i32)
    (local $value i32)

    (local.set $remaining (local.get $seconds))
    (local.set $units (i32.const 0))

    ;; Clear memory
    (i32.store (i32.const 0) (i32.const 0))   ;; years
    (i32.store (i32.const 4) (i32.const 0))   ;; months
    (i32.store (i32.const 8) (i32.const 0))   ;; days
    (i32.store (i32.const 12) (i32.const 0))  ;; hours
    (i32.store (i32.const 16) (i32.const 0))  ;; minutes
    (i32.store (i32.const 20) (i32.const 0))  ;; seconds

    ;; Years (31536000 seconds)
    (local.set $value (i32.div_s (local.get $remaining) (i32.const 31536000)))
    (local.set $remaining (i32.rem_s (local.get $remaining) (i32.const 31536000)))
    (if (i32.and
          (i32.gt_s (local.get $value) (i32.const 0))
          (i32.lt_s (local.get $units) (local.get $max_units)))
      (then
        (i32.store (i32.const 0) (local.get $value))
        (local.set $units (i32.add (local.get $units) (i32.const 1)))
      )
    )

    ;; Months (2592000 seconds)
    (local.set $value (i32.div_s (local.get $remaining) (i32.const 2592000)))
    (local.set $remaining (i32.rem_s (local.get $remaining) (i32.const 2592000)))
    (if (i32.and
          (i32.gt_s (local.get $value) (i32.const 0))
          (i32.lt_s (local.get $units) (local.get $max_units)))
      (then
        (i32.store (i32.const 4) (local.get $value))
        (local.set $units (i32.add (local.get $units) (i32.const 1)))
      )
    )

    ;; Days (86400 seconds)
    (local.set $value (i32.div_s (local.get $remaining) (i32.const 86400)))
    (local.set $remaining (i32.rem_s (local.get $remaining) (i32.const 86400)))
    (if (i32.and
          (i32.gt_s (local.get $value) (i32.const 0))
          (i32.lt_s (local.get $units) (local.get $max_units)))
      (then
        (i32.store (i32.const 8) (local.get $value))
        (local.set $units (i32.add (local.get $units) (i32.const 1)))
      )
    )

    ;; Hours (3600 seconds)
    (local.set $value (i32.div_s (local.get $remaining) (i32.const 3600)))
    (local.set $remaining (i32.rem_s (local.get $remaining) (i32.const 3600)))
    (if (i32.and
          (i32.gt_s (local.get $value) (i32.const 0))
          (i32.lt_s (local.get $units) (local.get $max_units)))
      (then
        (i32.store (i32.const 12) (local.get $value))
        (local.set $units (i32.add (local.get $units) (i32.const 1)))
      )
    )

    ;; Minutes (60 seconds)
    (local.set $value (i32.div_s (local.get $remaining) (i32.const 60)))
    (local.set $remaining (i32.rem_s (local.get $remaining) (i32.const 60)))
    (if (i32.and
          (i32.gt_s (local.get $value) (i32.const 0))
          (i32.lt_s (local.get $units) (local.get $max_units)))
      (then
        (i32.store (i32.const 16) (local.get $value))
        (local.set $units (i32.add (local.get $units) (i32.const 1)))
      )
    )

    ;; Seconds
    (if (i32.and
          (i32.gt_s (local.get $remaining) (i32.const 0))
          (i32.lt_s (local.get $units) (local.get $max_units)))
      (then
        (i32.store (i32.const 20) (local.get $remaining))
        (local.set $units (i32.add (local.get $units) (i32.const 1)))
      )
    )

    (local.get $units)
  )
)
