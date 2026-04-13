use leptos::prelude::*;

#[component]
pub fn ComparisonPage() -> impl IntoView {
    view! {
        <div class="page-header" style="display: block; text-align: center;">
            <h1 class="page-title">"Oxigit vs the rest"</h1>
            <p class="page-subtitle">"See how Oxigit compares to Gitea, Forgejo, and GitHub."</p>
        </div>

        <div class="compare-table-wrap">
            <table class="compare-table">
                <thead>
                    <tr>
                        <th></th>
                        <th class="compare-highlight">"Oxigit"</th>
                        <th>"Gitea"</th>
                        <th>"Forgejo"</th>
                        <th>"GitHub"</th>
                    </tr>
                </thead>
                <tbody>
                    // Core Git
                    <tr class="compare-category-row">
                        <td colspan="5">"Core Git"</td>
                    </tr>
                    <CompareRow feature="Git repos (public & private)" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                    <CompareRow feature="Issues & pull requests" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                    <CompareRow feature="Code browsing & blame" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                    <CompareRow feature="SSH & HTTP transport" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                    <CompareRow feature="Forking & branching" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                    <CompareRow feature="Merge conflict resolution UI" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::Yes />

                    // AI Features
                    <tr class="compare-category-row">
                        <td colspan="5">"AI Features"</td>
                    </tr>
                    <CompareRow feature="AI-aware commits" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::No />
                    <CompareRow feature="AI session tracking" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::No />
                    <CompareRow feature="Vibe scores & metrics" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::No />
                    <CompareRow feature="Prompt history & replay" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::No />
                    <CompareRow feature="AI Hub dashboard" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::No />
                    <CompareRow feature="Recipes (shareable AI workflows)" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::No />
                    <CompareRow feature="AI diff summaries & risk flags" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::Partial />
                    <CompareRow feature="Deploy previews" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::Partial />

                    // Collaboration
                    <tr class="compare-category-row">
                        <td colspan="5">"Collaboration"</td>
                    </tr>
                    <CompareRow feature="Organizations & teams" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                    <CompareRow feature="Code review" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                    <CompareRow feature="Guardrails & policies" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::Partial />

                    // Hosting & Licensing
                    <tr class="compare-category-row">
                        <td colspan="5">"Hosting & Licensing"</td>
                    </tr>
                    <CompareRow feature="Self-hosted" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Partial />
                    <CompareRow feature="Cloud / SaaS" oxigit=Support::Yes gitea=Support::Partial forgejo=Support::Partial github=Support::Yes />
                    <CompareRow feature="Open source" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::No />
                    <CompareRow feature="Written in Rust" oxigit=Support::Yes gitea=Support::No forgejo=Support::No github=Support::No />
                    <CompareRow feature="Free tier" oxigit=Support::Yes gitea=Support::Yes forgejo=Support::Yes github=Support::Yes />
                </tbody>
            </table>
        </div>

        // Platform detail sections
        <div class="compare-sections">
            <div class="card compare-section compare-section-oxigit">
                <h3>"Oxigit"</h3>
                <p class="compare-section-tagline">"The AI-native Git platform"</p>
                <p>
                    "Oxigit is built from the ground up for the age of AI-assisted development. "
                    "Every commit can carry AI metadata \u{2014} which tool wrote it, what prompt was used, "
                    "and how risky the changes are. Features like Vibe Scores, the AI Hub, and Recipes "
                    "give teams visibility and control over AI-generated code."
                </p>
                <ul>
                    <li>"Open source and self-hostable"</li>
                    <li>"Written in Rust for performance and reliability"</li>
                    <li>"Built-in AI session tracking and prompt history"</li>
                    <li>"Recipes let you share and replay AI workflows"</li>
                    <li>"Deploy previews and conflict resolution UI included"</li>
                </ul>
            </div>

            <div class="card compare-section">
                <h3>"Gitea"</h3>
                <p class="compare-section-tagline">"Lightweight self-hosted Git service"</p>
                <p>
                    "Gitea is a mature, lightweight Git hosting solution written in Go. "
                    "It covers the essentials \u{2014} repos, issues, pull requests, and CI \u{2014} and is easy to deploy. "
                    "However, it has no built-in awareness of AI-generated code or tooling."
                </p>
                <ul>
                    <li>"Easy to deploy with low resource usage"</li>
                    <li>"Large plugin and integration ecosystem"</li>
                    <li>"No AI-specific features"</li>
                    <li>"No built-in deploy previews or conflict resolution UI"</li>
                </ul>
            </div>

            <div class="card compare-section">
                <h3>"Forgejo"</h3>
                <p class="compare-section-tagline">"Community-governed Gitea fork"</p>
                <p>
                    "Forgejo is a soft fork of Gitea with a focus on community governance and sustainability. "
                    "Functionally similar to Gitea, it prioritizes transparency and contributor-friendly processes. "
                    "Like Gitea, it lacks AI-native features."
                </p>
                <ul>
                    <li>"Community-first governance model"</li>
                    <li>"Compatible with Gitea\u{2019}s ecosystem"</li>
                    <li>"No AI-specific features"</li>
                    <li>"Focus on stability and federation"</li>
                </ul>
            </div>

            <div class="card compare-section">
                <h3>"GitHub"</h3>
                <p class="compare-section-tagline">"The industry standard"</p>
                <p>
                    "GitHub is the largest Git hosting platform with an unmatched ecosystem. "
                    "While GitHub Copilot adds AI code completion, GitHub itself doesn\u{2019}t track AI provenance "
                    "at the commit level \u{2014} you can\u{2019}t see which commits were AI-generated, what prompts were used, "
                    "or assess the risk of AI-authored changes."
                </p>
                <ul>
                    <li>"Largest ecosystem and community"</li>
                    <li>"Copilot for code completion (separate product)"</li>
                    <li>"Closed source, cloud-first"</li>
                    <li>"No AI commit tracking or session-level visibility"</li>
                    <li>"Self-hosted option (Enterprise) is expensive"</li>
                </ul>
            </div>
        </div>

        // CTA
        <div class="compare-cta">
            <h2>"Ready to try the AI-native way?"</h2>
            <p>"Oxigit is free, open source, and ready to use."</p>
            <div class="compare-cta-buttons">
                <a href="/explore" class="btn btn-primary">"Explore public repos"</a>
                <a href="/register" class="btn">"Create an account"</a>
            </div>
        </div>
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Support {
    Yes,
    No,
    Partial,
}

#[component]
fn CompareRow(
    feature: &'static str,
    oxigit: Support,
    gitea: Support,
    forgejo: Support,
    github: Support,
) -> impl IntoView {
    let cell = |s: Support| match s {
        Support::Yes => view! { <td class="compare-cell"><span class="compare-check" title="Yes">{"\u{2713}"}</span></td> },
        Support::No => view! { <td class="compare-cell"><span class="compare-cross" title="No">{"\u{2717}"}</span></td> },
        Support::Partial => view! { <td class="compare-cell"><span class="compare-partial" title="Partial">{"\u{25D0}"}</span></td> },
    };

    view! {
        <tr>
            <td class="compare-feature">{feature}</td>
            {cell(oxigit)}
            {cell(gitea)}
            {cell(forgejo)}
            {cell(github)}
        </tr>
    }
}
