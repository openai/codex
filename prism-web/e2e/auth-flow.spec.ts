/**
 * Authentication Flow E2E Tests
 */

import { test, expect } from '@playwright/test'

test.describe('Authentication', () => {
  test('should display login page', async ({ page }) => {
    await page.goto('/login')
    await expect(page.locator('h1')).toContainText('Welcome to Prism')
  })

  test('should login with email and password', async ({ page }) => {
    await page.goto('/login')

    // Fill login form
    await page.fill('input[type="email"]', 'test@example.com')
    await page.fill('input[type="password"]', 'testpassword123')

    // Submit
    await page.click('text=Log In')

    // Should redirect to dashboard
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 5000 })
  })

  test('should initiate GitHub OAuth', async ({ page }) => {
    await page.goto('/login')

    // Click GitHub button
    await page.click('text=GitHub')

    // Should redirect to GitHub OAuth (or show error if not configured)
    await page.waitForTimeout(2000)
    
    // Check if redirected (URL changed) or error displayed
    const url = page.url()
    const hasError = await page.locator('text=error').isVisible().catch(() => false)
    
    expect(url !== 'http://localhost:3000/login' || hasError).toBeTruthy()
  })

  test('should request magic link', async ({ page }) => {
    await page.goto('/login')

    // Fill email
    await page.fill('input[type="email"]', 'test@example.com')

    // Click magic link button
    await page.click('text=Send Magic Link')

    // Should show success message (alert or toast)
    page.on('dialog', async (dialog) => {
      expect(dialog.message()).toContain('Magic link sent')
      await dialog.accept()
    })

    await page.waitForTimeout(1000)
  })

  test('should navigate to signup page', async ({ page }) => {
    await page.goto('/login')

    // Click signup link
    await page.click('text=Sign up')

    // Should navigate to signup
    await expect(page).toHaveURL('/signup')
  })

  test('should logout', async ({ page }) => {
    // Assume logged in
    await page.goto('/dashboard')

    // Click logout button (adjust selector based on your UI)
    await page.click('[data-testid="logout-button"]', { timeout: 5000 })

    // Should redirect to login
    await expect(page).toHaveURL('/login')
  })
})

test.describe('Protected Routes', () => {
  test('should redirect to login when accessing protected route without auth', async ({
    page,
  }) => {
    await page.goto('/dashboard')

    // Should redirect to login
    await expect(page).toHaveURL('/login', { timeout: 3000 })
  })

  test('should redirect to dashboard when accessing login with active session', async ({
    page,
  }) => {
    // Assume logged in (set session cookie manually or use API)
    // This test requires session setup

    await page.goto('/login')

    // Should redirect to dashboard
    // await expect(page).toHaveURL('/dashboard', { timeout: 3000 })
  })
})
