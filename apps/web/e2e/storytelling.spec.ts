import { expect, test } from '@playwright/test'

test.describe('Product storytelling routes', () => {
  /**
   * ARRANGE: navigate to landing, problem, and future presentation routes.
   * ACTION: inspect headings and key narrative sections across all storytelling pages.
   * ASSERT: marketing value proposition, regulatory citations, and roadmap stages are visible.
   * FAILURE MEANS: product narrative or regulatory justification is broken or missing copy.
   */
  test('landing, problem and future routes render the intended product narrative', async ({ page }) => {
    await page.goto('/')
    await expect(
      page.getByRole('heading', {
        level: 1,
        name: 'Verifiable identity and custody for physical livestock.',
      }),
    ).toBeVisible()
    await expect(page.getByRole('link', { name: 'Open Demo' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'The Problem' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'The Future' })).toBeVisible()

    await page.goto('/problem')
    await expect(
      page.getByRole('heading', {
        level: 1,
        name: 'The asset moves. The data fragments. The market still needs proof.',
      }),
    ).toBeVisible()
    await expect(page.getByText('NO SHARED VERIFIABLE HISTORY')).toBeVisible()
    await expect(page.getByRole('link', { name: /MAPA \/ PNIB/ })).toBeVisible()
    await expect(page.getByRole('link', { name: /European Commission/ })).toBeVisible()
    await expect(page.getByRole('link', { name: /WOAH Terrestrial Code/ })).toBeVisible()

    await page.goto('/future')
    await expect(
      page.getByRole('heading', {
        level: 1,
        name: 'Start with what the physical asset is. Build upward from proof.',
      }),
    ).toBeVisible()
    await expect(page.getByText('PROVEN TODAY').first()).toBeVisible()
    await expect(page.getByText('EXPANSION PATH').first()).toBeVisible()
  })

  /**
   * ARRANGE: set mobile 390x844 viewport size and iterate through storytelling routes.
   * ACTION: evaluate document scrollWidth against clientWidth on mobile.
   * ASSERT: scrollWidth matches clientWidth with no horizontal page overflow.
   * FAILURE MEANS: mobile responsive layout breaks or introduces horizontal scroll.
   */
  test('presentation pages do not overflow horizontally on the mobile target viewport', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 390, height: 844 })

    for (const path of ['/', '/problem', '/future']) {
      await page.goto(path)
      const dimensions = await page.evaluate(() => ({
        clientWidth: document.documentElement.clientWidth,
        scrollWidth: document.documentElement.scrollWidth,
      }))

      expect(dimensions.scrollWidth).toBe(dimensions.clientWidth)
    }
  })
})
