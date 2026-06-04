import { Bash } from "npm:just-bash";

console.log("=== Testing just-bash in Deno ===\n");

// 1. Basic echo
const bash = new Bash();
let result = await bash.exec("echo 'Hello from just-bash!'");
console.log("1. Basic echo:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log("   exitCode:", result.exitCode);
console.log();

// 2. Variables and expansion
result = await bash.exec('NAME="World" && echo "Hello, $NAME!"');
console.log("2. Variables:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 3. Pipes
result = await bash.exec('echo "banana\\napple\\ncherry" | sort');
console.log("3. Pipes (sort):");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 4. File operations (virtual FS)
result = await bash.exec('echo "hello world" > /tmp/test.txt && cat /tmp/test.txt');
console.log("4. File write + read:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 5. Persistence - read the file we wrote above
result = await bash.exec('cat /tmp/test.txt');
console.log("5. Persistent file read:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 6. jq
result = await bash.exec('echo \'{"name":"deno","version":2}\' | jq .name');
console.log("6. jq:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 7. grep
result = await bash.exec('echo "line1\\nline2\\nline3" | grep "2"');
console.log("7. grep:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 8. sed
result = await bash.exec('echo "hello world" | sed "s/world/deno/"');
console.log("8. sed:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 9. awk
result = await bash.exec('echo "a 1\\nb 2\\nc 3" | awk \'{print $2, $1}\'');
console.log("9. awk:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 10. Arithmetic
result = await bash.exec('echo $((6 * 7))');
console.log("10. Arithmetic:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 11. Loops
result = await bash.exec('for i in 1 2 3; do echo "item $i"; done');
console.log("11. Loops:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 12. Functions
result = await bash.exec('greet() { echo "Hi, $1!"; }; greet Deno');
console.log("12. Functions:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 13. Conditionals
result = await bash.exec('if [ 5 -gt 3 ]; then echo "yes"; else echo "no"; fi');
console.log("13. Conditionals:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 14. find
result = await bash.exec('mkdir -p /tmp/a/b/c && touch /tmp/a/x.txt /tmp/a/b/y.txt && find /tmp/a -name "*.txt"');
console.log("14. find:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 15. wc, head, tail
result = await bash.exec('seq 1 100 | wc -l');
console.log("15. wc (count 1-100):");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 16. base64
result = await bash.exec('echo -n "hello" | base64');
console.log("16. base64:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 17. Error handling
result = await bash.exec('cat /nonexistent 2>&1');
console.log("17. Error (cat nonexistent):");
console.log("   stderr:", JSON.stringify(result.stderr));
console.log("   exitCode:", result.exitCode);
console.log();

// 18. Command substitution
result = await bash.exec('echo "Today is $(date +%Y)"');
console.log("18. Command substitution:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 19. Here document
result = await bash.exec('cat <<EOF\nline1\nline2\nEOF');
console.log("19. Here document:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 20. Arrays
result = await bash.exec('arr=(one two three); echo ${arr[1]}; echo ${#arr[@]}');
console.log("20. Arrays:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

console.log("=== All basic tests completed ===");
