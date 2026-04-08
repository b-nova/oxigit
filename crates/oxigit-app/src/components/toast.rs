use leptos::prelude::*;

use super::icons::IconX;

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub variant: ToastVariant,
}

#[derive(Clone, Debug)]
pub enum ToastVariant {
    Success,
    Error,
}

#[derive(Clone)]
pub struct ToastContext {
    toasts: ReadSignal<Vec<Toast>>,
    set_toasts: WriteSignal<Vec<Toast>>,
    next_id: ReadSignal<u64>,
    set_next_id: WriteSignal<u64>,
}

impl ToastContext {
    pub fn push(&self, message: impl Into<String>, variant: ToastVariant) {
        let id = self.next_id.get_untracked();
        self.set_next_id.set(id + 1);
        let toast = Toast {
            id,
            message: message.into(),
            variant,
        };
        self.set_toasts.update(|t| t.push(toast));

        // Auto-dismiss after 3 seconds on client
        #[cfg(target_arch = "wasm32")]
        {
            let set_toasts = self.set_toasts;
            use leptos::wasm_bindgen::JsCast;
            let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                set_toasts.update(|t| t.retain(|toast| toast.id != id));
            });
            let _ = leptos::web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    3000,
                );
        }
    }

    pub fn success(&self, message: impl Into<String>) {
        self.push(message, ToastVariant::Success);
    }

    pub fn error(&self, message: impl Into<String>) {
        self.push(message, ToastVariant::Error);
    }
}

pub fn use_toast() -> ToastContext {
    expect_context::<ToastContext>()
}

#[component]
pub fn ToastProvider(children: Children) -> impl IntoView {
    let (toasts, set_toasts) = signal(Vec::<Toast>::new());
    let (next_id, set_next_id) = signal(0u64);

    let ctx = ToastContext {
        toasts,
        set_toasts,
        next_id,
        set_next_id,
    };

    provide_context(ctx);

    view! {
        {children()}
        <ToastContainer />
    }
}

#[component]
fn ToastContainer() -> impl IntoView {
    let ctx = expect_context::<ToastContext>();

    view! {
        <div class="toast-container">
            <For
                each=move || ctx.toasts.get()
                key=|t| t.id
                let:toast
            >
                {
                    let variant_class = match toast.variant {
                        ToastVariant::Success => "toast toast-success",
                        ToastVariant::Error => "toast toast-error",
                    };
                    let id = toast.id;
                    let set_toasts = ctx.set_toasts;
                    let dismiss = move |_| {
                        set_toasts.update(|t| t.retain(|toast| toast.id != id));
                    };
                    view! {
                        <div class=variant_class>
                            <span class="toast-message">{toast.message.clone()}</span>
                            <button class="toast-dismiss" on:click=dismiss>
                                <IconX />
                            </button>
                        </div>
                    }
                }
            </For>
        </div>
    }
}
