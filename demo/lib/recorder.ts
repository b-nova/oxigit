/**
 * Generic scene-based Playwright recorder for Oxigit demo videos.
 * Takes a Scene[] array and produces a video with annotations and effects.
 */

import { chromium } from "@playwright/test";
import type { Page } from "@playwright/test";
import {
  showAnnotation,
  clearAnnotation,
  smoothScrollTo,
  highlightElement,
  typeSlowly,
} from "./annotations.js";
import { composeVideo } from "./effects.js";
import type { Scene, ZoomKeyframe } from "./scene-types.js";
import { readdirSync, statSync, mkdirSync } from "node:fs";
import { join } from "node:path";

export interface RecordOptions {
  /** Base URL of the Oxigit server. Default: http://127.0.0.1:9100 */
  baseUrl?: string;
  /** Username to login with. Default: demo */
  user?: string;
  /** Password. Default: demopass123 */
  password?: string;
  /** Viewport width. Default: 1280 */
  width?: number;
  /** Viewport height. Default: 720 */
  height?: number;
  /** Whether to login before recording scenes. Default: true */
  login?: boolean;
  /** Whether to add an "oxigit.com" end card. Default: true */
  endCard?: boolean;
  /** Output filename (without extension). Default: "demo" */
  outputName?: string;
  /** Whether to apply ffmpeg post-processing. Default: true */
  postProcess?: boolean;
}

const DEFAULTS: Required<RecordOptions> = {
  baseUrl: "http://127.0.0.1:9100",
  user: "demo",
  password: "demopass123",
  width: 1280,
  height: 720,
  login: true,
  endCard: true,
  outputName: "demo",
  postProcess: true,
};

/**
 * Record a video from a list of scenes.
 * Returns the path to the final video file.
 */
export async function recordVideo(
  scenes: Scene[],
  opts?: RecordOptions,
): Promise<string> {
  const o = { ...DEFAULTS, ...opts };
  const rawDir = "./videos/raw/";
  const finalDir = "./videos/";

  mkdirSync(rawDir, { recursive: true });
  mkdirSync(finalDir, { recursive: true });
  console.log(`Recording ${scenes.length} scenes at ${o.baseUrl}...\n`);

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: o.width, height: o.height },
    recordVideo: {
      dir: rawDir,
      size: { width: o.width, height: o.height },
    },
  });

  const page = await context.newPage();
  const zoomKeyframes: ZoomKeyframe[] = [];
  const recordingStart = Date.now();

  // --- Login phase (not counted as a scene) ---
  if (o.login) {
    await page.goto(`${o.baseUrl}/login`);
    await page.waitForLoadState("networkidle");
    await page.fill("#username", o.user);
    await page.fill("#password", o.password);
    await page.click('button[type="submit"]');
    await page.waitForURL("**/repos", { timeout: 10000 });
    await page.waitForLoadState("networkidle");
    await page.waitForTimeout(500);
  }

  // --- Execute each scene ---
  for (let i = 0; i < scenes.length; i++) {
    const scene = scenes[i];
    console.log(`  Scene ${i + 1}/${scenes.length}: ${scene.annotation}`);

    // Navigate
    if (scene.route) {
      const url = scene.route.startsWith("http")
        ? scene.route
        : `${o.baseUrl}${scene.route}`;
      await page.goto(url);
      await page.waitForLoadState("networkidle");
      await page.waitForTimeout(600);
    }

    // Scroll to element
    if (scene.scrollTo) {
      const target = page.locator(scene.scrollTo).first();
      if (await target.isVisible({ timeout: 2000 }).catch(() => false)) {
        await smoothScrollTo(page, target, 1000);
      }
    }

    // Show annotation
    const holdMs = scene.waitMs ?? 3000;
    // Show annotation early but don't wait for it yet
    const annotationPromise = showAnnotation(page, scene.annotation, holdMs);

    // Highlight element (during annotation hold)
    if (scene.highlight) {
      const target = page.locator(scene.highlight).first();
      if (await target.isVisible({ timeout: 2000 }).catch(() => false)) {
        await highlightElement(page, target, Math.min(holdMs, 2000));
      }
    }

    // Zoom target — capture bounding box for ffmpeg
    if (scene.zoom) {
      const target = page.locator(scene.zoom).first();
      if (await target.isVisible({ timeout: 2000 }).catch(() => false)) {
        const box = await target.boundingBox();
        if (box) {
          const now = (Date.now() - recordingStart) / 1000;
          zoomKeyframes.push({
            startSec: now,
            endSec: now + holdMs / 1000,
            x: box.x,
            y: box.y,
            width: box.width,
            height: box.height,
          });
        }
      }
    }

    // Type into field
    if (scene.typeInto) {
      await typeSlowly(page, scene.typeInto.selector, scene.typeInto.text, 80);
    }

    // Expand diff toggle
    if (scene.expandDiff) {
      const toggle = page.locator(".turn-diff-toggle summary").first();
      if (await toggle.isVisible({ timeout: 2000 }).catch(() => false)) {
        await toggle.click();
        await page.waitForTimeout(800);
      }
    }

    // Click element (after annotation so the viewer sees what's about to be clicked)
    if (scene.click) {
      const target = page.locator(scene.click).first();
      if (await target.isVisible({ timeout: 2000 }).catch(() => false)) {
        await target.click();
        await page.waitForLoadState("networkidle");
        await page.waitForTimeout(600);
      }
    }

    // Wait for annotation to finish
    await annotationPromise;
  }

  // --- End card ---
  if (o.endCard) {
    await showAnnotation(page, "Oxigit — AI-native Git. Try it at oxigit.com", 3500);
    await clearAnnotation(page);
    await page.waitForTimeout(500);
  }

  // --- Finalize ---
  await context.close();
  await browser.close();

  // Find the most recent raw video file
  const allRaw = readdirSync(rawDir)
    .filter((f) => f.endsWith(".webm"))
    .map((f) => ({
      name: f,
      mtime: statSync(join(rawDir, f)).mtimeMs,
    }))
    .sort((a, b) => b.mtime - a.mtime);
  const rawVideoPath = join(rawDir, allRaw[0].name);

  if (o.postProcess) {
    const outputPath = join(finalDir, `${o.outputName}.mp4`);
    console.log("\nPost-processing with ffmpeg...");
    composeVideo(rawVideoPath, outputPath, zoomKeyframes);
    return outputPath;
  }

  return rawVideoPath;
}
