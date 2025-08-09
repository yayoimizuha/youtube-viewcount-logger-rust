import {chromium} from 'npm:playwright';
import * as path from 'jsr:@std/path';

console.log(chromium.executablePath());

try {
    await Deno.remove(path.join(Deno.cwd(), 'playwright_data'), {recursive: true});
} catch (e) {
    console.error(e);
}

if (Deno.args.length != 1) {
    console.error('Usage: deno run --allow-all instagram_follower.ts username');
    Deno.exit(1);
}
const username = Deno.args[0];


const chromium_process = new Deno.Command(chromium.executablePath(), {
    args: [
        '--remote-debugging-port=9222',
        "--headless",
        '--user-data-dir=' + path.join(Deno.cwd(), 'playwright_data'),
    ]
}).spawn();

// await new Promise(resolve => setTimeout(resolve, 2 * 1000));

const browser = await chromium.connectOverCDP('http://localhost:9222', {timeout: 5000});
const ctx = await browser.newContext();
const page = await ctx.newPage();

page.on('request', (request) => {
    if (request.url().includes('https://www.instagram.com/graphql/query')) {
        request.response().then(async (response) => {
            if (response?.ok) {
                const json = await response.json();
                console.log(json);
            }
        });
    }
})

page.on('request', (request) => {
    if (request.url().includes(`https://www.instagram.com/api/v1/users/web_profile_info/?username=${username}`)) {
        request.response().then(async (response) => {
            if (response?.ok) {
                const json = await response.json();
                console.log(json);
            }
        });
    }
})
await page.goto(`https://www.instagram.com/${username}/`, {waitUntil: 'networkidle'});


await browser.close();
chromium_process.kill()