import puppeteer from 'puppeteer';

const run = async () => {
  const browser = await puppeteer.launch({ headless: 'new' });
  const page = await browser.newPage();

  await page.goto('https://ginandjuice.shop/login', { waitUntil: 'networkidle2' });

  // Type username
  await page.waitForSelector('input[name="username"]');
  await page.type('input[name="username"]', 'carlos');

  // Wait a moment for password field to render
  await page.waitForTimeout(1000); // adjust if needed

  // Type password
  await page.waitForSelector('input[name="password"]');
  await page.type('input[name="password"]', 'hunter2');

  // Click submit button
  await page.click('button[type="submit"]');

  // Wait for navigation
  await page.waitForNavigation({ waitUntil: 'networkidle2' });

  // Extract cookies (for playbook varManager if needed)
  const cookies = await page.cookies();
  const sessionCookie = cookies.find(c => c.name === 'session');
  console.log('✅ Session Cookie:', sessionCookie?.value || 'Not found');

  await browser.close();
};

run();
