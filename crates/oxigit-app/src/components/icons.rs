use leptos::prelude::*;

#[component]
pub fn IconFolder() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M2 4.5C2 3.67 2.67 3 3.5 3H6l1.5 2H12.5C13.33 5 14 5.67 14 6.5V11.5C14 12.33 13.33 13 12.5 13H3.5C2.67 13 2 12.33 2 11.5V4.5Z" />
        </svg>
    }
}

#[component]
pub fn IconFile() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 2H4.5C3.67 2 3 2.67 3 3.5V12.5C3 13.33 3.67 14 4.5 14H11.5C12.33 14 13 13.33 13 12.5V6L9 2Z" />
            <polyline points="9 2 9 6 13 6" />
        </svg>
    }
}

#[component]
pub fn IconGear() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="8" r="2.5" />
            <path d="M6.8 1.5h2.4l.3 1.8.9.4 1.6-.9 1.7 1.7-.9 1.6.4.9 1.8.3v2.4l-1.8.3-.4.9.9 1.6-1.7 1.7-1.6-.9-.9.4-.3 1.8H6.8l-.3-1.8-.9-.4-1.6.9-1.7-1.7.9-1.6-.4-.9-1.8-.3V6.8l1.8-.3.4-.9-.9-1.6L4 2.3l1.6.9.9-.4.3-1.7Z" />
        </svg>
    }
}

#[component]
pub fn IconUser() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="5" r="2.5" />
            <path d="M3 14c0-2.76 2.24-5 5-5s5 2.24 5 5" />
        </svg>
    }
}

#[component]
pub fn IconLogout() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M6 14H3.5C2.67 14 2 13.33 2 12.5V3.5C2 2.67 2.67 2 3.5 2H6" />
            <polyline points="10 11 14 8 10 5" />
            <line x1="14" y1="8" x2="6" y2="8" />
        </svg>
    }
}

#[component]
pub fn IconBranch() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="6" y1="3" x2="6" y2="10" />
            <circle cx="6" cy="12" r="2" />
            <circle cx="6" cy="3" r="0.5" fill="currentColor" />
            <circle cx="12" cy="5" r="2" />
            <path d="M10 5C8 5 6 5 6 7" />
        </svg>
    }
}

#[component]
pub fn IconCommit() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="8" r="3" />
            <line x1="1" y1="8" x2="5" y2="8" />
            <line x1="11" y1="8" x2="15" y2="8" />
        </svg>
    }
}

#[component]
pub fn IconPullRequest() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="4" cy="4" r="2" />
            <circle cx="4" cy="12" r="2" />
            <circle cx="12" cy="12" r="2" />
            <line x1="4" y1="6" x2="4" y2="10" />
            <path d="M12 10V7C12 5.9 11.1 5 10 5H8" />
            <polyline points="10 3 8 5 10 7" />
        </svg>
    }
}

#[component]
pub fn IconIssueOpen() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="8" r="6" />
            <circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" />
        </svg>
    }
}

#[component]
pub fn IconIssueClosed() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="8" r="6" />
            <polyline points="5.5 8 7.5 10 10.5 6" />
        </svg>
    }
}

#[component]
pub fn IconSearch() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="7" cy="7" r="4.5" />
            <line x1="10.5" y1="10.5" x2="14" y2="14" />
        </svg>
    }
}

#[component]
pub fn IconPlus() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="8" y1="3" x2="8" y2="13" />
            <line x1="3" y1="8" x2="13" y2="8" />
        </svg>
    }
}

#[component]
pub fn IconFork() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="5" cy="3" r="2" />
            <circle cx="11" cy="3" r="2" />
            <circle cx="8" cy="13" r="2" />
            <line x1="5" y1="5" x2="5" y2="7" />
            <line x1="11" y1="5" x2="11" y2="7" />
            <path d="M5 7C5 9 8 9 8 11" />
            <path d="M11 7C11 9 8 9 8 11" />
        </svg>
    }
}

#[component]
pub fn IconLock() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3.5" y="7" width="9" height="7" rx="1.5" />
            <path d="M5.5 7V5C5.5 3.62 6.62 2.5 8 2.5C9.38 2.5 10.5 3.62 10.5 5V7" />
        </svg>
    }
}

#[component]
pub fn IconRepo() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 1.5h8c.83 0 1.5.67 1.5 1.5v10c0 .83-.67 1.5-1.5 1.5H4c-.83 0-1.5-.67-1.5-1.5V3c0-.83.67-1.5 1.5-1.5Z" />
            <line x1="6" y1="1.5" x2="6" y2="14.5" />
            <line x1="8.5" y1="5" x2="11" y2="5" />
            <line x1="8.5" y1="7.5" x2="11" y2="7.5" />
        </svg>
    }
}

#[component]
pub fn IconTrash() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="2.5 4 13.5 4" />
            <path d="M5.5 4V2.5C5.5 2.22 5.72 2 6 2h4c.28 0 .5.22.5.5V4" />
            <path d="M3.5 4l.77 9.25c.06.7.65 1.25 1.35 1.25h4.76c.7 0 1.29-.55 1.35-1.25L12.5 4" />
        </svg>
    }
}

#[component]
pub fn IconCheck() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 8 6.5 11.5 13 4.5" />
        </svg>
    }
}

#[component]
pub fn IconComment() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M2.5 3C2.5 2.17 3.17 1.5 4 1.5h8c.83 0 1.5.67 1.5 1.5v7c0 .83-.67 1.5-1.5 1.5H6L3 14V11.5H4C3.17 11.5 2.5 10.83 2.5 10V3Z" />
        </svg>
    }
}

#[component]
pub fn IconServer() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="2" width="12" height="5" rx="1" />
            <rect x="2" y="9" width="12" height="5" rx="1" />
            <circle cx="5" cy="4.5" r="0.5" fill="currentColor" stroke="none" />
            <circle cx="5" cy="11.5" r="0.5" fill="currentColor" stroke="none" />
        </svg>
    }
}

#[component]
pub fn IconSun() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="8" r="3" />
            <line x1="8" y1="1" x2="8" y2="3" />
            <line x1="8" y1="13" x2="8" y2="15" />
            <line x1="2.3" y1="2.3" x2="3.7" y2="3.7" />
            <line x1="12.3" y1="12.3" x2="13.7" y2="13.7" />
            <line x1="1" y1="8" x2="3" y2="8" />
            <line x1="13" y1="8" x2="15" y2="8" />
            <line x1="2.3" y1="13.7" x2="3.7" y2="12.3" />
            <line x1="12.3" y1="3.7" x2="13.7" y2="2.3" />
        </svg>
    }
}

#[component]
pub fn IconMoon() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M13.5 8.5a5.5 5.5 0 1 1-7-7 4.5 4.5 0 0 0 7 7Z" />
        </svg>
    }
}

#[component]
pub fn IconRust() -> impl IntoView {
    view! {
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="8" cy="8" r="6" />
            <path d="M6 10V6.5C6 6.22 6.22 6 6.5 6H9C9.55 6 10 6.45 10 7C10 7.55 9.55 8 9 8H7" />
            <line x1="7" y1="8" x2="10" y2="10" />
        </svg>
    }
}
