use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingCard;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvoiceInfo {
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub status: String,
    pub invoice_url: Option<String>,
    pub pdf_url: Option<String>,
}

#[server]
async fn fetch_invoices() -> Result<Vec<InvoiceInfo>, ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{
            require_auth, get_control_pool, get_stripe_config, sfn_err,
        };
        use oxigit_core::{billing, db};

        let user = require_auth().await?;
        let pool = get_control_pool().await?;
        let stripe = get_stripe_config()
            .await?
            .ok_or_else(|| ServerFnError::new("Billing not configured"))?;

        let sub = db::get_subscription(&pool, user.id)
            .await
            .map_err(sfn_err)?;

        let sub = match sub {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        let invoices = billing::list_invoices(&stripe.secret_key, &sub.stripe_customer_id)
            .await
            .map_err(sfn_err)?;

        Ok(invoices
            .into_iter()
            .map(|inv| {
                let amount_f = inv.amount_paid as f64 / 100.0;
                let currency_upper = inv.currency.to_uppercase();
                InvoiceInfo {
                    date: format_timestamp(inv.created),
                    amount: format!("{:.2} {}", amount_f, currency_upper),
                    currency: currency_upper,
                    status: inv.status.unwrap_or_else(|| "unknown".to_string()),
                    invoice_url: inv.hosted_invoice_url,
                    pdf_url: inv.invoice_pdf,
                }
            })
            .collect())
    }
    #[cfg(not(feature = "saas"))]
    {
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[cfg(all(feature = "ssr", feature = "saas"))]
fn format_timestamp(ts: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_secs(ts as u64);
    let datetime: chrono::DateTime<chrono::Utc> = d.into();
    datetime.format("%Y-%m-%d").to_string()
}

#[component]
pub fn BillingDetailsPage() -> impl IntoView {
    let invoices = Resource::new(|| (), |_| fetch_invoices());

    view! {
        <div class="page-header">
            <h1 class="page-title">"Billing"</h1>
        </div>

        <Suspense fallback=|| view! { <LoadingCard /> }>
            {move || Suspend::new(async move {
                match invoices.await {
                    Ok(invoices) if invoices.is_empty() => view! {
                        <div class="card" style="max-width: 600px;">
                            <div class="card-body">
                                <p class="text-secondary">"No billing history yet."</p>
                                <a href="/subscription" class="btn" style="margin-top: var(--space-3);">"View subscription"</a>
                            </div>
                        </div>
                    }.into_any(),
                    Ok(invoices) => view! {
                        <div class="card" style="max-width: 800px;">
                            <div class="card-header">"Invoice History"</div>
                            <table class="table">
                                <thead>
                                    <tr>
                                        <th>"Date"</th>
                                        <th>"Amount"</th>
                                        <th>"Status"</th>
                                        <th></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {invoices.into_iter().map(|inv| {
                                        let status_class = match inv.status.as_str() {
                                            "paid" => "badge-success",
                                            "open" => "badge-warning",
                                            "void" | "uncollectible" => "badge-danger",
                                            _ => "",
                                        };
                                        view! {
                                            <tr>
                                                <td>{inv.date}</td>
                                                <td>{inv.amount}</td>
                                                <td><span class={format!("badge {}", status_class)}>{inv.status}</span></td>
                                                <td style="text-align: right;">
                                                    {inv.pdf_url.map(|url| view! {
                                                        <a href=url target="_blank" class="btn btn-sm">"Download PDF"</a>
                                                    })}
                                                    {inv.invoice_url.map(|url| view! {
                                                        <a href=url target="_blank" class="btn btn-sm" style="margin-left: var(--space-2);">"View"</a>
                                                    })}
                                                </td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        </div>
                    }.into_any(),
                    Err(e) => view! {
                        <ErrorDisplay error=e.to_string() />
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}
