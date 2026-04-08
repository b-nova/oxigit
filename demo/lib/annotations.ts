/**
 * Text overlay annotations and visual helpers for Playwright video recording.
 * Injects DOM elements to make the demo video clear and easy to follow.
 */

import type { Page, Locator } from "@playwright/test";

/**
 * Show a floating annotation banner at the bottom of the viewport.
 * Fades in, holds for `durationMs`, then fades out.
 */
export async function showAnnotation(
  page: Page,
  text: string,
  durationMs = 3000,
): Promise<void> {
  await page.evaluate((params) => {
    document.getElementById("demo-annotation")?.remove();

    const el = document.createElement("div");
    el.id = "demo-annotation";
    el.textContent = params.text;
    Object.assign(el.style, {
      position: "fixed",
      bottom: "32px",
      left: "50%",
      transform: "translateX(-50%)",
      background: "rgba(12, 15, 20, 0.92)",
      color: "#e6edf3",
      padding: "16px 32px",
      borderRadius: "12px",
      fontSize: "18px",
      fontFamily: "Inter, system-ui, sans-serif",
      fontWeight: "500",
      zIndex: "99999",
      border: "1px solid rgba(139, 148, 158, 0.3)",
      boxShadow: "0 8px 32px rgba(0,0,0,0.4)",
      transition: "opacity 0.4s ease",
      opacity: "0",
      maxWidth: "80vw",
      textAlign: "center",
      letterSpacing: "0.01em",
    });
    document.body.appendChild(el);
    requestAnimationFrame(() => {
      el.style.opacity = "1";
    });
  }, { text });

  await page.waitForTimeout(durationMs);

  await page.evaluate(() => {
    const el = document.getElementById("demo-annotation");
    if (el) el.style.opacity = "0";
  });
  await page.waitForTimeout(500);
}

export async function clearAnnotation(page: Page): Promise<void> {
  await page.evaluate(() => {
    document.getElementById("demo-annotation")?.remove();
  });
}

/**
 * Smoothly scroll a locator into the center of the viewport.
 * Uses JS smooth scroll so it's visible in the video recording.
 */
export async function smoothScrollTo(
  page: Page,
  locator: Locator,
  waitAfterMs = 800,
): Promise<void> {
  await locator.evaluate((el) => {
    el.scrollIntoView({ behavior: "smooth", block: "center" });
  });
  await page.waitForTimeout(waitAfterMs);
}

/**
 * Briefly highlight an element with a glowing outline to draw attention.
 */
export async function highlightElement(
  page: Page,
  locator: Locator,
  durationMs = 2000,
): Promise<void> {
  await locator.evaluate((el) => {
    const prev = el.getAttribute("style") ?? "";
    el.setAttribute(
      "style",
      prev +
        "; outline: 3px solid rgba(88, 166, 255, 0.8); outline-offset: 4px; border-radius: 8px; transition: outline-color 0.3s ease;",
    );
    el.dataset.prevStyle = prev;
  });

  await page.waitForTimeout(durationMs);

  await locator.evaluate((el) => {
    el.setAttribute("style", el.dataset.prevStyle ?? "");
    delete el.dataset.prevStyle;
  });
}

/**
 * Type text into an input field character by character for a visible typing effect.
 */
export async function typeSlowly(
  page: Page,
  selector: string,
  text: string,
  delayMs = 80,
): Promise<void> {
  await page.click(selector);
  await page.type(selector, text, { delay: delayMs });
}
