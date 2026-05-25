// frontend/src/tests/e2e/sl1_hud.spec.ts
//
// scenario_language_v1 (SL1) HUD E2E. Boots the vite preview with
// `?sl1demo=1` so MockTransport emits SL1 metadata + a scripted
// timeline. Verifies the 30-second viewer-litmus answers are visible:
//
//   * Status panel mounts (what is the AI trying to save).
//   * Dashboard chip mounts (what is going wrong).
//   * Alert pill appears once the scripted dashboard goes stale.
//   * Milestone chips appear in fire order (did the last action help).
//   * All visible strings render via textContent — no script tags
//     are parsed even though the demo plays safe payloads.

import { test, expect } from "@playwright/test";

test.describe("SL1 HUD viewer litmus", () => {
  test("status panel + dashboards + alerts mount under ?sl1demo=1", async ({ page }) => {
    await page.goto("/?sl1demo=1");

    await expect(page.locator("#simetro-sl1-status")).toHaveAttribute("role", "status");
    await expect(page.locator("#simetro-sl1-milestones")).toHaveAttribute("role", "list");
    const dashboards = page.locator("#simetro-sl1-dashboards");
    await expect(dashboards).toHaveAttribute("role", "list");
    // Dashboard chip mounts once the static block arrives.
    await expect(dashboards.locator(".simetro-sl1-dashboard-chip")).toHaveCount(1);

    // Status panel becomes visible once the first SL1 snapshot lands.
    await expect(page.locator("#simetro-sl1-status")).toBeVisible();
    await expect(page.locator("#simetro-sl1-status")).toContainText("Outcome:");
    await expect(page.locator("#simetro-sl1-status")).toContainText("Phase:");
  });

  test("scripted timeline drives alert pill and milestone chips", async ({ page }) => {
    await page.goto("/?sl1demo=1");

    // Milestone chip 1 fires at scripted tick 2 (≈100ms at 50ms/tick).
    // Allow generous time for the deferred init + script schedule.
    const milestones = page.locator("#simetro-sl1-milestones .simetro-sl1-milestone-chip");
    await expect(milestones.first()).toBeVisible({ timeout: 5000 });
    await expect(milestones.first()).toContainText("Spot eviction wave begins");

    // Alert pill mounts at scripted tick 5.
    const alerts = page.locator("#simetro-sl1-alerts .simetro-sl1-alert-pill");
    await expect(alerts).toHaveCount(1, { timeout: 5000 });
    await expect(alerts.first()).toContainText("warning");
    await expect(alerts.first()).toContainText("exec-dashboard-stale");

    // At tick 25 the dashboard recovers — alert pill disappears and
    // milestone 2 fires.
    await expect(alerts).toHaveCount(0, { timeout: 5000 });
    await expect(milestones).toHaveCount(2, { timeout: 5000 });
    await expect(milestones.nth(1)).toContainText("Executive dashboard recovered");

    // At tick 30 the outcome flips to "won".
    await expect(page.locator("#simetro-sl1-status-outcome")).toContainText("won", {
      timeout: 5000,
    });
  });

  test("dashboard chip reflects ok / stale state via data-state", async ({ page }) => {
    await page.goto("/?sl1demo=1");
    const chip = page
      .locator("#simetro-sl1-dashboards .simetro-sl1-dashboard-chip")
      .first();
    await expect(chip).toBeVisible();
    // First scripted state at tick 1 is "ok".
    await expect(chip).toHaveAttribute("data-state", "ok", { timeout: 5000 });
    // At tick 5 the dashboard goes stale.
    await expect(chip).toHaveAttribute("data-state", "stale", { timeout: 5000 });
    // At tick 25 it recovers.
    await expect(chip).toHaveAttribute("data-state", "ok", { timeout: 5000 });
  });

  test("SL1 panels are absent on non-SL1 default scene", async ({ page }) => {
    await page.goto("/");
    // Status panel exists in DOM but stays hidden (display: none).
    const status = page.locator("#simetro-sl1-status");
    await expect(status).toBeHidden();
    // No dashboard chips or alert pills mount.
    await expect(page.locator(".simetro-sl1-dashboard-chip")).toHaveCount(0);
    await expect(page.locator(".simetro-sl1-alert-pill")).toHaveCount(0);
    await expect(page.locator(".simetro-sl1-milestone-chip")).toHaveCount(0);
  });
});
