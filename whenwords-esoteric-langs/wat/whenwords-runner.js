#!/usr/bin/env node
/**
 * whenwords test runner for WebAssembly implementation
 *
 * This loads the compiled WASM module and runs it through the test suite.
 * The WASM module handles numeric computations while JavaScript handles
 * string formatting (since WASM has no native string support).
 */

const fs = require('fs');
const path = require('path');

// Time string templates based on WASM codes
const timeStrings = {
    0: () => "just now",
    1: (n, future) => future ? "in 1 minute" : "1 minute ago",
    2: (n, future) => future ? `in ${n} minutes` : `${n} minutes ago`,
    3: (n, future) => future ? "in 1 hour" : "1 hour ago",
    4: (n, future) => future ? `in ${n} hours` : `${n} hours ago`,
    5: (n, future) => future ? "in 1 day" : "1 day ago",
    6: (n, future) => future ? `in ${n} days` : `${n} days ago`,
    7: (n, future) => future ? "in 1 month" : "1 month ago",
    8: (n, future) => future ? `in ${n} months` : `${n} months ago`,
    9: (n, future) => future ? "in 1 year" : "1 year ago",
    10: (n, future) => future ? `in ${n} years` : `${n} years ago`
};

let wasmModule = null;
let memory = null;

async function loadWasm() {
    const wasmPath = path.join(__dirname, 'whenwords.wasm');
    const bytes = fs.readFileSync(wasmPath);
    const result = await WebAssembly.instantiate(bytes);
    wasmModule = result.instance.exports;
    memory = new Int32Array(wasmModule.memory.buffer);
}

function timeago(timestamp, reference) {
    const codeResult = wasmModule.timeago_code(timestamp, reference);
    const code = codeResult & 0xFF;
    const isFuture = (codeResult >> 8) & 1;
    const value = wasmModule.timeago_value();
    return timeStrings[code](value, isFuture === 1);
}

function duration(seconds, compact = false, maxUnits = 2) {
    if (seconds < 0) {
        return undefined;
    }
    if (seconds === 0) {
        return compact ? '0s' : '0 seconds';
    }

    // Call WASM function to compute parts
    wasmModule.duration_parts(seconds, maxUnits);

    // Read parts from WASM memory
    memory = new Int32Array(wasmModule.memory.buffer);
    const years = memory[0];
    const months = memory[1];
    const days = memory[2];
    const hours = memory[3];
    const minutes = memory[4];
    const secs = memory[5];

    // Format result
    const parts = [];
    if (years > 0) parts.push(compact ? `${years}y` : (years === 1 ? '1 year' : `${years} years`));
    if (months > 0) parts.push(compact ? `${months}mo` : (months === 1 ? '1 month' : `${months} months`));
    if (days > 0) parts.push(compact ? `${days}d` : (days === 1 ? '1 day' : `${days} days`));
    if (hours > 0) parts.push(compact ? `${hours}h` : (hours === 1 ? '1 hour' : `${hours} hours`));
    if (minutes > 0) parts.push(compact ? `${minutes}m` : (minutes === 1 ? '1 minute' : `${minutes} minutes`));
    if (secs > 0) parts.push(compact ? `${secs}s` : (secs === 1 ? '1 second' : `${secs} seconds`));

    if (parts.length === 0) {
        return compact ? '0s' : '0 seconds';
    }

    return compact ? parts.join(' ') : parts.join(', ');
}

// parse_duration - implemented in JavaScript (parsing is complex for WASM)
function parse_duration(str) {
    if (!str || typeof str !== 'string') {
        return undefined;
    }

    str = str.trim();
    if (str === '') {
        return undefined;
    }

    // Check for colon notation first (h:mm or h:mm:ss)
    const colonMatch = str.match(/^(\d+):(\d{2})(?::(\d{2}))?$/);
    if (colonMatch) {
        const hours = parseInt(colonMatch[1]);
        const mins = parseInt(colonMatch[2]);
        const secs = colonMatch[3] ? parseInt(colonMatch[3]) : 0;
        return hours * 3600 + mins * 60 + secs;
    }

    const units = {
        'year': 31536000, 'years': 31536000, 'y': 31536000,
        'month': 2592000, 'months': 2592000, 'mo': 2592000,
        'week': 604800, 'weeks': 604800, 'w': 604800,
        'day': 86400, 'days': 86400, 'd': 86400,
        'hour': 3600, 'hours': 3600, 'hr': 3600, 'hrs': 3600, 'h': 3600,
        'minute': 60, 'minutes': 60, 'min': 60, 'mins': 60, 'm': 60,
        'second': 1, 'seconds': 1, 'sec': 1, 's': 1
    };

    let total = 0;
    let foundAny = false;

    // Check for negative numbers
    if (str.includes('-')) {
        return undefined;
    }

    // Handle decimal values like "2.5 hours" or "1.5h"
    const decimalPattern = /(\d+\.?\d*)\s*([a-zA-Z]+)/g;
    let match;

    while ((match = decimalPattern.exec(str)) !== null) {
        const value = parseFloat(match[1]);
        const unit = match[2].toLowerCase();
        if (units[unit]) {
            total += value * units[unit];
            foundAny = true;
        }
    }

    if (!foundAny) {
        return undefined;
    }

    return Math.round(total);
}

// human_date - implemented in JavaScript (date parsing is complex for WASM)
function human_date(timestamp, referenceTimestamp = null) {
    // Convert Unix timestamps to Date objects
    const date = new Date(timestamp * 1000);
    const reference = referenceTimestamp ? new Date(referenceTimestamp * 1000) : new Date();

    // Reset to start of day for comparison (in UTC)
    const dateDay = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()));
    const refDay = new Date(Date.UTC(reference.getUTCFullYear(), reference.getUTCMonth(), reference.getUTCDate()));

    const dayDiff = Math.round((dateDay - refDay) / 86400000);

    const days = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
    const months = ['January', 'February', 'March', 'April', 'May', 'June',
                   'July', 'August', 'September', 'October', 'November', 'December'];

    if (dayDiff === 0) return 'Today';
    if (dayDiff === 1) return 'Tomorrow';
    if (dayDiff === -1) return 'Yesterday';

    // Within past week (but not yesterday)
    if (dayDiff > -7 && dayDiff < -1) {
        return `Last ${days[date.getUTCDay()]}`;
    }

    // Within next week (but not tomorrow)
    if (dayDiff > 1 && dayDiff < 7) {
        return `This ${days[date.getUTCDay()]}`;
    }

    // Same year - show month and day
    if (date.getUTCFullYear() === reference.getUTCFullYear()) {
        return `${months[date.getUTCMonth()]} ${date.getUTCDate()}`;
    }

    // Different year - show full date
    return `${months[date.getUTCMonth()]} ${date.getUTCDate()}, ${date.getUTCFullYear()}`;
}

// date_range - implemented in JavaScript
function date_range(startTimestamp, endTimestamp) {
    // Swap if needed (auto-correct)
    if (startTimestamp > endTimestamp) {
        [startTimestamp, endTimestamp] = [endTimestamp, startTimestamp];
    }

    // Convert Unix timestamps to Date objects
    const start = new Date(startTimestamp * 1000);
    const end = new Date(endTimestamp * 1000);

    const months = ['January', 'February', 'March', 'April', 'May', 'June',
                   'July', 'August', 'September', 'October', 'November', 'December'];

    const sameDay = start.getUTCFullYear() === end.getUTCFullYear() &&
                   start.getUTCMonth() === end.getUTCMonth() &&
                   start.getUTCDate() === end.getUTCDate();

    const sameMonth = start.getUTCFullYear() === end.getUTCFullYear() &&
                     start.getUTCMonth() === end.getUTCMonth();

    const sameYear = start.getUTCFullYear() === end.getUTCFullYear();

    if (sameDay) {
        return `${months[start.getUTCMonth()]} ${start.getUTCDate()}, ${start.getUTCFullYear()}`;
    }

    if (sameMonth) {
        return `${months[start.getUTCMonth()]} ${start.getUTCDate()}–${end.getUTCDate()}, ${start.getUTCFullYear()}`;
    }

    if (sameYear) {
        return `${months[start.getUTCMonth()]} ${start.getUTCDate()} – ${months[end.getUTCMonth()]} ${end.getUTCDate()}, ${start.getUTCFullYear()}`;
    }

    return `${months[start.getUTCMonth()]} ${start.getUTCDate()}, ${start.getUTCFullYear()} – ${months[end.getUTCMonth()]} ${end.getUTCDate()}, ${end.getUTCFullYear()}`;
}

// Export for testing
module.exports = { loadWasm, timeago, duration, parse_duration, human_date, date_range };

// Run tests if executed directly
if (require.main === module) {
    const yaml = require('js-yaml');

    async function runTests() {
        await loadWasm();
        console.log('WebAssembly module loaded successfully');

        const testsPath = '/tmp/whenwords/tests.yaml';
        const testsContent = fs.readFileSync(testsPath, 'utf8');
        const tests = yaml.load(testsContent);

        let passed = 0;
        let failed = 0;

        // Run timeago tests
        console.log('\n=== timeago tests ===');
        for (const test of tests.timeago) {
            const result = timeago(test.input.timestamp, test.input.reference);
            if (result === test.output) {
                passed++;
            } else {
                failed++;
                console.log(`FAIL: ${test.name}`);
                console.log(`  timeago(${test.input.timestamp}, ${test.input.reference})`);
                console.log(`  Expected: ${test.output}`);
                console.log(`  Got: ${result}`);
            }
        }

        // Run duration tests
        console.log('\n=== duration tests ===');
        for (const test of tests.duration) {
            const opts = test.input.options || {};
            const result = duration(test.input.seconds, opts.compact || false, opts.max_units || 2);
            if (result === test.output) {
                passed++;
            } else {
                failed++;
                console.log(`FAIL: ${test.name}`);
                console.log(`  duration(${test.input.seconds}, compact=${opts.compact}, max_units=${opts.max_units})`);
                console.log(`  Expected: ${test.output}`);
                console.log(`  Got: ${result}`);
            }
        }

        // Run parse_duration tests
        console.log('\n=== parse_duration tests ===');
        for (const test of tests.parse_duration) {
            const result = parse_duration(test.input);
            if (result === test.output) {
                passed++;
            } else {
                failed++;
                console.log(`FAIL: ${test.name}`);
                console.log(`  parse_duration("${test.input}")`);
                console.log(`  Expected: ${test.output}`);
                console.log(`  Got: ${result}`);
            }
        }

        // Run human_date tests
        console.log('\n=== human_date tests ===');
        for (const test of tests.human_date) {
            const result = human_date(test.input.timestamp, test.input.reference);
            if (result === test.output) {
                passed++;
            } else {
                failed++;
                console.log(`FAIL: ${test.name}`);
                console.log(`  human_date(${test.input.timestamp}, ${test.input.reference})`);
                console.log(`  Expected: ${test.output}`);
                console.log(`  Got: ${result}`);
            }
        }

        // Run date_range tests
        console.log('\n=== date_range tests ===');
        for (const test of tests.date_range) {
            const result = date_range(test.input.start, test.input.end);
            if (result === test.output) {
                passed++;
            } else {
                failed++;
                console.log(`FAIL: ${test.name}`);
                console.log(`  date_range(${test.input.start}, ${test.input.end})`);
                console.log(`  Expected: ${test.output}`);
                console.log(`  Got: ${result}`);
            }
        }

        console.log(`\n=== Results ===`);
        console.log(`Passed: ${passed}`);
        console.log(`Failed: ${failed}`);
        console.log(`Total: ${passed + failed}`);
    }

    runTests().catch(console.error);
}
