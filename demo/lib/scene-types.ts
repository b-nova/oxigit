/**
 * Shared scene type definitions for the video recorder.
 */

export interface Scene {
  /** Text overlay annotation shown at the bottom of the viewport. */
  annotation: string;

  /** Oxigit route to navigate to (e.g. "/demo/ai-webapp/ai"). Full path from root. */
  route?: string;

  /** Playwright selector to click. */
  click?: string;

  /** Playwright selector to smooth-scroll into view. */
  scrollTo?: string;

  /** Playwright selector to highlight with a glow outline. */
  highlight?: string;

  /**
   * Playwright selector to zoom into via ffmpeg post-processing.
   * The recorder captures the element's bounding box and timestamps
   * so ffmpeg can crop+scale that region in the final video.
   */
  zoom?: string;

  /** Type text into an input field with a visible typing effect. */
  typeInto?: { selector: string; text: string };

  /** Expand a diff toggle (.turn-diff-toggle) by clicking its summary. */
  expandDiff?: boolean;

  /** How long to hold this scene in ms (default 3000). */
  waitMs?: number;
}

/** Metadata recorded during Playwright execution for ffmpeg post-processing. */
export interface ZoomKeyframe {
  /** Start time in seconds from the beginning of the video. */
  startSec: number;
  /** End time in seconds. */
  endSec: number;
  /** Bounding box of the zoom target in the 1280x720 viewport. */
  x: number;
  y: number;
  width: number;
  height: number;
}
