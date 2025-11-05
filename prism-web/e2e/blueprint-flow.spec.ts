/**
 * Blueprint Flow E2E Tests
 */

import { test, expect } from '@playwright/test'

test.describe('Blueprint Management', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to blueprints page
    await page.goto('/blueprints')
  })

  test('should display blueprints page', async ({ page }) => {
    await expect(page.locator('h1')).toContainText('Blueprint Mode')
  })

  test('should create a new blueprint', async ({ page }) => {
    // Click create button
    await page.click('text=Create Blueprint')

    // Fill form
    await page.fill('input[placeholder*="Add JWT authentication"]', 'Test Blueprint E2E')
    await page.selectOption('select', 'single')
    await page.fill('input[type="number"]', '50000')

    // Submit
    await page.click('text=Create Blueprint')

    // Should show success or new blueprint in list
    await expect(page.locator('text=Test Blueprint E2E')).toBeVisible({ timeout: 5000 })
  })

  test('should approve a blueprint', async ({ page }) => {
    // Assume a blueprint exists in Pending state
    await page.click('text=Pending')
    
    // Click first blueprint
    await page.click('[data-testid="blueprint-card"]', { timeout: 5000 })

    // Click approve button
    await page.click('text=Approve')

    // Should show success
    await expect(page.locator('text=Approved')).toBeVisible({ timeout: 3000 })
  })

  test('should reject a blueprint with reason', async ({ page }) => {
    await page.click('text=Pending')
    await page.click('[data-testid="blueprint-card"]', { timeout: 5000 })

    // Click reject
    await page.click('text=Reject')

    // Enter rejection reason (handle prompt)
    page.on('dialog', async (dialog) => {
      await dialog.accept('Not ready for implementation')
    })

    await expect(page.locator('text=Rejected')).toBeVisible({ timeout: 3000 })
  })

  test('should filter blueprints by state', async ({ page }) => {
    // Click Approved filter
    await page.click('text=Approved')

    // Should only show approved blueprints
    const blueprints = await page.locator('[data-testid="blueprint-card"]').all()
    
    for (const blueprint of blueprints) {
      await expect(blueprint.locator('text=Approved')).toBeVisible()
    }
  })
})

test.describe('Blueprint Execution', () => {
  test('should execute approved blueprint', async ({ page }) => {
    await page.goto('/blueprints')

    // Find an approved blueprint
    await page.click('text=Approved')
    await page.click('[data-testid="blueprint-card"]', { timeout: 5000 })

    // Navigate to execution page
    await page.click('text=Execute')

    // Should show execution page
    await expect(page.locator('h1')).toContainText('Blueprint Execution')

    // Start execution
    await page.click('text=Start Execution')

    // Should show progress
    await expect(page.locator('text=Executing')).toBeVisible({ timeout: 3000 })
  })

  test('should display real-time progress', async ({ page }) => {
    await page.goto('/blueprints/test-bp-1/execute')

    await page.click('text=Start Execution')

    // Wait for progress updates
    await page.waitForSelector('text=Step', { timeout: 5000 })
    
    // Should show progress bar
    await expect(page.locator('[role="progressbar"]')).toBeVisible()
  })

  test('should cancel execution', async ({ page }) => {
    await page.goto('/blueprints/test-bp-1/execute')

    await page.click('text=Start Execution')
    await page.waitForTimeout(1000)

    // Cancel execution
    await page.click('text=Cancel')

    // Should stop
    await expect(page.locator('text=Executing')).not.toBeVisible({ timeout: 3000 })
  })
})
