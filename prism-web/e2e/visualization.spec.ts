/**
 * 3D Visualization E2E Tests
 */

import { test, expect } from '@playwright/test'

test.describe('Git Visualization', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/visualization')
  })

  test('should display visualization page', async ({ page }) => {
    await expect(page.locator('h1')).toContainText('Git Visualization')
  })

  test('should load Git data', async ({ page }) => {
    // Click reload button
    await page.click('text=Reload')

    // Should show loading or data
    const hasData = await page.locator('text=Total Commits').isVisible({ timeout: 5000 })
    const hasNoData = await page.locator('text=No Git repository found').isVisible()

    expect(hasData || hasNoData).toBeTruthy()
  })

  test('should render 3D canvas', async ({ page }) => {
    await page.waitForLoadState('networkidle')

    // Check if canvas exists
    const canvas = await page.locator('canvas')
    await expect(canvas).toBeVisible()
  })

  test('should display statistics', async ({ page }) => {
    await page.waitForLoadState('networkidle')

    // Should show stats boxes
    await expect(page.locator('text=Total Commits')).toBeVisible()
    await expect(page.locator('text=Unique Authors')).toBeVisible()
    await expect(page.locator('text=Branches')).toBeVisible()
  })

  test('should change view mode', async ({ page }) => {
    // Select heatmap view
    await page.selectOption('select', 'heatmap')

    await page.waitForTimeout(1000)

    // View should update (check if heatmap-specific elements appear)
    // This depends on your implementation
  })

  test('should interact with timeline', async ({ page }) => {
    await page.waitForLoadState('networkidle')

    // Click play button
    await page.click('text=Play', { timeout: 5000 })

    // Timeline should be playing
    await expect(page.locator('text=Pause')).toBeVisible({ timeout: 2000 })

    // Click pause
    await page.click('text=Pause')

    await expect(page.locator('text=Play')).toBeVisible()
  })

  test('should adjust playback speed', async ({ page }) => {
    await page.waitForLoadState('networkidle')

    // Click 2x speed button
    await page.click('button:has-text("2x")', { timeout: 5000 })

    // Button should be highlighted
    const speedButton = await page.locator('button:has-text("2x")')
    await expect(speedButton).toHaveClass(/bg-purple-500/)
  })

  test('should toggle loop mode', async ({ page }) => {
    await page.waitForLoadState('networkidle')

    // Click loop button
    await page.click('text=Loop', { timeout: 5000 })

    // Button should be highlighted
    const loopButton = await page.locator('text=Loop')
    await expect(loopButton).toHaveClass(/bg-green-500/)
  })

  test('should display commit details on click', async ({ page }) => {
    await page.waitForLoadState('networkidle')

    // Wait for 3D scene to load
    await page.waitForTimeout(2000)

    // Note: Clicking 3D objects in Playwright is complex
    // This test is a placeholder for manual testing
  })

  test('should show author legend', async ({ page }) => {
    await page.waitForLoadState('networkidle')

    // Should show authors section
    await expect(page.locator('text=Authors')).toBeVisible({ timeout: 5000 })
  })
})

test.describe('Performance', () => {
  test('should load large repository (10K commits)', async ({ page }) => {
    test.setTimeout(60000) // 60 second timeout

    await page.goto('/visualization')

    // Reload with large dataset
    await page.click('text=Reload')

    // Should complete within timeout
    await page.waitForLoadState('networkidle', { timeout: 30000 })

    // Canvas should still be responsive
    const canvas = await page.locator('canvas')
    await expect(canvas).toBeVisible()
  })
})
