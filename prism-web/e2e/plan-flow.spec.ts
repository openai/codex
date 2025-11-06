/**
 * Plan Flow E2E Tests
 */

import { test, expect } from '@playwright/test'

test.describe('Plan Management', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to Plans page
    await page.goto('/Plans')
  })

  test('should display Plans page', async ({ page }) => {
    await expect(page.locator('h1')).toContainText('plan mode')
  })

  test('should create a new Plan', async ({ page }) => {
    // Click create button
    await page.click('text=Create Plan')

    // Fill form
    await page.fill('input[placeholder*="Add JWT authentication"]', 'Test Plan E2E')
    await page.selectOption('select', 'single')
    await page.fill('input[type="number"]', '50000')

    // Submit
    await page.click('text=Create Plan')

    // Should show success or new Plan in list
    await expect(page.locator('text=Test Plan E2E')).toBeVisible({ timeout: 5000 })
  })

  test('should approve a Plan', async ({ page }) => {
    // Assume a Plan exists in Pending state
    await page.click('text=Pending')
    
    // Click first Plan
    await page.click('[data-testid="Plan-card"]', { timeout: 5000 })

    // Click approve button
    await page.click('text=Approve')

    // Should show success
    await expect(page.locator('text=Approved')).toBeVisible({ timeout: 3000 })
  })

  test('should reject a Plan with reason', async ({ page }) => {
    await page.click('text=Pending')
    await page.click('[data-testid="Plan-card"]', { timeout: 5000 })

    // Click reject
    await page.click('text=Reject')

    // Enter rejection reason (handle prompt)
    page.on('dialog', async (dialog) => {
      await dialog.accept('Not ready for implementation')
    })

    await expect(page.locator('text=Rejected')).toBeVisible({ timeout: 3000 })
  })

  test('should filter Plans by state', async ({ page }) => {
    // Click Approved filter
    await page.click('text=Approved')

    // Should only show approved Plans
    const Plans = await page.locator('[data-testid="Plan-card"]').all()
    
    for (const Plan of Plans) {
      await expect(Plan.locator('text=Approved')).toBeVisible()
    }
  })
})

test.describe('Plan Execution', () => {
  test('should execute approved Plan', async ({ page }) => {
    await page.goto('/Plans')

    // Find an approved Plan
    await page.click('text=Approved')
    await page.click('[data-testid="Plan-card"]', { timeout: 5000 })

    // Navigate to execution page
    await page.click('text=Execute')

    // Should show execution page
    await expect(page.locator('h1')).toContainText('Plan Execution')

    // Start execution
    await page.click('text=Start Execution')

    // Should show progress
    await expect(page.locator('text=Executing')).toBeVisible({ timeout: 3000 })
  })

  test('should display real-time progress', async ({ page }) => {
    await page.goto('/plans/test-bp-1/execute')

    await page.click('text=Start Execution')

    // Wait for progress updates
    await page.waitForSelector('text=Step', { timeout: 5000 })
    
    // Should show progress bar
    await expect(page.locator('[role="progressbar"]')).toBeVisible()
  })

  test('should cancel execution', async ({ page }) => {
    await page.goto('/plans/test-bp-1/execute')

    await page.click('text=Start Execution')
    await page.waitForTimeout(1000)

    // Cancel execution
    await page.click('text=Cancel')

    // Should stop
    await expect(page.locator('text=Executing')).not.toBeVisible({ timeout: 3000 })
  })
})
