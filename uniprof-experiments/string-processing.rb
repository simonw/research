#!/usr/bin/env ruby
# Test program for uniprof: String processing in Ruby.
# This tests various CPU-intensive string operations.

def generate_text(length)
  words = %w[the quick brown fox jumps over lazy dog and cat]
  Array.new(length) { words.sample }.join(' ')
end

def count_words(text)
  text.downcase.scan(/\w+/).each_with_object(Hash.new(0)) do |word, counts|
    counts[word] += 1
  end
end

def find_palindromes(text)
  words = text.downcase.scan(/\w+/)
  words.select { |word| word.length > 2 && word == word.reverse }
end

def reverse_words(text)
  text.split.map(&:reverse).join(' ')
end

def calculate_fibonacci(n)
  return n if n <= 1
  calculate_fibonacci(n - 1) + calculate_fibonacci(n - 2)
end

def do_work
  puts "Generating large text..."
  start = Time.now
  text = generate_text(100_000)
  elapsed = Time.now - start
  puts "Generated #{text.length} characters in #{elapsed.round(3)}s"

  puts "\nCounting words..."
  start = Time.now
  word_counts = count_words(text)
  elapsed = Time.now - start
  puts "Found #{word_counts.length} unique words in #{elapsed.round(3)}s"

  puts "\nFinding palindromes..."
  start = Time.now
  palindromes = find_palindromes(text)
  elapsed = Time.now - start
  puts "Found #{palindromes.uniq.length} unique palindromes in #{elapsed.round(3)}s"

  puts "\nReversing words..."
  start = Time.now
  reversed = reverse_words(text[0..10_000])
  elapsed = Time.now - start
  puts "Reversed #{reversed.split.length} words in #{elapsed.round(3)}s"

  puts "\nCalculating Fibonacci(30)..."
  start = Time.now
  fib = calculate_fibonacci(30)
  elapsed = Time.now - start
  puts "Result: #{fib} in #{elapsed.round(3)}s"

  puts "\nPerforming array operations..."
  start = Time.now
  data = (1..100_000).map { |i| i ** 2 }
  filtered = data.select { |x| x % 3 == 0 }
  sum = filtered.reduce(:+)
  elapsed = Time.now - start
  puts "Sum: #{sum} in #{elapsed.round(3)}s"
end

puts "Starting CPU-intensive operations for profiling...\n\n"
do_work
puts "\nDone!"
