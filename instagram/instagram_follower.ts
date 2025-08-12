import {chromium} from 'npm:playwright';
import * as path from 'jsr:@std/path';
import * as fs from 'node:fs'
import * as process from 'node:process'
import {Browser, BrowserContext, Page} from "npm:playwright-core";

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
        '--user-data-dir=' + path.join(Deno.cwd(), 'playwright_data'),
    ].concat((() => {
        switch (process.platform) {
            default:
                return ['--headless', '--no-sandbox'] as string[]
            case "win32":
                return [] as string[]
        }
    })())
}).spawn();

// await new Promise(resolve => setTimeout(resolve, 2 * 1000));

const browser: Browser = await chromium.connectOverCDP('http://localhost:9222', {timeout: 5000});
const ctx: BrowserContext = await browser.newContext();
const page: Page = await ctx.newPage();

page.on('request', (request) => {
    if (request.url().includes('https://www.instagram.com/graphql/query')) {
        request.response().then(async (response) => {
            if (response?.ok) {
                const json = await response.json();
                console.log(json);
                fs.writeFileSync('query.json', JSON.stringify(json, null, 2));
            }
        });
    }
})

page.on('request', (request) => {
    if (request.url().includes(`https://www.instagram.com/api/v1/users/web_profile_info/?username=${username}`)) {
        request.response().then(async (response) => {
            console.log(json);
            if (response?.ok) {
                const json = await response.json();
                // console.log(json);
                fs.writeFileSync('web_profile_info.json', JSON.stringify(json, null, 2));

            }
        });
    }
})
try {
    const resp = await page.goto(`https://www.instagram.com/${username}/`, {timeout: 5000})
    console.log(resp.status())
} catch (e) {
    console.error(e)
}

await browser.close();
chromium_process.kill()

