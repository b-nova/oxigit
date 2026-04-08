use leptos::prelude::*;

use super::icons::{IconClipboardCheck, IconCopy};

#[cfg(target_arch = "wasm32")]
mod js {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = r#"
        export function copy_to_clipboard(text) {
            navigator.clipboard.writeText(text);
        }
    "#)]
    extern "C" {
        pub fn copy_to_clipboard(text: &str);
    }
}

#[component]
pub fn CopyButton(#[prop(into)] text: String) -> impl IntoView {
    let (copied, set_copied) = signal(false);

    let on_click = move |_| {
        let _ = &text;
        #[cfg(target_arch = "wasm32")]
        js::copy_to_clipboard(&text);

        set_copied.set(true);

        #[cfg(target_arch = "wasm32")]
        {
            use leptos::wasm_bindgen::JsCast;
            let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                set_copied.set(false);
            });
            let _ = leptos::web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    2000,
                );
        }
    };

    view! {
        <button class="copy-btn" title="Copy to clipboard" on:click=on_click>
            {move || {
                if copied.get() {
                    view! { <span class="copy-btn-success"><IconClipboardCheck /></span> }.into_any()
                } else {
                    view! { <span><IconCopy /></span> }.into_any()
                }
            }}
        </button>
    }
}
