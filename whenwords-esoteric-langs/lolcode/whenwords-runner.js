#!/usr/bin/env node
/**
 * whenwords LOLCODE implementation runner
 * Transpiles LOLCODE to JavaScript and adds remaining functions
 */

const fs = require('fs');
const path = require('path');
const yaml = require('js-yaml');

// Helper functions and variables needed by transpiled LOLCODE
function _xor(op1, op2) { return !!((op1 || op2) && !(op1 && op2)); }
function _char(n) { return String.fromCharCode(n); }
function _ord(c) { return c.charCodeAt(0); }
function _len(x) { return x.length !== undefined ? x.length : 0; }
const NOOB = null;  // LOLCODE null type

// Load and run the transpiled LOLCODE
function loadLolcode() {
    return new Promise((resolve, reject) => {
        const parse = require('/opt/node22/lib/node_modules/lolcode/parser.js');
        const lolcodeFile = fs.readFileSync(path.join(__dirname, 'whenwords.lol'), 'utf8');

        parse(lolcodeFile, (err, warn, jsCode) => {
            if (err) {
                reject(new Error(err));
                return;
            }

            // Execute the transpiled code to define functions
            try {
                eval(jsCode);
                resolve({ timeago, duration });
            } catch (e) {
                reject(e);
            }
        });
    });
}

let lolFunctions = null;

// Export the timeago function from LOLCODE
function timeago(timestamp, reference = timestamp) {
    return lolFunctions.timeago(timestamp, reference);
}

// Export the duration function from LOLCODE
function duration(seconds, options = {}) {
    if (seconds < 0) throw new Error('Negative seconds not allowed');
    const compact = options.compact || false;
    const maxUnits = options.max_units !== undefined ? options.max_units : 2;
    return lolFunctions.duration(seconds, compact, maxUnits);
}

// parse_duration - implemented in JavaScript as LOLCODE lacks regex
function parse_duration(str) {
    if (!str || str.trim() === '') {
        throw new Error('Empty string');
    }

    str = str.trim().toLowerCase();

    // Check for negative
    if (str.startsWith('-')) {
        throw new Error('Negative duration');
    }

    // Unit conversion map
    const units = {
        's': 1, 'sec': 1, 'secs': 1, 'second': 1, 'seconds': 1,
        'm': 60, 'min': 60, 'mins': 60, 'minute': 60, 'minutes': 60,
        'h': 3600, 'hr': 3600, 'hrs': 3600, 'hour': 3600, 'hours': 3600,
        'd': 86400, 'day': 86400, 'days': 86400,
        'w': 604800, 'wk': 604800, 'wks': 604800, 'week': 604800, 'weeks': 604800
    };

    let total = 0;
    let foundUnits = false;

    // Handle colon notation (h:mm or h:mm:ss)
    const colonMatch = str.match(/^(\d+):(\d+)(?::(\d+))?$/);
    if (colonMatch) {
        const hours = parseInt(colonMatch[1], 10);
        const minutes = parseInt(colonMatch[2], 10);
        const seconds = colonMatch[3] ? parseInt(colonMatch[3], 10) : 0;
        return hours * 3600 + minutes * 60 + seconds;
    }

    // Remove 'and', commas for parsing
    str = str.replace(/,/g, ' ').replace(/\band\b/g, ' ');

    // Match number + unit patterns
    const pattern = /(\d+\.?\d*)\s*([a-z]+)/g;
    let match;

    while ((match = pattern.exec(str)) !== null) {
        const value = parseFloat(match[1]);
        const unit = match[2];

        if (units[unit]) {
            total += value * units[unit];
            foundUnits = true;
        }
    }

    if (!foundUnits) {
        // Check if it's just a number without units
        if (/^\d+\.?\d*$/.test(str.trim())) {
            throw new Error('No units specified');
        }
        throw new Error('No parseable units');
    }

    return Math.round(total);
}

// human_date - implemented in JavaScript for date handling
function human_date(timestamp, reference) {
    const date = new Date(timestamp * 1000);
    const refDate = new Date(reference * 1000);

    // Get UTC day boundaries
    const dateDay = Math.floor(timestamp / 86400);
    const refDay = Math.floor(reference / 86400);
    const dayDiff = dateDay - refDay;

    const months = ['January', 'February', 'March', 'April', 'May', 'June',
                    'July', 'August', 'September', 'October', 'November', 'December'];
    const weekdays = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];

    // Same day
    if (dayDiff === 0) {
        return 'Today';
    }

    // Yesterday
    if (dayDiff === -1) {
        return 'Yesterday';
    }

    // Tomorrow
    if (dayDiff === 1) {
        return 'Tomorrow';
    }

    // Within past 7 days (exclusive of exactly 7 days)
    if (dayDiff > -7 && dayDiff < -1) {
        return 'Last ' + weekdays[date.getUTCDay()];
    }

    // Within next 7 days (exclusive of exactly 7 days)
    if (dayDiff > 1 && dayDiff < 7) {
        return 'This ' + weekdays[date.getUTCDay()];
    }

    // Different year
    const dateYear = date.getUTCFullYear();
    const refYear = refDate.getUTCFullYear();

    if (dateYear !== refYear) {
        return months[date.getUTCMonth()] + ' ' + date.getUTCDate() + ', ' + dateYear;
    }

    // Same year
    return months[date.getUTCMonth()] + ' ' + date.getUTCDate();
}

// date_range - implemented in JavaScript
function date_range(start, end) {
    // Swap if end before start
    if (end < start) {
        [start, end] = [end, start];
    }

    const startDate = new Date(start * 1000);
    const endDate = new Date(end * 1000);

    const months = ['January', 'February', 'March', 'April', 'May', 'June',
                    'July', 'August', 'September', 'October', 'November', 'December'];

    const startYear = startDate.getUTCFullYear();
    const endYear = endDate.getUTCFullYear();
    const startMonth = startDate.getUTCMonth();
    const endMonth = endDate.getUTCMonth();
    const startDay = startDate.getUTCDate();
    const endDay = endDate.getUTCDate();

    // Same day
    if (startYear === endYear && startMonth === endMonth && startDay === endDay) {
        return months[startMonth] + ' ' + startDay + ', ' + startYear;
    }

    // Same month and year
    if (startYear === endYear && startMonth === endMonth) {
        return months[startMonth] + ' ' + startDay + '–' + endDay + ', ' + startYear;
    }

    // Same year, different months
    if (startYear === endYear) {
        return months[startMonth] + ' ' + startDay + ' – ' + months[endMonth] + ' ' + endDay + ', ' + startYear;
    }

    // Different years
    return months[startMonth] + ' ' + startDay + ', ' + startYear + ' – ' +
           months[endMonth] + ' ' + endDay + ', ' + endYear;
}

// Test runner
async function runTests() {
    // Load LOLCODE functions first
    lolFunctions = await loadLolcode();

    const testsFile = fs.readFileSync('/tmp/whenwords/tests.yaml', 'utf8');
    const tests = yaml.load(testsFile);

    let passed = 0;
    let failed = 0;

    console.log('Running whenwords LOLCODE tests...\n');

    // Test timeago
    console.log('=== timeago tests ===');
    for (const test of tests.timeago) {
        try {
            const result = timeago(test.input.timestamp, test.input.reference);
            if (result === test.output) {
                passed++;
                console.log(`✓ ${test.name}`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: expected "${test.output}", got "${result}"`);
            }
        } catch (e) {
            if (test.error) {
                passed++;
                console.log(`✓ ${test.name} (expected error)`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: unexpected error: ${e.message}`);
            }
        }
    }

    // Test duration
    console.log('\n=== duration tests ===');
    for (const test of tests.duration) {
        try {
            const options = test.input.options || {};
            const result = duration(test.input.seconds, options);
            if (test.error) {
                failed++;
                console.log(`✗ ${test.name}: expected error but got "${result}"`);
            } else if (result === test.output) {
                passed++;
                console.log(`✓ ${test.name}`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: expected "${test.output}", got "${result}"`);
            }
        } catch (e) {
            if (test.error) {
                passed++;
                console.log(`✓ ${test.name} (expected error)`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: unexpected error: ${e.message}`);
            }
        }
    }

    // Test parse_duration
    console.log('\n=== parse_duration tests ===');
    for (const test of tests.parse_duration) {
        try {
            const result = parse_duration(test.input);
            if (test.error) {
                failed++;
                console.log(`✗ ${test.name}: expected error but got ${result}`);
            } else if (result === test.output) {
                passed++;
                console.log(`✓ ${test.name}`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: expected ${test.output}, got ${result}`);
            }
        } catch (e) {
            if (test.error) {
                passed++;
                console.log(`✓ ${test.name} (expected error)`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: unexpected error: ${e.message}`);
            }
        }
    }

    // Test human_date
    console.log('\n=== human_date tests ===');
    for (const test of tests.human_date) {
        try {
            const result = human_date(test.input.timestamp, test.input.reference);
            if (result === test.output) {
                passed++;
                console.log(`✓ ${test.name}`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: expected "${test.output}", got "${result}"`);
            }
        } catch (e) {
            if (test.error) {
                passed++;
                console.log(`✓ ${test.name} (expected error)`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: unexpected error: ${e.message}`);
            }
        }
    }

    // Test date_range
    console.log('\n=== date_range tests ===');
    for (const test of tests.date_range) {
        try {
            const result = date_range(test.input.start, test.input.end);
            if (result === test.output) {
                passed++;
                console.log(`✓ ${test.name}`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: expected "${test.output}", got "${result}"`);
            }
        } catch (e) {
            if (test.error) {
                passed++;
                console.log(`✓ ${test.name} (expected error)`);
            } else {
                failed++;
                console.log(`✗ ${test.name}: unexpected error: ${e.message}`);
            }
        }
    }

    console.log(`\n=== Results ===`);
    console.log(`Passed: ${passed}`);
    console.log(`Failed: ${failed}`);
    console.log(`Total: ${passed + failed}`);

    return failed === 0;
}

// Run if called directly
if (require.main === module) {
    runTests().then(success => {
        process.exit(success ? 0 : 1);
    }).catch(err => {
        console.error('Error:', err);
        process.exit(1);
    });
}

module.exports = { timeago, duration, parse_duration, human_date, date_range };
