use axum::{
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use oxigit_core::db;

use crate::AppState;

/// GET /api/badge/{username}.svg
pub async fn founding_badge(
    Path(username_svg): Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    let username = username_svg.strip_suffix(".svg").unwrap_or(&username_svg);
    let pool = state.pool();

    let slot = match db::get_founding_member_slot_by_username(&pool, username).await {
        Ok(Some(slot)) => slot,
        _ => return (StatusCode::NOT_FOUND, "Not a founding member").into_response(),
    };

    let svg = generate_founding_badge_svg(username, slot);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(axum::body::Body::from(svg))
        .unwrap()
}

fn generate_founding_badge_svg(username: &str, slot: i64) -> String {
    let slot_display = format!("#{:03}", slot);
    // Truncate long usernames for display
    let display_name = if username.len() > 18 {
        format!("{}...", &username[..15])
    } else {
        username.to_string()
    };

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="260" viewBox="0 0 200 260" fill="none">
  <defs>
    <linearGradient id="gold" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#fde68a"/>
      <stop offset="50%" stop-color="#f59e0b"/>
      <stop offset="100%" stop-color="#b45309"/>
    </linearGradient>
    <linearGradient id="gold-inner" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#fbbf24"/>
      <stop offset="100%" stop-color="#d97706"/>
    </linearGradient>
    <linearGradient id="dark-bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#1a1a2e"/>
      <stop offset="100%" stop-color="#0f0f1a"/>
    </linearGradient>
    <filter id="glow">
      <feGaussianBlur stdDeviation="2" result="blur"/>
      <feMerge>
        <feMergeNode in="blur"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
    <filter id="shadow">
      <feDropShadow dx="0" dy="2" stdDeviation="3" flood-color="#00000040"/>
    </filter>
  </defs>

  <!-- Shield outer shape -->
  <path d="M100 8 L188 40 L188 130 Q188 195 100 248 Q12 195 12 130 L12 40 Z"
        fill="url(#gold)" filter="url(#shadow)"/>

  <!-- Shield inner dark area -->
  <path d="M100 20 L178 48 L178 128 Q178 188 100 236 Q22 188 22 128 L22 48 Z"
        fill="url(#dark-bg)"/>

  <!-- Inner gold border -->
  <path d="M100 20 L178 48 L178 128 Q178 188 100 236 Q22 188 22 128 L22 48 Z"
        fill="none" stroke="#d97706" stroke-width="1.5" opacity="0.6"/>

  <!-- Decorative top line -->
  <line x1="50" y1="65" x2="150" y2="65" stroke="#f59e0b" stroke-width="0.5" opacity="0.4"/>

  <!-- Oxigit logo (scaled from favicon) -->
  <g transform="translate(100, 90)" filter="url(#glow)">
    <!-- Connecting lines -->
    <line x1="-18" y1="-18" x2="-9" y2="-9" stroke="#e08a4a" stroke-linecap="round" stroke-width="2.5"/>
    <line x1="18" y1="18" x2="9" y2="9" stroke="#e08a4a" stroke-linecap="round" stroke-width="2.5"/>
    <!-- Top-left circle -->
    <circle cx="-22" cy="-22" r="4" fill="#e08a4a"/>
    <!-- Bottom-right circle -->
    <circle cx="22" cy="22" r="4" fill="#e08a4a"/>
    <!-- Center ring -->
    <circle cx="0" cy="0" r="12" fill="#e08a4a"/>
    <circle cx="0" cy="0" r="7" fill="#1a1a2e"/>
  </g>

  <!-- Star accents -->
  <text x="42" y="145" font-family="system-ui, sans-serif" font-size="10" fill="#f59e0b" text-anchor="middle">&#9733;</text>
  <text x="158" y="145" font-family="system-ui, sans-serif" font-size="10" fill="#f59e0b" text-anchor="middle">&#9733;</text>

  <!-- FOUNDING text -->
  <text x="100" y="148" font-family="system-ui, -apple-system, sans-serif" font-size="16" font-weight="700"
        fill="#fde68a" text-anchor="middle" letter-spacing="3">FOUNDING</text>

  <!-- MEMBER text -->
  <text x="100" y="168" font-family="system-ui, -apple-system, sans-serif" font-size="14" font-weight="600"
        fill="#fbbf24" text-anchor="middle" letter-spacing="2">MEMBER</text>

  <!-- Decorative line below text -->
  <line x1="60" y1="178" x2="140" y2="178" stroke="#f59e0b" stroke-width="0.5" opacity="0.4"/>

  <!-- Slot number -->
  <text x="100" y="198" font-family="system-ui, -apple-system, sans-serif" font-size="20" font-weight="700"
        fill="#fde68a" text-anchor="middle" filter="url(#glow)">{slot_display}</text>

  <!-- Username -->
  <text x="100" y="222" font-family="system-ui, -apple-system, sans-serif" font-size="11" font-weight="400"
        fill="#a0a0b0" text-anchor="middle">{display_name}</text>
</svg>"##,
        slot_display = slot_display,
        display_name = display_name,
    )
}
