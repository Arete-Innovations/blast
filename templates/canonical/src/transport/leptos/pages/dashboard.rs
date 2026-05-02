use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::{AlertKind, BadgeColor, Currency, PageLayout, StatusKind};
use crate::transport::leptos::components::cells::{BadgeCell, MoneyCell, RelativeDateCell};
use crate::transport::leptos::components::{Alert, AppShell, AuthGuard, AuthGuardMode, Card, PageShell, StatusDot};

import_crate_style!(style, "src/transport/leptos/pages/dashboard.module.scss");

#[component]
pub fn DashboardPage() -> impl IntoView {
    let now = chrono::Utc::now();
    let two_hr_ago = now - chrono::Duration::hours(2);
    let five_hr_ago = now - chrono::Duration::hours(5);
    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Bleed>
            <AppShell title="Dashboard".to_string()>
                <Alert kind=AlertKind::Info dismissible=true>
                    <strong>"Welcome aboard. "</strong>"This dashboard is a stub — replace its body with whatever your app needs."
                </Alert>
                <div class=style::grid>
                    <Card title=Some("Members".to_string())>
                        <strong class=style::stat>"42"</strong>
                        <span class=style::stat_label>"active in the last 24h"</span>
                    </Card>
                    <Card title=Some("Revenue".to_string())>
                        <strong class=style::stat>
                            <MoneyCell amount=128_400 currency=Currency::Usd/>
                        </strong>
                        <span class=style::stat_label>"this billing cycle"</span>
                    </Card>
                    <Card title=Some("Status".to_string())>
                        <StatusDot kind=StatusKind::Online label="API healthy".to_string()/>
                        <StatusDot kind=StatusKind::Online label="DB connected".to_string()/>
                        <StatusDot kind=StatusKind::Pending label="Background jobs".to_string()/>
                    </Card>
                </div>
                <Card title=Some("Recent activity".to_string())>
                    <ul class=style::feed>
                        <li class=style::feed_item>
                            <BadgeCell text="signup".to_string() color=BadgeColor::Success/>
                            <span class=style::feed_text>"alex@catablast.dev created an account"</span>
                            <span class=style::feed_time><RelativeDateCell value=now/></span>
                        </li>
                        <li class=style::feed_item>
                            <BadgeCell text="invoice".to_string() color=BadgeColor::Info/>
                            <span class=style::feed_text>"Powerplant Inc. paid $1,200.00"</span>
                            <span class=style::feed_time><RelativeDateCell value=two_hr_ago/></span>
                        </li>
                        <li class=style::feed_item>
                            <BadgeCell text="warning".to_string() color=BadgeColor::Warning/>
                            <span class=style::feed_text>"Quota at 78% — consider scaling"</span>
                            <span class=style::feed_time><RelativeDateCell value=five_hr_ago/></span>
                        </li>
                    </ul>
                </Card>
            </AppShell>
            </PageShell>
        </AuthGuard>
    }
}
