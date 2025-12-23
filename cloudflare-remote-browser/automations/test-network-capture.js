// Setup capture
await page.captureNetwork({
    urlPattern: 'httpbin.org/get',
    key: 'my-test-capture'
});
// Navigate
await page.goto('https://httpbin.org/get');
await page.sleep(2000);
// Verify capture
const data = await page.getCapturedResponse('my-test-capture');
// Explicitly set data to see it in "Collected Data" panel
if (data) {
    await page.setData('captured_data', data);
}
return { success: !!data, data };