/**
 * Record a short 10-second preview video for LinkedIn.
 * Focus: vibe coding + git — fast cuts through the AI-native features.
 * Run with: npm run preview
 */

import { chromium } from "@playwright/test";
import { showAnnotation, clearAnnotation } from "../lib/annotations.js";

const BASE = process.env.OXIGIT_URL ?? "http://127.0.0.1:9100";
const USER = "demo";
const PASS = "demopass123";
const REPO = "ai-webapp";

async function main() {
  console.log("Recording LinkedIn preview...\n");

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    recordVideo: {
      dir: "./videos/",
      size: { width: 1280, height: 720 },
    },
  });

  const page = await context.newPage();

  // Login silently (not part of the video feel — just get authenticated fast)
  await page.goto(`${BASE}/login`);
  await page.waitForLoadState("networkidle");
  await page.fill("#username", USER);
  await page.fill("#password", PASS);
  await page.click('button[type="submit"]');
  await page.waitForURL("**/repos", { timeout: 10000 });
  await page.waitForLoadState("networkidle");

  // --- Beat 1: AI Hub with sessions (~3s) ---
  await page.goto(`${BASE}/${USER}/${REPO}/ai`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(400);
  await showAnnotation(page, "Vibe coding meets Git", 2500);

  // --- Beat 2: Session detail — conversation timeline (~3s) ---
  const sessionLink = page.locator("a.session-card").first();
  if (await sessionLink.isVisible({ timeout: 2000 }).catch(() => false)) {
    await sessionLink.click();
    await page.waitForLoadState("networkidle");
    await page.waitForTimeout(400);
  }
  await showAnnotation(page, "Every prompt. Every commit. Tracked.", 2500);

  // --- Beat 3: Revert operations (~2.5s) ---
  const opsCard = page.locator("div.card", {
    has: page.locator("div.card-header", { hasText: "Session Operations" }),
  });
  if (await opsCard.isVisible({ timeout: 2000 }).catch(() => false)) {
    await opsCard.evaluate((el) =>
      el.scrollIntoView({ behavior: "smooth", block: "center" }),
    );
    await page.waitForTimeout(400);
  }
  await showAnnotation(page, "Revert. Squash. Cherry-pick.", 2000);

  // --- Beat 4: End card (~2s) ---
  await showAnnotation(page, "oxigit.com", 2000);
  await clearAnnotation(page);
  await page.waitForTimeout(300);

  await context.close();
  await browser.close();

  console.log("\nLinkedIn preview saved to demo/videos/");
}

main().catch((err) => {
  console.error("Recording failed:", err);
  process.exit(1);
});
