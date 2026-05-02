use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::{AvatarSize, BadgeColor, ButtonKind, PageLayout};
use crate::transport::leptos::components::cells::BadgeCell;
use crate::transport::leptos::components::{AppShell, AuthGuard, AuthGuardMode, AvatarCell, Button, Card, PageShell};
use crate::transport::leptos::signals::session::use_session;

import_crate_style!(style, "src/transport/leptos/pages/profile.module.scss");

#[component]
pub fn ProfilePage() -> impl IntoView {
    let session = use_session();
    let user_id = move || match session.get() {
        Some(s) => s.user_id.to_string(),
        None => "—".to_string(),
    };
    let role = move || match session.get() {
        Some(s) => format!("{:?}", s.role),
        None => "—".to_string(),
    };
    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Bleed>
            <AppShell title="Profile".to_string()>
                <Card>
                    <div class=style::identity>
                        <AvatarCell name="You".to_string() size=AvatarSize::Lg/>
                        <div class=style::identity_meta>
                            <h3 class=style::identity_name>"Your account"</h3>
                            <p class=style::identity_email>"Manage your identity and security settings."</p>
                        </div>
                    </div>
                </Card>

                <Card title=Some("Identity".to_string())>
                    <div class=style::section>
                        <div class=style::row>
                            <span class=style::label>"User ID"</span>
                            <span class=style::value>{user_id}</span>
                        </div>
                        <div class=style::row>
                            <span class=style::label>"Role"</span>
                            <span class=style::value>
                                <BadgeCell text=role() color=BadgeColor::Info/>
                            </span>
                        </div>
                        <div class=style::row>
                            <span class=style::label>"Status"</span>
                            <span class=style::value>
                                <BadgeCell text="Active".to_string() color=BadgeColor::Success/>
                            </span>
                        </div>
                    </div>
                </Card>

                <Card title=Some("Security".to_string())>
                    <p class=style::identity_email>"Reset your password or revoke active sessions."</p>
                    <div class=style::actions>
                        <Button kind=ButtonKind::Secondary>"Change password"</Button>
                        <Button kind=ButtonKind::Danger>"Revoke all sessions"</Button>
                    </div>
                </Card>
            </AppShell>
            </PageShell>
        </AuthGuard>
    }
}
