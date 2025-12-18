// Quick capacity test script for manual testing
const { chromium } = require('playwright');

async function quickCapacityTest(maxContexts = 5) {
  console.log(`Testing capacity with ${maxContexts} concurrent contexts...`);
  
  const browser = await chromium.connectOverCDP('http://localhost:9222');
  const contexts = [];
  const startTime = Date.now();
  
  try {
    // Create contexts
    for (let i = 0; i < maxContexts; i++) {
      console.log(`Creating context ${i + 1}/${maxContexts}...`);
      const context = await browser.newContext({
        viewport: { width: 1280, height: 720 },
        userAgent: `TestContext-${i}`
      });
      
      const page = await context.newPage();
      await page.goto('https://httpbin.org/get', { timeout: 10000 });
      
      contexts.push({ context, page, index: i });
      console.log(`✓ Context ${i + 1} created successfully`);
    }
    
    const creationTime = Date.now() - startTime;
    console.log(`\nAll ${contexts.length} contexts created in ${creationTime}ms`);
    
    // Basic memory check
    const memInfo = await browser.evaluate(() => performance.memory);
    console.log(`Memory usage: ${(memInfo.usedJSHeapSize / 1024 / 1024).toFixed(1)}MB used`);
    
    // Run for 30 seconds
    console.log('Running contexts for 30 seconds...');
    await new Promise(resolve => setTimeout(resolve, 30000));
    
    console.log('Test completed successfully!');
    
  } catch (error) {
    console.error('Test failed:', error.message);
    console.log(`Created ${contexts.length} contexts before failure`);
  } finally {
    // Cleanup
    console.log('Cleaning up...');
    for (const { context } of contexts) {
      try {
        await context.close();
      } catch (e) {
        console.warn('Context cleanup failed:', e.message);
      }
    }
    await browser.close();
  }
}

// Run test if called directly
if (require.main === module) {
  const maxContexts = parseInt(process.argv[2]) || 5;
  quickCapacityTest(maxContexts).catch(console.error);
}

module.exports = { quickCapacityTest };
