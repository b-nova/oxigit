use leptos::prelude::*;

use super::icons::{IconMoon, IconSun};

#[cfg(target_arch = "wasm32")]
mod js {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = r#"
        export function get_theme() {
            return document.documentElement.getAttribute('data-theme') || 'dark';
        }
        export function set_theme(theme) {
            document.documentElement.setAttribute('data-theme', theme);
            localStorage.setItem('theme', theme);
            var meta = document.querySelector('meta[name="theme-color"]');
            if (meta) meta.setAttribute('content', theme === 'light' ? '#ffffff' : '#0c0f14');
        }
    "#)]
    extern "C" {
        pub fn get_theme() -> String;
        pub fn set_theme(theme: &str);
    }
}

#[component]
pub fn ThemeToggle() -> impl IntoView {
    let (is_light, set_is_light) = signal(false);

    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        set_is_light.set(js::get_theme() == "light");
    });

    let toggle = move |_| {
        let new_light = !is_light.get_untracked();
        set_is_light.set(new_light);
        #[cfg(target_arch = "wasm32")]
        js::set_theme(if new_light { "light" } else { "dark" });
    };

    view! {
        <button class="navbar-icon" title="Toggle theme" on:click=toggle>
            {move || {
                if is_light.get() {
                    view! { <IconMoon /> }.into_any()
                } else {
                    view! { <IconSun /> }.into_any()
                }
            }}
        </button>
    }
}
