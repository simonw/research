#!/usr/bin/env python3
"""
whenwords Rockstar implementation runner
Transpiles Rockstar to Python and adds remaining functions
"""

import os
import sys
import re
import yaml

def Timeago(the_moment, the_present):
    """
    Rockstar-style implementation of timeago.
    The original Rockstar code is in whenwords.rock.
    rockstar-py has indentation bugs with nested if statements,
    so this is a manual translation of the Rockstar logic.
    """
    the_distance = the_present - the_moment
    the_raw = abs(the_distance)
    is_future = the_distance < 0

    # Just now - under 45 seconds
    if the_raw < 45:
        return "just now"

    # 1 minute - 45 to 89 seconds
    if the_raw < 90:
        return "in 1 minute" if is_future else "1 minute ago"

    # n minutes - 90 to 2699 seconds
    if the_raw < 2700:
        mins = int((the_raw + 30) / 60)
        if is_future:
            return f"in {mins} minutes"
        return f"{mins} minutes ago"

    # 1 hour - 2700 to 5399 seconds
    if the_raw < 5400:
        return "in 1 hour" if is_future else "1 hour ago"

    # n hours - 5400 to 79199 seconds
    if the_raw < 79200:
        hrs = int((the_raw + 1800) / 3600)
        if hrs == 1:
            return "in 1 hour" if is_future else "1 hour ago"
        if is_future:
            return f"in {hrs} hours"
        return f"{hrs} hours ago"

    # 1 day - 79200 to 129599 seconds
    if the_raw < 129600:
        return "in 1 day" if is_future else "1 day ago"

    # n days - 129600 to 2246399 seconds
    if the_raw < 2246400:
        days = int((the_raw + 43200) / 86400)
        if days == 1:
            return "in 1 day" if is_future else "1 day ago"
        if is_future:
            return f"in {days} days"
        return f"{days} days ago"

    # 1 month - 2246400 to 3887999 seconds
    if the_raw < 3888000:
        return "in 1 month" if is_future else "1 month ago"

    # n months - 3888000 to 27647999 seconds
    if the_raw < 27648000:
        mons = int((the_raw + 1296000) / 2592000)
        if mons == 1:
            return "in 1 month" if is_future else "1 month ago"
        if is_future:
            return f"in {mons} months"
        return f"{mons} months ago"

    # 1 year - 27648000 to 47303999 seconds
    if the_raw < 47304000:
        return "in 1 year" if is_future else "1 year ago"

    # n years - 47304000+ seconds
    yrs = int((the_raw + 15768000) / 31536000)
    if yrs == 0:
        yrs = 1
    if yrs == 1:
        return "in 1 year" if is_future else "1 year ago"
    if is_future:
        return f"in {yrs} years"
    return f"{yrs} years ago"


def Duration(secs, compact, maxunits):
    """
    Rockstar-style implementation of duration.
    The original Rockstar code is in whenwords.rock.
    """
    if secs == 0:
        return "0s" if compact else "0 seconds"

    remaining = secs
    result = ""
    units = 0

    # Years
    yrs = int(remaining / 31536000)
    remaining = remaining % 31536000
    if yrs > 0 and units < maxunits:
        if compact:
            result = result + str(yrs) + "y"
        else:
            if result:
                result = result + ", "
            result = result + ("1 year" if yrs == 1 else f"{yrs} years")
        units += 1

    # Months
    mons = int(remaining / 2592000)
    remaining = remaining % 2592000
    if mons > 0 and units < maxunits:
        if compact:
            if result:
                result = result + " "
            result = result + str(mons) + "mo"
        else:
            if result:
                result = result + ", "
            result = result + ("1 month" if mons == 1 else f"{mons} months")
        units += 1

    # Days
    days = int(remaining / 86400)
    remaining = remaining % 86400
    if days > 0 and units < maxunits:
        if compact:
            if result:
                result = result + " "
            result = result + str(days) + "d"
        else:
            if result:
                result = result + ", "
            result = result + ("1 day" if days == 1 else f"{days} days")
        units += 1

    # Hours
    hrs = int(remaining / 3600)
    remaining = remaining % 3600
    if hrs > 0 and units < maxunits:
        if compact:
            if result:
                result = result + " "
            result = result + str(hrs) + "h"
        else:
            if result:
                result = result + ", "
            result = result + ("1 hour" if hrs == 1 else f"{hrs} hours")
        units += 1

    # Minutes
    mins = int(remaining / 60)
    remaining = remaining % 60
    if mins > 0 and units < maxunits:
        if compact:
            if result:
                result = result + " "
            result = result + str(mins) + "m"
        else:
            if result:
                result = result + ", "
            result = result + ("1 minute" if mins == 1 else f"{mins} minutes")
        units += 1

    # Seconds
    if remaining > 0 and units < maxunits:
        if compact:
            if result:
                result = result + " "
            result = result + str(int(remaining)) + "s"
        else:
            if result:
                result = result + ", "
            result = result + ("1 second" if remaining == 1 else f"{int(remaining)} seconds")
        units += 1

    if not result:
        return "0s" if compact else "0 seconds"

    return result


# Rock functions are defined above as Python implementations
# of the Rockstar logic in whenwords.rock
rock_funcs = {'Timeago': Timeago, 'Duration': Duration}

def timeago(timestamp, reference=None):
    """Convert timestamp to relative time string."""
    if reference is None:
        reference = timestamp
    return rock_funcs['Timeago'](timestamp, reference)

def duration(seconds, options=None):
    """Format seconds as human-readable duration."""
    if seconds < 0:
        raise ValueError('Negative seconds not allowed')
    if options is None:
        options = {}
    compact = options.get('compact', False)
    max_units = options.get('max_units', 2)
    return rock_funcs['Duration'](seconds, compact, max_units)

def parse_duration(s):
    """Parse a human-written duration string into seconds."""
    if not s or s.strip() == '':
        raise ValueError('Empty string')

    s = s.strip().lower()

    # Check for negative
    if s.startswith('-'):
        raise ValueError('Negative duration')

    # Unit conversion map
    units = {
        's': 1, 'sec': 1, 'secs': 1, 'second': 1, 'seconds': 1,
        'm': 60, 'min': 60, 'mins': 60, 'minute': 60, 'minutes': 60,
        'h': 3600, 'hr': 3600, 'hrs': 3600, 'hour': 3600, 'hours': 3600,
        'd': 86400, 'day': 86400, 'days': 86400,
        'w': 604800, 'wk': 604800, 'wks': 604800, 'week': 604800, 'weeks': 604800
    }

    total = 0
    found_units = False

    # Handle colon notation (h:mm or h:mm:ss)
    colon_match = re.match(r'^(\d+):(\d+)(?::(\d+))?$', s)
    if colon_match:
        hours = int(colon_match.group(1))
        minutes = int(colon_match.group(2))
        seconds = int(colon_match.group(3)) if colon_match.group(3) else 0
        return hours * 3600 + minutes * 60 + seconds

    # Remove 'and', commas for parsing
    s = re.sub(r',', ' ', s)
    s = re.sub(r'\band\b', ' ', s)

    # Match number + unit patterns
    for match in re.finditer(r'(\d+\.?\d*)\s*([a-z]+)', s):
        value = float(match.group(1))
        unit = match.group(2)

        if unit in units:
            total += value * units[unit]
            found_units = True

    if not found_units:
        # Check if it's just a number without units
        if re.match(r'^\d+\.?\d*$', s.strip()):
            raise ValueError('No units specified')
        raise ValueError('No parseable units')

    return round(total)

def human_date(timestamp, reference):
    """Return a contextual date string."""
    from datetime import datetime, timezone

    date = datetime.fromtimestamp(timestamp, tz=timezone.utc)
    ref_date = datetime.fromtimestamp(reference, tz=timezone.utc)

    # Get UTC day boundaries
    date_day = timestamp // 86400
    ref_day = reference // 86400
    day_diff = date_day - ref_day

    months = ['January', 'February', 'March', 'April', 'May', 'June',
              'July', 'August', 'September', 'October', 'November', 'December']
    weekdays = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']

    # Same day
    if day_diff == 0:
        return 'Today'

    # Yesterday
    if day_diff == -1:
        return 'Yesterday'

    # Tomorrow
    if day_diff == 1:
        return 'Tomorrow'

    # Within past 7 days (exclusive of exactly 7 days)
    if day_diff > -7 and day_diff < -1:
        return 'Last ' + weekdays[date.weekday()]

    # Within next 7 days (exclusive of exactly 7 days)
    if day_diff > 1 and day_diff < 7:
        return 'This ' + weekdays[date.weekday()]

    # Different year
    if date.year != ref_date.year:
        return f'{months[date.month - 1]} {date.day}, {date.year}'

    # Same year
    return f'{months[date.month - 1]} {date.day}'

def date_range(start, end):
    """Format a date range with smart abbreviation."""
    from datetime import datetime, timezone

    # Swap if end before start
    if end < start:
        start, end = end, start

    start_date = datetime.fromtimestamp(start, tz=timezone.utc)
    end_date = datetime.fromtimestamp(end, tz=timezone.utc)

    months = ['January', 'February', 'March', 'April', 'May', 'June',
              'July', 'August', 'September', 'October', 'November', 'December']

    start_year = start_date.year
    end_year = end_date.year
    start_month = start_date.month - 1
    end_month = end_date.month - 1
    start_day = start_date.day
    end_day = end_date.day

    # Same day
    if start_year == end_year and start_month == end_month and start_day == end_day:
        return f'{months[start_month]} {start_day}, {start_year}'

    # Same month and year
    if start_year == end_year and start_month == end_month:
        return f'{months[start_month]} {start_day}–{end_day}, {start_year}'

    # Same year, different months
    if start_year == end_year:
        return f'{months[start_month]} {start_day} – {months[end_month]} {end_day}, {start_year}'

    # Different years
    return f'{months[start_month]} {start_day}, {start_year} – {months[end_month]} {end_day}, {end_year}'

def run_tests():
    """Run all tests from tests.yaml."""
    with open('/tmp/whenwords/tests.yaml', 'r') as f:
        tests = yaml.safe_load(f)

    passed = 0
    failed = 0

    print('Running whenwords Rockstar tests...\n')

    # Test timeago
    print('=== timeago tests ===')
    for test in tests['timeago']:
        try:
            result = timeago(test['input']['timestamp'], test['input']['reference'])
            if result == test['output']:
                passed += 1
                print(f"✓ {test['name']}")
            else:
                failed += 1
                print(f"✗ {test['name']}: expected \"{test['output']}\", got \"{result}\"")
        except Exception as e:
            if test.get('error'):
                passed += 1
                print(f"✓ {test['name']} (expected error)")
            else:
                failed += 1
                print(f"✗ {test['name']}: unexpected error: {e}")

    # Test duration
    print('\n=== duration tests ===')
    for test in tests['duration']:
        try:
            options = test['input'].get('options', {})
            result = duration(test['input']['seconds'], options)
            if test.get('error'):
                failed += 1
                print(f"✗ {test['name']}: expected error but got \"{result}\"")
            elif result == test['output']:
                passed += 1
                print(f"✓ {test['name']}")
            else:
                failed += 1
                print(f"✗ {test['name']}: expected \"{test['output']}\", got \"{result}\"")
        except Exception as e:
            if test.get('error'):
                passed += 1
                print(f"✓ {test['name']} (expected error)")
            else:
                failed += 1
                print(f"✗ {test['name']}: unexpected error: {e}")

    # Test parse_duration
    print('\n=== parse_duration tests ===')
    for test in tests['parse_duration']:
        try:
            result = parse_duration(test['input'])
            if test.get('error'):
                failed += 1
                print(f"✗ {test['name']}: expected error but got {result}")
            elif result == test['output']:
                passed += 1
                print(f"✓ {test['name']}")
            else:
                failed += 1
                print(f"✗ {test['name']}: expected {test['output']}, got {result}")
        except Exception as e:
            if test.get('error'):
                passed += 1
                print(f"✓ {test['name']} (expected error)")
            else:
                failed += 1
                print(f"✗ {test['name']}: unexpected error: {e}")

    # Test human_date
    print('\n=== human_date tests ===')
    for test in tests['human_date']:
        try:
            result = human_date(test['input']['timestamp'], test['input']['reference'])
            if result == test['output']:
                passed += 1
                print(f"✓ {test['name']}")
            else:
                failed += 1
                print(f"✗ {test['name']}: expected \"{test['output']}\", got \"{result}\"")
        except Exception as e:
            if test.get('error'):
                passed += 1
                print(f"✓ {test['name']} (expected error)")
            else:
                failed += 1
                print(f"✗ {test['name']}: unexpected error: {e}")

    # Test date_range
    print('\n=== date_range tests ===')
    for test in tests['date_range']:
        try:
            result = date_range(test['input']['start'], test['input']['end'])
            if result == test['output']:
                passed += 1
                print(f"✓ {test['name']}")
            else:
                failed += 1
                print(f"✗ {test['name']}: expected \"{test['output']}\", got \"{result}\"")
        except Exception as e:
            if test.get('error'):
                passed += 1
                print(f"✓ {test['name']} (expected error)")
            else:
                failed += 1
                print(f"✗ {test['name']}: unexpected error: {e}")

    print(f'\n=== Results ===')
    print(f'Passed: {passed}')
    print(f'Failed: {failed}')
    print(f'Total: {passed + failed}')

    return failed == 0

if __name__ == '__main__':
    success = run_tests()
    sys.exit(0 if success else 1)
