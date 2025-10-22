#!/usr/bin/env node
/**
 * Test program for uniprof: Prime number calculation.
 * This tests CPU-intensive operations in Node.js.
 */

function isPrime(n) {
    if (n <= 1) return false;
    if (n <= 3) return true;
    if (n % 2 === 0 || n % 3 === 0) return false;

    for (let i = 5; i * i <= n; i += 6) {
        if (n % i === 0 || n % (i + 2) === 0) return false;
    }
    return true;
}

function findPrimes(max) {
    const primes = [];
    for (let i = 2; i <= max; i++) {
        if (isPrime(i)) {
            primes.push(i);
        }
    }
    return primes;
}

function sieveOfEratosthenes(max) {
    const isPrimeArray = new Array(max + 1).fill(true);
    isPrimeArray[0] = isPrimeArray[1] = false;

    for (let i = 2; i * i <= max; i++) {
        if (isPrimeArray[i]) {
            for (let j = i * i; j <= max; j += i) {
                isPrimeArray[j] = false;
            }
        }
    }

    return isPrimeArray
        .map((prime, index) => prime ? index : null)
        .filter(x => x !== null);
}

function fibonacci(n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}

function doWork() {
    console.log('Finding primes up to 50,000 using naive method...');
    let start = Date.now();
    const primes1 = findPrimes(50000);
    let elapsed = (Date.now() - start) / 1000;
    console.log(`Found ${primes1.length} primes in ${elapsed.toFixed(3)}s`);

    console.log('\nFinding primes up to 500,000 using Sieve of Eratosthenes...');
    start = Date.now();
    const primes2 = sieveOfEratosthenes(500000);
    elapsed = (Date.now() - start) / 1000;
    console.log(`Found ${primes2.length} primes in ${elapsed.toFixed(3)}s`);

    console.log('\nCalculating Fibonacci(35) recursively...');
    start = Date.now();
    const fib = fibonacci(35);
    elapsed = (Date.now() - start) / 1000;
    console.log(`Result: ${fib} in ${elapsed.toFixed(3)}s`);

    console.log('\nPerforming array operations...');
    start = Date.now();
    const data = Array.from({ length: 1000000 }, (_, i) => i);
    const squares = data.map(x => x * x);
    const evens = squares.filter(x => x % 2 === 0);
    const sum = evens.reduce((a, b) => a + b, 0);
    elapsed = (Date.now() - start) / 1000;
    console.log(`Sum: ${sum} in ${elapsed.toFixed(3)}s`);
}

console.log('Starting CPU-intensive operations for profiling...\n');
doWork();
console.log('\nDone!');
