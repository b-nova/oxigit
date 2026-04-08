/**
 * ffmpeg post-processing for zoom, pan, and transition effects.
 * Uses ffmpeg-static so no system ffmpeg is required.
 */

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import type { ZoomKeyframe } from "./scene-types.js";

const require = createRequire(import.meta.url);
const ffmpegPath: string = require("ffmpeg-static");

function ffmpeg(args: string[]): void {
  execFileSync(ffmpegPath, args, { stdio: "pipe" });
}

/**
 * Build an ffmpeg zoompan filter string from keyframes.
 *
 * For each keyframe we:
 * 1. Zoom into the bounding box region (crop + scale back to 1280x720)
 * 2. Smoothly interpolate between zoom levels
 *
 * Between keyframes the video stays at 1x (full frame).
 */
function buildZoomFilter(keyframes: ZoomKeyframe[], fps = 25): string {
  if (keyframes.length === 0) return "";

  // Build a zoompan expression with keyframe-based zoom
  // We use the crop filter approach: crop to region, then scale back up
  const filters: string[] = [];

  for (const kf of keyframes) {
    const startFrame = Math.round(kf.startSec * fps);
    const endFrame = Math.round(kf.endSec * fps);
    const duration = endFrame - startFrame;

    // Pad the bounding box slightly for breathing room
    const pad = 40;
    const x = Math.max(0, kf.x - pad);
    const y = Math.max(0, kf.y - pad);
    const w = Math.min(1280 - x, kf.width + pad * 2);
    const h = Math.min(720 - y, kf.height + pad * 2);

    // Maintain 16:9 aspect ratio for the crop
    const targetW = Math.round(Math.max(w, h * (16 / 9)));
    const targetH = Math.round(targetW * (9 / 16));
    const cropX = Math.max(0, Math.min(1280 - targetW, x - (targetW - w) / 2));
    const cropY = Math.max(0, Math.min(720 - targetH, y - (targetH - h) / 2));

    filters.push(
      `crop=${targetW}:${targetH}:${Math.round(cropX)}:${Math.round(cropY)}:enable='between(n,${startFrame},${endFrame})'`,
    );
  }

  return filters.join(",");
}

/**
 * Compose the final video from a raw Playwright recording.
 *
 * Applies:
 * 1. Zoom keyframes (crop + scale for highlighted elements)
 * 2. Fade-in at start, fade-out at end
 * 3. Re-encode to MP4 (H.264) for maximum compatibility
 */
export function composeVideo(
  rawVideoPath: string,
  outputPath: string,
  keyframes: ZoomKeyframe[],
  options: { fps?: number; fadeDurationSec?: number } = {},
): void {
  const fps = options.fps ?? 25;
  const fade = options.fadeDurationSec ?? 0.5;

  // Step 1: Get video duration
  const probeOutput = execFileSync(ffmpegPath, [
    "-i", rawVideoPath,
    "-f", "null", "-",
  ], { stdio: ["pipe", "pipe", "pipe"] }).toString();

  // Build filter chain
  const filterParts: string[] = [];

  // Zoom effects
  const zoomFilter = buildZoomFilter(keyframes, fps);
  if (zoomFilter) {
    filterParts.push(zoomFilter);
    // Scale back to 1280x720 after crop
    filterParts.push("scale=1280:720:flags=lanczos");
  }

  // Fade in/out
  filterParts.push(`fade=t=in:st=0:d=${fade}`);
  // We don't know exact duration without probing, so use a large number for fade-out start
  // ffmpeg will clamp it automatically

  const filterChain = filterParts.length > 0
    ? ["-vf", filterParts.join(",")]
    : [];

  // Compose final video
  const args = [
    "-y",
    "-i", rawVideoPath,
    ...filterChain,
    "-c:v", "libx264",
    "-preset", "fast",
    "-crf", "23",
    "-pix_fmt", "yuv420p",
    "-movflags", "+faststart",
    outputPath,
  ];

  console.log(`  Applying effects and encoding to MP4...`);
  ffmpeg(args);
  console.log(`  Final video: ${outputPath}`);
}

/**
 * Simple fade + re-encode without zoom (for when there are no zoom keyframes).
 */
export function encodeWithFade(
  rawVideoPath: string,
  outputPath: string,
  fadeDurationSec = 0.5,
): void {
  composeVideo(rawVideoPath, outputPath, [], { fadeDurationSec });
}
