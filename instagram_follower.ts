import {chromium} from 'npm:playwright';

console.log(chromium.executablePath());
const chromium_process = new Deno.Command(chromium.executablePath(), {
    args: [
        '--remote-debugging-port=9222',
        // "--headless"
        '--user-data-dir=C:\\Users\\tomokazu\\RustroverProjects\\youtube-viewcount-logger-rust\\playwright_data',
    ]
}).spawn();

// sleep 5 secs
await new Promise(resolve => setTimeout(resolve, 10 * 1000));

await (async () => {
    const browser = await chromium.connectOverCDP('http://localhost:9222', {timeout: 5000});
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    await page.goto('https://example.com');
    await new Promise(resolve => setTimeout(resolve, 5000));
    await browser.close();
})()


chromium_process.kill()