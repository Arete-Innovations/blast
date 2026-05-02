use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::{AdminUserRow, AvatarSize, BadgeColor, ButtonKind, FilterDef, PageLayout};
use crate::transport::leptos::components::cells::{BadgeCell, RelativeDateCell};
use crate::transport::leptos::components::{AppShell, AuthGuard, AuthGuardMode, AvatarCell, Button, Card, FilterBar, PageShell, Pagination};

import_crate_style!(style, "src/transport/leptos/pages/admin.module.scss");

fn rows() -> Vec<AdminUserRow> {
    vec![
        AdminUserRow { name: "Ada Lovelace", email: "ada@catablast.dev", role: "Admin", role_color: BadgeColor::Danger, status: "Active", status_color: BadgeColor::Success, last_seen_offset_min: 5 },
        AdminUserRow { name: "Grace Hopper", email: "grace@catablast.dev", role: "Editor", role_color: BadgeColor::Info, status: "Active", status_color: BadgeColor::Success, last_seen_offset_min: 22 },
        AdminUserRow { name: "Alan Turing", email: "alan@catablast.dev", role: "Editor", role_color: BadgeColor::Info, status: "Pending", status_color: BadgeColor::Warning, last_seen_offset_min: 180 },
        AdminUserRow { name: "Linus Torvalds", email: "linus@catablast.dev", role: "Viewer", role_color: BadgeColor::Default, status: "Active", status_color: BadgeColor::Success, last_seen_offset_min: 45 },
        AdminUserRow { name: "Margaret Hamilton", email: "margaret@catablast.dev", role: "Admin", role_color: BadgeColor::Danger, status: "Active", status_color: BadgeColor::Success, last_seen_offset_min: 12 },
        AdminUserRow { name: "Dennis Ritchie", email: "dennis@catablast.dev", role: "Viewer", role_color: BadgeColor::Default, status: "Suspended", status_color: BadgeColor::Danger, last_seen_offset_min: 14_400 },
    ]
}

#[component]
pub fn AdminPage() -> impl IntoView {
    let now = chrono::Utc::now();
    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Bleed>
            <AppShell title="Admin".to_string()>
                <div class=style::kpis>
                    <Card>
                        <div class=style::kpi>
                            <span class=style::kpi_label>"Users"</span>
                            <strong class=style::kpi_value>"1,284"</strong>
                            <span class=style::kpi_delta>"+38 this week"</span>
                        </div>
                    </Card>
                    <Card>
                        <div class=style::kpi>
                            <span class=style::kpi_label>"Active sessions"</span>
                            <strong class=style::kpi_value>"217"</strong>
                            <span class=style::kpi_delta>"+4.2%"</span>
                        </div>
                    </Card>
                    <Card>
                        <div class=style::kpi>
                            <span class=style::kpi_label>"Failed logins (24h)"</span>
                            <strong class=style::kpi_value>"7"</strong>
                            <span class=style::kpi_delta data-trend="down">"-62%"</span>
                        </div>
                    </Card>
                    <Card>
                        <div class=style::kpi>
                            <span class=style::kpi_label>"Storage used"</span>
                            <strong class=style::kpi_value>"42.7 GB"</strong>
                            <span class=style::kpi_delta>"+1.1 GB"</span>
                        </div>
                    </Card>
                </div>

                <Card title=Some("Users".to_string())>
                    <div class=style::toolbar>
                        <FilterBar filters=vec![
                            FilterDef::text("name", "Name").with_placeholder("Search users…"),
                            FilterDef::select("role", "Role", vec![
                                ("admin".to_string(), "Admin".to_string()),
                                ("editor".to_string(), "Editor".to_string()),
                                ("viewer".to_string(), "Viewer".to_string()),
                            ]),
                            FilterDef::bool("active", "Active"),
                        ]/>
                        <div class=style::toolbar_actions>
                            <Button kind=ButtonKind::Secondary compact=true>"Export"</Button>
                            <Button kind=ButtonKind::Primary compact=true>"Invite user"</Button>
                        </div>
                    </div>

                    <table class=style::table>
                        <thead>
                            <tr>
                                <th>"User"</th>
                                <th>"Role"</th>
                                <th>"Status"</th>
                                <th>"Last seen"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows().into_iter().map(|r| {
                                let last_seen = now - chrono::Duration::minutes(r.last_seen_offset_min);
                                view! {
                                    <tr>
                                        <td>
                                            <div class=style::user_cell>
                                                <AvatarCell name=r.name.to_string() size=AvatarSize::Sm/>
                                                <div>
                                                    <div class=style::user_name>{r.name}</div>
                                                    <div class=style::user_email>{r.email}</div>
                                                </div>
                                            </div>
                                        </td>
                                        <td><BadgeCell text=r.role.to_string() color=r.role_color/></td>
                                        <td><BadgeCell text=r.status.to_string() color=r.status_color/></td>
                                        <td><RelativeDateCell value=last_seen/></td>
                                        <td>
                                            <div class=style::row_actions>
                                                <Button kind=ButtonKind::Ghost compact=true>"Edit"</Button>
                                                <Button kind=ButtonKind::Danger compact=true>"Revoke"</Button>
                                            </div>
                                        </td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>

                    <Pagination total_pages=14 current_page=1/>
                </Card>
            </AppShell>
            </PageShell>
        </AuthGuard>
    }
}
