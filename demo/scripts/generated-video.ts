import { recordVideo } from "../lib/recorder.js";
import type { Scene } from "../lib/scene-types.js";

const scenes: Scene[] = [
  {
    annotation: "The AI Hub — all coding sessions at a glance",
    route: "/demo/ai-webapp/ai",
    highlight: "a.session-card",
    waitMs: 3000,
  },
  {
    annotation: "Session detail: prompts, commits, and diffs",
    click: "a.session-card",
    zoom: ".session-header",
    waitMs: 3500,
  },
  {
    annotation: "Expand to see the generated code",
    scrollTo: ".turn-diff-toggle summary",
    expandDiff: true,
    zoom: ".diff-container",
    waitMs: 3500,
  },
  {
    annotation: "Revert, squash, or cherry-pick an entire session",
    scrollTo: "div.card:has(div.card-header:has-text('Session Operations'))",
    highlight: "div.card:has(div.card-header:has-text('Session Operations'))",
    zoom: "button.btn-danger",
    waitMs: 4000,
  },
];

recordVideo(scenes, {
  outputName: "test-skill",
}).then((path) => {
  console.log(`\nVideo ready: ${path}`);
}).catch((err) => {
  console.error("Recording failed:", err);
  process.exit(1);
});
