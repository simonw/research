import { Bash } from "npm:just-bash";

console.log("=== Advanced just-bash Tests ===\n");

// Test with network enabled for curl
const bash = new Bash({
  network: {
    dangerouslyAllowFullInternetAccess: true,
  },
});

// 1. echo -e for newlines
let result = await bash.exec('echo -e "line1\\nline2\\nline3" | grep "2"');
console.log("1. echo -e + grep:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 2. printf
result = await bash.exec('printf "Name: %s, Age: %d\\n" "Alice" 30');
console.log("2. printf:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 3. Process substitution style - diff
result = await bash.exec('echo -e "a\\nb\\nc" > /tmp/f1.txt && echo -e "a\\nx\\nc" > /tmp/f2.txt && diff /tmp/f1.txt /tmp/f2.txt');
console.log("3. diff:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log("   exitCode:", result.exitCode);
console.log();

// 4. tar
result = await bash.exec('mkdir -p /tmp/tartest && echo "data" > /tmp/tartest/file.txt && tar cf /tmp/archive.tar -C /tmp tartest && ls -la /tmp/archive.tar');
console.log("4. tar create:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 5. sqlite3
result = await bash.exec(`sqlite3 /tmp/test.db "CREATE TABLE users (id INTEGER, name TEXT); INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob'); SELECT * FROM users;"`);
console.log("5. sqlite3:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log("   stderr:", JSON.stringify(result.stderr));
console.log();

// 6. sha256sum
result = await bash.exec('echo -n "hello" | sha256sum');
console.log("6. sha256sum:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 7. Complex pipeline
result = await bash.exec('seq 1 20 | awk \'{sum+=$1} END {print "sum=" sum, "avg=" sum/NR}\'');
console.log("7. Complex pipeline (seq+awk):");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 8. xargs
result = await bash.exec('echo "1 2 3 4 5" | xargs -n 1 echo "num:"');
console.log("8. xargs:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 9. sort with options
result = await bash.exec('echo -e "banana 3\\napple 1\\ncherry 2" | sort -k2 -n');
console.log("9. sort -k2 -n:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 10. cut + paste
result = await bash.exec('echo -e "a:b:c\\nd:e:f" | cut -d: -f2');
console.log("10. cut:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 11. tr
result = await bash.exec('echo "hello world" | tr "a-z" "A-Z"');
console.log("11. tr:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 12. Environment variables persist
result = await bash.exec('export MY_VAR="test123"');
result = await bash.exec('echo $MY_VAR');
console.log("12. Env persistence across exec:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 13. Complex script with multiple features
result = await bash.exec(`
count=0
for f in /tmp/f1.txt /tmp/f2.txt; do
  lines=$(wc -l < "$f")
  count=$((count + lines))
done
echo "Total lines: $count"
`);
console.log("13. Complex script:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 14. yq (YAML processing)
result = await bash.exec('echo "name: test\\nversion: 1.0" | yq .name');
console.log("14. yq (YAML):");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log("   stderr:", JSON.stringify(result.stderr));
console.log();

// 15. tree
result = await bash.exec('mkdir -p /tmp/demo/sub1 /tmp/demo/sub2 && touch /tmp/demo/a.txt /tmp/demo/sub1/b.txt && tree /tmp/demo');
console.log("15. tree:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 16. curl (with network enabled)
console.log("16. curl test:");
try {
  result = await bash.exec('curl -s https://httpbin.org/get | jq .url');
  console.log("   stdout:", JSON.stringify(result.stdout));
  console.log("   stderr:", JSON.stringify(result.stderr));
  console.log("   exitCode:", result.exitCode);
} catch (e) {
  console.log("   error:", e.message);
}
console.log();

// 17. curl POST
console.log("17. curl POST test:");
try {
  result = await bash.exec('curl -s -X POST https://httpbin.org/post -d "key=value" | jq .form');
  console.log("   stdout:", JSON.stringify(result.stdout));
  console.log("   exitCode:", result.exitCode);
} catch (e) {
  console.log("   error:", e.message);
}
console.log();

// 18. curl with headers
console.log("18. curl with headers:");
try {
  result = await bash.exec('curl -s -H "X-Custom: test" https://httpbin.org/headers | jq .headers');
  console.log("   stdout:", result.stdout.substring(0, 200));
  console.log("   exitCode:", result.exitCode);
} catch (e) {
  console.log("   error:", e.message);
}
console.log();

// 19. du
result = await bash.exec('du -sh /tmp/demo');
console.log("19. du:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 20. Multiple redirections
result = await bash.exec('echo "out" > /tmp/out.txt 2>/tmp/err.txt && cat /tmp/out.txt');
console.log("20. Redirections:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 21. Brace expansion
result = await bash.exec('echo {a,b,c}{1,2}');
console.log("21. Brace expansion:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 22. Parameter expansion
result = await bash.exec('str="hello.world.txt"; echo "${str%.txt}"; echo "${str##*.}"');
console.log("22. Parameter expansion:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 23. Subshell
result = await bash.exec('(cd /tmp && pwd); pwd');
console.log("23. Subshell:");
console.log("   stdout:", JSON.stringify(result.stdout));
console.log();

// 24. html-to-markdown
console.log("24. html-to-markdown:");
try {
  result = await bash.exec('echo "<h1>Title</h1><p>Hello <strong>world</strong></p>" | html-to-markdown');
  console.log("   stdout:", JSON.stringify(result.stdout));
} catch (e) {
  console.log("   error:", e.message);
}
console.log();

// 25. Test exit codes
result = await bash.exec('true && echo "yes" || echo "no"');
console.log("25. Exit codes:");
console.log("   stdout:", JSON.stringify(result.stdout));

console.log("\n=== Advanced tests completed ===");
