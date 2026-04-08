/**
 * Record a demo video of Oxigit's AI features using Playwright.
 * Run with: npm run record
 * Requires: Oxigit server running with seeded data (run `npm run seed` first)
 */

import { chromium } from "@playwright/test";
import {
  showAnnotation,
  clearAnnotation,
  smoothScrollTo,
  highlightElement,
  typeSlowly,
} from "../lib/annotations.js";

const BASE = process.env.OXIGIT_URL ?? "http://127.0.0.1:9100";
const USER = "demo";
const PASS = "demopass123";
const REPO = "ai-webapp";

async function main() {
  console.log("Launching browser for recording...\n");

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    recordVideo: {
      dir: "./videos/",
      size: { width: 1280, height: 720 },
    },
  });

  const page = await context.newPage();

  // =========================================================================
  // Scene 1: Landing page
  // =========================================================================
  console.log("Scene 1: Landing page");
  await page.goto(BASE);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);
  await showAnnotation(page, "Oxigit — The AI-native Git platform", 3500);

  // =========================================================================
  // Scene 2: Login
  // =========================================================================
  console.log("Scene 2: Login");
  await page.goto(`${BASE}/login`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(800);
  await showAnnotation(page, "Step 1: Sign in to your Oxigit account", 2000);
  await typeSlowly(page, "#username", USER, 100);
  await page.waitForTimeout(300);
  await typeSlowly(page, "#password", PASS, 60);
  await page.waitForTimeout(500);
  const loginBtn = page.locator('button[type="submit"]');
  await highlightElement(page, loginBtn, 1000);
  await loginBtn.click();
  await page.waitForURL("**/repos", { timeout: 10000 });
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1500);

  // =========================================================================
  // Scene 3: Repository list → open demo repo
  // =========================================================================
  console.log("Scene 3: Repository list");
  await showAnnotation(page, "Step 2: Open a repository", 2000);
  const repoLink = page.locator(`a[href="/${USER}/${REPO}"]`).first();
  if (await repoLink.isVisible({ timeout: 3000 }).catch(() => false)) {
    await highlightElement(page, repoLink, 1500);
    await repoLink.click();
  } else {
    await page.goto(`${BASE}/${USER}/${REPO}`);
  }
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);
  await showAnnotation(
    page,
    "This repo was built with multiple AI coding tools",
    3000,
  );

  // =========================================================================
  // Scene 4: Repository settings — Install AI hooks
  // =========================================================================
  console.log("Scene 4: Repo settings — AI Hooks");
  await page.goto(`${BASE}/${USER}/${REPO}/settings`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(800);
  await showAnnotation(
    page,
    "Step 3: Install AI hooks in Repository Settings",
    2500,
  );

  // Scroll to AI Hooks section
  const aiHooksCard = page.locator("div.card", {
    has: page.locator("div.card-header", { hasText: "AI Hooks" }),
  });
  await smoothScrollTo(page, aiHooksCard, 1200);
  await highlightElement(page, aiHooksCard, 2000);
  await showAnnotation(
    page,
    "One-click install for Claude Code, Codex CLI, and Gemini CLI",
    3000,
  );

  // Install button for Claude Code
  const claudeCodeItem = page.locator("div.list-item", {
    has: page.locator("text=Claude Code"),
  });
  const installBtn = claudeCodeItem.locator('button[type="submit"]');
  if (await installBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
    const isDisabled = await installBtn.isDisabled();
    if (!isDisabled) {
      await highlightElement(page, installBtn, 1500);
      await installBtn.click();
      await page.waitForLoadState("networkidle");
      await page.waitForTimeout(1500);
      await showAnnotation(
        page,
        "Hook installed — AI metadata is now captured on every commit",
        3500,
      );
    } else {
      await highlightElement(page, claudeCodeItem, 2000);
      await showAnnotation(
        page,
        "Hook already installed — AI metadata is captured on every commit",
        3500,
      );
    }
  }

  // =========================================================================
  // Scene 5: Commits list
  // =========================================================================
  console.log("Scene 5: Commits");
  await page.goto(`${BASE}/${USER}/${REPO}/commits`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(800);
  await showAnnotation(
    page,
    "Step 4: View commits — each shows the AI tool that created it",
    3500,
  );

  // =========================================================================
  // Scene 6: Single commit detail
  // =========================================================================
  console.log("Scene 6: Commit detail");
  const commitLink = page.locator('a[href*="/commits/"]').first();
  if (await commitLink.isVisible({ timeout: 3000 }).catch(() => false)) {
    await highlightElement(page, commitLink, 1200);
    await commitLink.click();
    await page.waitForLoadState("networkidle");
  }
  await page.waitForTimeout(800);
  await showAnnotation(
    page,
    "Commit detail: full diff with AI-generated summary",
    3500,
  );
  await page.evaluate(() => window.scrollBy({ top: 400, behavior: "smooth" }));
  await page.waitForTimeout(2000);

  // =========================================================================
  // Scene 7: AI Hub — overview of all sessions
  // =========================================================================
  console.log("Scene 7: AI Hub");
  await page.goto(`${BASE}/${USER}/${REPO}/ai`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);
  await showAnnotation(
    page,
    "Step 5: The AI Hub — all coding sessions at a glance",
    3500,
  );

  // Highlight session cards to show grouping by tool
  const sessionCards = page.locator("a.session-card");
  const cardCount = await sessionCards.count();
  if (cardCount > 0) {
    // Highlight the first session card
    await highlightElement(page, sessionCards.first(), 1500);
    await showAnnotation(
      page,
      "Sessions group commits by AI tool and coding session",
      3000,
    );
  }

  // Show unsessioned commits if visible
  const unsessionedSection = page.locator("div.card-flush", {
    has: page.locator("div.card-header", { hasText: "Individual AI commits" }),
  });
  if (await unsessionedSection.isVisible({ timeout: 2000 }).catch(() => false)) {
    await smoothScrollTo(page, unsessionedSection, 800);
    await highlightElement(page, unsessionedSection, 2000);
    await showAnnotation(
      page,
      "Commits without a session ID are listed separately",
      2500,
    );
  }

  // =========================================================================
  // Scene 8: Session detail — deep dive into a session
  // =========================================================================
  console.log("Scene 8: Session detail");
  // Scroll back to top and click the first session
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: "smooth" }));
  await page.waitForTimeout(500);
  const sessionLink = page.locator("a.session-card").first();
  if (await sessionLink.isVisible({ timeout: 3000 }).catch(() => false)) {
    await highlightElement(page, sessionLink, 1200);
    await sessionLink.click();
    await page.waitForLoadState("networkidle");
    await page.waitForTimeout(1000);

    // Show the session header (tool, model, vibe score)
    const sessionHeader = page.locator(".session-header").first();
    if (await sessionHeader.isVisible({ timeout: 2000 }).catch(() => false)) {
      await highlightElement(page, sessionHeader, 2000);
    }
    await showAnnotation(
      page,
      "Session detail: tool, model, vibe score, and time range",
      3000,
    );

    // Show the conversation timeline with prompts
    const conversationCard = page.locator("div.card", {
      has: page.locator("div.card-header", { hasText: "Conversation" }),
    });
    if (await conversationCard.isVisible({ timeout: 2000 }).catch(() => false)) {
      await smoothScrollTo(page, conversationCard, 1000);
      await showAnnotation(
        page,
        "The conversation timeline shows each prompt and the commits it produced",
        3500,
      );

      // Highlight a prompt turn
      const promptTurn = page.locator(".turn-prompt").first();
      if (await promptTurn.isVisible({ timeout: 2000 }).catch(() => false)) {
        await highlightElement(page, promptTurn, 2000);
        await showAnnotation(
          page,
          "Each prompt is shown alongside the code it generated",
          3000,
        );
      }

      // Expand a diff toggle to show generated code
      const diffToggle = page.locator(".turn-diff-toggle summary").first();
      if (await diffToggle.isVisible({ timeout: 2000 }).catch(() => false)) {
        await highlightElement(page, diffToggle, 1200);
        await diffToggle.click();
        await page.waitForTimeout(800);
        await showAnnotation(
          page,
          "Expand to see the exact code generated by each prompt",
          3500,
        );

        // Scroll to show the diff content
        const diffContainer = page.locator(".diff-container").first();
        if (await diffContainer.isVisible({ timeout: 2000 }).catch(() => false)) {
          await smoothScrollTo(page, diffContainer, 1000);
          await page.waitForTimeout(2000);
        }
      }
    }

    // Show Session Operations (Revert, Squash, Cherry-pick)
    const opsCard = page.locator("div.card", {
      has: page.locator("div.card-header", { hasText: "Session Operations" }),
    });
    if (await opsCard.isVisible({ timeout: 2000 }).catch(() => false)) {
      await smoothScrollTo(page, opsCard, 1000);
      await highlightElement(page, opsCard, 2000);
      await showAnnotation(
        page,
        "Session Operations: revert, squash, or cherry-pick an entire session",
        4000,
      );

      // Highlight the Revert button specifically
      const revertBtn = page.locator("button.btn-danger", {
        hasText: "Revert Session",
      });
      if (await revertBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
        await highlightElement(page, revertBtn, 2000);
        await showAnnotation(
          page,
          "Revert Session undoes all AI-generated changes with a single revert commit",
          3500,
        );
      }

      // Highlight the Squash button
      const squashBtn = page.locator("button.btn-primary", {
        hasText: "Squash Session",
      });
      if (await squashBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
        await highlightElement(page, squashBtn, 1500);
        await showAnnotation(
          page,
          "Squash collapses all session commits into one clean commit",
          3000,
        );
      }
    }
  } else {
    console.log("  (no session links found, skipping)");
  }

  // =========================================================================
  // Scene 9: Prompt detail page
  // =========================================================================
  console.log("Scene 9: Prompt detail");
  // Navigate directly to prompt 0 of session-alpha
  await page.goto(`${BASE}/${USER}/${REPO}/ai/session-alpha/prompt/0`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);

  // Show the prompt header with the prompt text
  const promptHeader = page.locator(".session-header").first();
  if (await promptHeader.isVisible({ timeout: 3000 }).catch(() => false)) {
    await highlightElement(page, promptHeader, 2000);
  }
  await showAnnotation(
    page,
    "Prompt detail: the exact prompt and every commit it generated",
    3500,
  );

  // Show the prompt text specifically
  const promptText = page.locator(".prompt-card-text").first();
  if (await promptText.isVisible({ timeout: 2000 }).catch(() => false)) {
    await highlightElement(page, promptText, 2500);
    await showAnnotation(
      page,
      '"Create the main entry point for a Rust web server using tokio"',
      3000,
    );
  }

  // Show prompt operations (Revert Prompt, Squash Prompt)
  const promptOpsCard = page.locator("div.card", {
    has: page.locator("div.card-header", { hasText: "Prompt Operations" }),
  });
  if (await promptOpsCard.isVisible({ timeout: 2000 }).catch(() => false)) {
    await smoothScrollTo(page, promptOpsCard, 1000);
    await highlightElement(page, promptOpsCard, 2000);
    await showAnnotation(
      page,
      "Prompt Operations: revert or squash at the individual prompt level",
      4000,
    );

    // Highlight the Revert Prompt button
    const revertPromptBtn = page.locator("button.btn-danger", {
      hasText: "Revert Prompt",
    });
    if (await revertPromptBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
      await highlightElement(page, revertPromptBtn, 2500);
      await showAnnotation(
        page,
        "Revert just this prompt — undo one AI interaction without affecting the rest",
        4000,
      );
    }
  }

  // =========================================================================
  // Scene 10: Navigate to another prompt to show prompt-level granularity
  // =========================================================================
  console.log("Scene 10: Second prompt");
  await page.goto(`${BASE}/${USER}/${REPO}/ai/session-alpha/prompt/2`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);

  const promptText2 = page.locator(".prompt-card-text").first();
  if (await promptText2.isVisible({ timeout: 2000 }).catch(() => false)) {
    await highlightElement(page, promptText2, 2000);
  }
  await showAnnotation(
    page,
    "Each prompt can produce multiple commits — all tracked together",
    3500,
  );

  // Show commits section
  const commitsCard = page.locator("div.card", {
    has: page.locator("div.card-header", { hasText: "Commits" }),
  });
  if (await commitsCard.isVisible({ timeout: 2000 }).catch(() => false)) {
    await smoothScrollTo(page, commitsCard, 800);
    await highlightElement(page, commitsCard, 2000);
    await showAnnotation(
      page,
      "Two commits from a single prompt — both linked and revertible",
      3000,
    );
  }

  // =========================================================================
  // Scene 11: Repository metrics
  // =========================================================================
  console.log("Scene 11: Repository metrics");
  await page.goto(`${BASE}/${USER}/${REPO}/metrics`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(800);
  await showAnnotation(
    page,
    "Step 6: Metrics — vibe scores, tool usage, and risk flags",
    3500,
  );
  await page.evaluate(() => window.scrollBy({ top: 300, behavior: "smooth" }));
  await page.waitForTimeout(2000);

  // =========================================================================
  // Scene 12: Explore
  // =========================================================================
  console.log("Scene 12: Explore");
  await page.goto(`${BASE}/explore`);
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(800);
  await showAnnotation(
    page,
    "Step 7: Explore and discover public repositories",
    3000,
  );

  // =========================================================================
  // End card
  // =========================================================================
  console.log("Scene 13: End card");
  await showAnnotation(
    page,
    "Oxigit — AI-native Git. Try it at oxigit.com",
    4000,
  );
  await clearAnnotation(page);
  await page.waitForTimeout(800);

  // Finalize
  await context.close();
  await browser.close();

  console.log("\nDemo video saved to demo/videos/");
}

main().catch((err) => {
  console.error("Recording failed:", err);
  process.exit(1);
});
