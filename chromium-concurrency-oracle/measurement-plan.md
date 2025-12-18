# Chromium Concurrency Measurement Plan

## Load Testing Strategy

### Test Environment Setup
```yaml
# docker-compose.test.yml
services:
  chromium-under-test:
    image: lscr.io/linuxserver/chromium:latest
    environment:
      - CHROME_CLI=--remote-debugging-port=9222 --remote-debugging-address=0.0.0.0 --disable-gpu --no-sandbox --disable-dev-shm-usage
    ports:
      - "9222:9222"
    shm_size: 2gb
    deploy:
      resources:
        limits:
          cpus: '8.0'
          memory: 12GB
        reservations:
          cpus: '2.0'
          memory: 4GB

  playwright-tester:
    image: node:18
    volumes:
      - ./test-scripts:/app
    working_dir: /app
    command: npm test
```

### Concurrent Context Test Script
```javascript
// test-scripts/concurrent-contexts.js
const { chromium } = require('playwright');

async function testConcurrentContexts(maxContexts, durationMinutes = 5) {
  const browser = await chromium.connectOverCDP('http://chromium-under-test:9222');
  const contexts = [];
  const results = {
    maxContextsAttempted: maxContexts,
    contextsCreated: 0,
    failures: [],
    memoryUsage: [],
    responseTimes: []
  };

  try {
    // Create contexts incrementally
    for (let i = 0; i < maxContexts; i++) {
      try {
        console.log(`Creating context ${i + 1}/${maxContexts}`);
        const startTime = Date.now();
        
        const context = await browser.newContext({
          viewport: { width: 1280, height: 720 },
          userAgent: `TestAgent-${i}`
        });
        
        const page = await context.newPage();
        await page.goto('https://httpbin.org/get', { timeout: 30000 });
        
        const responseTime = Date.now() - startTime;
        contexts.push({ context, page, index: i, responseTime });
        results.contextsCreated++;
        results.responseTimes.push(responseTime);
        
        console.log(`✓ Context ${i + 1} created in ${responseTime}ms`);
        
        // Basic interaction test
        await page.evaluate(() => {
          const div = document.createElement('div');
          div.textContent = `Context ${i}`;
          document.body.appendChild(div);
          return div.textContent;
        });
        
      } catch (error) {
        console.log(`✗ Context ${i + 1} failed: ${error.message}`);
        results.failures.push({
          contextIndex: i,
          error: error.message,
          timestamp: new Date().toISOString()
        });
        break; // Stop on first failure
      }
    }
    
    // Monitor memory during test
    const monitoringInterval = setInterval(async () => {
      try {
        const memInfo = await browser.evaluate(() => {
          // Get Chrome memory info via CDP
          return performance.memory;
        });
        results.memoryUsage.push({
          timestamp: new Date().toISOString(),
          contexts: contexts.length,
          ...memInfo
        });
      } catch (e) {
        console.warn('Memory monitoring failed:', e.message);
      }
    }, 1000);
    
    // Run test duration
    console.log(`Running ${contexts.length} contexts for ${durationMinutes} minutes...`);
    await new Promise(resolve => setTimeout(resolve, durationMinutes * 60 * 1000));
    
    clearInterval(monitoringInterval);
    
  } finally {
    // Cleanup
    console.log('Cleaning up contexts...');
    for (const { context } of contexts) {
      try {
        await context.close();
      } catch (e) {
        console.warn('Context cleanup failed:', e.message);
      }
    }
    await browser.close();
  }
  
  return results;
}

// Run tests with different concurrency levels
async function runCapacityTests() {
  const testLevels = [1, 3, 5, 7, 10, 15];
  const results = {};
  
  for (const level of testLevels) {
    console.log(`\n=== Testing ${level} concurrent contexts ===`);
    try {
      const result = await testConcurrentContexts(level, 2); // 2 minute test
      results[level] = result;
      console.log(`Result: ${result.contextsCreated}/${level} contexts created`);
      
      if (result.failures.length > 0) {
        console.log('Failures:', result.failures.length);
        break; // Stop testing higher levels if current fails
      }
    } catch (error) {
      console.error(`Test failed at level ${level}:`, error.message);
      results[level] = { error: error.message };
      break;
    }
  }
  
  // Save results
  require('fs').writeFileSync(
    '/app/results.json', 
    JSON.stringify(results, null, 2)
  );
  
  return results;
}

runCapacityTests().catch(console.error);
```

### Memory Pressure Test
```javascript
// test-scripts/memory-pressure.js
async function testMemoryPressure(contextCount, pageCountPerContext = 3) {
  const browser = await chromium.connectOverCDP(process.env.CDP_ENDPOINT);
  const contexts = [];
  
  for (let i = 0; i < contextCount; i++) {
    const context = await browser.newContext();
    const pages = [];
    
    for (let j = 0; j < pageCountPerContext; j++) {
      const page = await context.newPage();
      // Load memory-intensive content
      await page.goto('https://httpbin.org/stream/100'); // Large response
      await page.evaluate(() => {
        // Create memory pressure
        const arrays = [];
        for (let k = 0; k < 10; k++) {
          arrays.push(new Array(1000000).fill(Math.random()));
        }
        return arrays.length;
      });
      pages.push(page);
    }
    
    contexts.push({ context, pages });
  }
  
  // Monitor memory for 30 seconds
  const memoryReadings = [];
  const interval = setInterval(async () => {
    try {
      const memInfo = await browser.evaluate(() => performance.memory);
      memoryReadings.push({
        timestamp: Date.now(),
        ...memInfo,
        contexts: contextCount,
        totalPages: contextCount * pageCountPerContext
      });
    } catch (e) {
      console.warn('Memory read failed:', e.message);
    }
  }, 1000);
  
  await new Promise(resolve => setTimeout(resolve, 30000));
  clearInterval(interval);
  
  // Cleanup
  for (const { context } of contexts) {
    await context.close();
  }
  await browser.close();
  
  return memoryReadings;
}
```

### CPU Utilization Test
```javascript
// test-scripts/cpu-utilization.js
async function testCPUUtilization(contextCount) {
  const browser = await chromium.connectOverCDP(process.env.CDP_ENDPOINT);
  const contexts = [];
  
  // Create contexts with CPU-intensive tasks
  for (let i = 0; i < contextCount; i++) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto('about:blank');
    
    // Start CPU-intensive JavaScript
    await page.evaluate(() => {
      function fibonacci(n) {
        return n <= 1 ? n : fibonacci(n - 1) + fibonacci(n - 2);
      }
      
      // Run CPU-intensive calculation in loop
      setInterval(() => {
        fibonacci(35); // Moderate CPU load
      }, 100);
    });
    
    contexts.push({ context, page });
  }
  
  // Monitor for 60 seconds
  const cpuReadings = [];
  const startTime = Date.now();
  
  while (Date.now() - startTime < 60000) {
    // In a real test, you'd collect system CPU metrics here
    // For now, just track context count and time
    cpuReadings.push({
      timestamp: Date.now(),
      contexts: contextCount,
      elapsed: Date.now() - startTime
    });
    
    await new Promise(resolve => setTimeout(resolve, 5000));
  }
  
  // Cleanup
  for (const { context } of contexts) {
    await context.close();
  }
  await browser.close();
  
  return cpuReadings;
}
```

## Metrics to Collect

### Performance Metrics
- **Context Creation Time**: Time to create new browser context
- **Page Load Time**: Time for initial page navigation
- **Memory Usage**: Chrome process memory consumption
- **CPU Utilization**: System CPU usage per context count
- **Response Times**: API call latency under load

### Stability Metrics  
- **Failure Rate**: Contexts that fail to create or crash
- **Recovery Time**: Time to recover from failures
- **Resource Limits**: Memory/CPU thresholds that cause failures

### Success Criteria
- **Maximum Stable Contexts**: Highest count with <5% failure rate
- **Memory per Context**: Average MB per additional context
- **CPU Scaling Factor**: CPU increase per additional context
- **Recovery Time**: Time to restore service after failure

## Test Execution

```bash
# Run capacity test
docker-compose -f docker-compose.test.yml up -d chromium-under-test
sleep 30  # Wait for Chrome to start
docker-compose -f docker-compose.test.yml run --rm playwright-tester

# Collect results
docker cp playwright-tester:/app/results.json ./results.json
```

## Expected Results Analysis

Based on research, expect:
- **1-3 contexts**: Stable, low resource usage
- **4-7 contexts**: Moderate performance degradation  
- **8-10 contexts**: High failure rate, memory pressure
- **10+ contexts**: System instability, OOM kills
