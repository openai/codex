import puppeteer from 'puppeteer';
import { VarManager } from './vars';
import type { Step } from './types';

/**
 * Execute a series of Puppeteer actions defined in a playbook step.
 */
export class PuppeteerClient {
  /**
   * Run the given actions in a headless browser and populate variables.
   */
  async run(pupBlock: { url?: string; actions: Step['puppeteer']['actions'] }, vars: VarManager): Promise<void> {
    const browser = await puppeteer.launch({ headless: 'new' });
    const page = await browser.newPage();
    // Navigate if URL is provided
    if (pupBlock.url) {
      const url = vars.substitute(pupBlock.url);
      await page.goto(url, { waitUntil: 'networkidle' });
    }
    for (const action of pupBlock.actions) {
      switch (action.type) {
        case 'type':
          await page.waitForSelector(action.selector);
          await page.type(action.selector, vars.substitute(action.text));
          break;
        case 'click':
          await page.waitForSelector(action.selector);
          await page.click(action.selector);
          break;
        case 'waitForNavigation':
          await page.waitForNavigation(action.options || {});
          break;
        case 'extractCookie':
          const cookies = await page.cookies();
          const ck = cookies.find(c => c.name === action.name);
          if (ck) vars.set(action.save_as, ck.value);
          break;
        default:
          // Unknown action
          break;
      }
    }
    await browser.close();
  }
}