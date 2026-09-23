import { expect, test } from '@playwright/test'

const routes = [
  {
    path: '/',
    screenshotName: 'landing',
    heading: 'Verifiable identity and custody for physical livestock.',
  },
  {
    path: '/problem',
    screenshotName: 'problem',
    heading: 'The asset moves. The data fragments. The market still needs proof.',
  },
  {
    path: '/future',
    screenshotName: 'future',
    heading: 'Start with what the physical asset is. Build upward from proof.',
  },
  {
    path: '/demo',
    screenshotName: 'demo',
    heading: 'Physical evidence → canonical custody',
  },
  {
    path: '/verify',
    screenshotName: 'verifier',
    heading: 'Verify evidence without trusting the interface.',
  },
] as const

const desktopViewports = [
  { width: 1366, height: 768 },
  { width: 1600, height: 900 },
  { width: 1920, height: 1080 },
] as const

test.describe('16:9 layout guardrails', () => {
  for (const viewport of desktopViewports) {
    test(`core routes fit ${viewport.width}x${viewport.height} without horizontal overflow`, async ({
      page,
    }, testInfo) => {
      await page.setViewportSize(viewport)

      for (const route of routes) {
        await page.goto(route.path)
        await expect(page.getByRole('heading', { level: 1, name: route.heading })).toBeVisible()

        const dimensions = await page.evaluate(() => ({
          clientWidth: document.documentElement.clientWidth,
          scrollWidth: document.documentElement.scrollWidth,
        }))

        expect(dimensions.scrollWidth).toBe(dimensions.clientWidth)

        await testInfo.attach(`${route.screenshotName}-${viewport.width}x${viewport.height}`, {
          body: await page.screenshot(),
          contentType: 'image/png',
        })
      }
    })
  }

  /**
   * ARRANGE: set 1600x900 viewport size and navigate to root landing page.
   * ACTION: inspect visibility of primary story and demo entrypoints.
   * ASSERT: all primary CTA links are located in the viewport without scrolling.
   * FAILURE MEANS: desktop landing page layout pushes primary paths below the fold.
   */
  test('landing keeps all three primary paths inside the first 1600x900 viewport', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1600, height: 900 })
    await page.goto('/')

    await expect(page.getByRole('link', { name: 'Open Demo' })).toBeInViewport()
    await expect(page.getByRole('link', { name: 'The Problem' })).toBeInViewport()
    await expect(page.getByRole('link', { name: 'The Future' })).toBeInViewport()
  })
})
