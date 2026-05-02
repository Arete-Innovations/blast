use chrono::{Duration, Utc};
use leptos::prelude::*;
use serde_json::json;
use stylance::import_crate_style;

use crate::structs::leptos::{
    AlertKind, BadgeColor, BoolVariant, BreadcrumbItem, Currency, DateFormat, DrawerSide, FilterDef, PageLayout, RouteName, SkeletonVariant, StatusKind, StepItem, StepStatus, TabItem,
};
use crate::transport::leptos::components::cells::{BadgeCell, BoolCell, DateCell, DurationCell, EmptyCell, EnumCell, JsonCell, MoneyCell, NumberCell, PercentCell, RelativeDateCell, TimeCell};
use crate::transport::leptos::components::{
    Alert, AvatarCell, Breadcrumb, Card, ConfirmDialog, Drawer, EmptyState, FieldError, FilterBar, FormGroup, HelpText, InputGroup, LinkCell, PageShell, Pagination, Skeleton, SortHeader, StatusDot, Stepper, Tabs,
};

import_crate_style!(style, "src/transport/leptos/pages/welcome.module.scss");

#[component]
pub fn WelcomePage() -> impl IntoView {
    view! {
        <PageShell layout=PageLayout::Bleed>
            <div class=style::shell>
                <Sidebar/>
                <main class=style::main>
                    {Hero().into_any()}
                    {TokensSection().into_any()}
                    {ButtonsSection().into_any()}
                    {FormsSection().into_any()}
                    {FeedbackSection().into_any()}
                    {LayoutSection().into_any()}
                    {CellsSection().into_any()}
                    {DialogsSection().into_any()}
                </main>
            </div>
        </PageShell>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    let sections = [
        ("overview", "Overview"),
        ("tokens", "Design tokens"),
        ("buttons", "Buttons"),
        ("forms", "Forms"),
        ("feedback", "Feedback"),
        ("layout", "Layout"),
        ("cells", "Cells"),
        ("dialogs", "Dialogs"),
    ];
    view! {
        <aside class=style::sidebar>
            <div class=style::brand>
                <span class=style::brand_kicker>"Catablast"</span>
                <h1 class=style::brand_title>"UI kit"</h1>
            </div>
            <ul class=style::nav>
                {sections.iter().map(|(id, label)| view! {
                    <li class=style::nav_item>
                        <a class=style::nav_link href={format!("#{}", id)}>{*label}</a>
                    </li>
                }).collect_view()}
            </ul>
        </aside>
    }
}

#[component]
fn Hero() -> impl IntoView {
    view! {
        <section id="overview" class=style::section>
            <div class=style::hero>
                <h2 class=style::hero_title>"A strict Rust UI kit"</h2>
                <p class=style::hero_lede>
                    "Every primitive your app needs — vendored, scoped via stylance, "
                    "tokenised in OKLCH, lint-enforced. Replace this page when you "
                    "ship; the components are yours."
                </p>
                <div class=style::hero_cta>
                    <a class={format!("{} {}", style::btn, style::btn_primary)} href={RouteName::Register.path().to_string()}>"Get started"</a>
                    <a class={format!("{} {}", style::btn, style::btn_ghost)} href={RouteName::Login.path().to_string()}>"Sign in"</a>
                </div>
            </div>
        </section>
    }
}

#[component]
fn TokensSection() -> impl IntoView {
    let colors = [
        ("Background", "--app-color-bg", "bg"),
        ("Surface", "--app-color-surface", "surface"),
        ("Foreground", "--app-color-fg", "fg"),
        ("Muted fg", "--app-color-fg-muted", "fg-muted"),
        ("Brand", "--app-color-brand", "brand"),
        ("Info", "--app-color-info", "info"),
        ("Success", "--app-color-success", "success"),
        ("Warning", "--app-color-warning", "warning"),
        ("Danger", "--app-color-danger", "danger"),
        ("Border", "--app-color-border-subtle", "border"),
    ];
    let scale = [
        ("xs", "--app-space-xs"),
        ("sm", "--app-space-sm"),
        ("md", "--app-space-md"),
        ("lg", "--app-space-lg"),
        ("xl", "--app-space-xl"),
        ("2xl", "--app-space-2xl"),
        ("3xl", "--app-space-3xl"),
    ];
    let typo = [
        ("2xs", "--app-fs-2xs"),
        ("xs", "--app-fs-xs"),
        ("sm", "--app-fs-sm"),
        ("md", "--app-fs-md"),
        ("lg", "--app-fs-lg"),
        ("xl", "--app-fs-xl"),
        ("2xl", "--app-fs-2xl"),
        ("3xl", "--app-fs-3xl"),
    ];
    view! {
        <section id="tokens" class=style::section>
            <header class=style::section_head>
                <h2 class=style::section_title>"Design tokens"</h2>
                <p class=style::section_lede>"OKLCH colors, rem-scaled spacing, fluid typography. Edit "<span class=style::code>"style/tokens.scss"</span>" — every component reflows."</p>
            </header>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Colors"</h3>
                <div class=style::swatch_grid>
                    {colors.iter().map(|(label, var, color_attr)| view! {
                        <div class=style::swatch>
                            <span class=style::swatch_chip data-color=*color_attr></span>
                            <span class=style::swatch_label>{*label}</span>
                            <span class=style::swatch_var>{*var}</span>
                        </div>
                    }).collect_view()}
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Spacing"</h3>
                <div class=style::scale>
                    {scale.iter().map(|(label, var)| view! {
                        <div class=style::scale_row>
                            <span class=style::scale_token>{format!("space-{} ({})", label, var)}</span>
                            <span class=style::scale_bar data-size=*label></span>
                        </div>
                    }).collect_view()}
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Typography"</h3>
                <div class=style::scale>
                    {typo.iter().map(|(label, var)| view! {
                        <div class=style::scale_row>
                            <span class=style::scale_token>{format!("fs-{} ({})", label, var)}</span>
                            <span class=style::scale_text data-fs=*label>"The quick brown fox"</span>
                        </div>
                    }).collect_view()}
                </div>
            </div>
        </section>
    }
}

#[component]
fn ButtonsSection() -> impl IntoView {
    view! {
        <section id="buttons" class=style::section>
            <header class=style::section_head>
                <h2 class=style::section_title>"Buttons"</h2>
                <p class=style::section_lede>"Native "<span class=style::code>"<button>"</span>" elements styled by base.scss. Variants via local classes — extract to your own component when you ship."</p>
            </header>

            <div class=style::demo_panel>
                <div class=style::row>
                    <button type="button" class={format!("{} {}", style::btn, style::btn_primary)}>"Primary"</button>
                    <button type="button" class=style::btn>"Secondary"</button>
                    <button type="button" class={format!("{} {}", style::btn, style::btn_danger)}>"Danger"</button>
                    <button type="button" class={format!("{} {}", style::btn, style::btn_ghost)}>"Ghost"</button>
                    <button type="button" class={format!("{} {}", style::btn, style::btn_primary)} disabled=true>"Disabled"</button>
                </div>
            </div>

            <div class=style::demo_panel>
                <p class=style::note>"Native (no class) — base.scss owns padding, border-radius, font-size scaling for 4K."</p>
                <div class=style::row>
                    <button type="button">"Native button"</button>
                    <input type="text" placeholder="Native input"/>
                </div>
            </div>
        </section>
    }
}

#[component]
fn FormsSection() -> impl IntoView {
    view! {
        <section id="forms" class=style::section>
            <header class=style::section_head>
                <h2 class=style::section_title>"Forms"</h2>
                <p class=style::section_lede>"FormGroup wraps label + control + error/help. InputGroup adds prefixes/suffixes."</p>
            </header>

            <div class=style::grid>
                <Card title=Some("Basic fields".to_string())>
                    <FormGroup label="Email".to_string() for_id="demo_email".to_string()>
                        <input id="demo_email" type="email" placeholder="you@catablast.dev"/>
                        <HelpText>"We'll never spam you."</HelpText>
                    </FormGroup>
                    <FormGroup label="Password".to_string() for_id="demo_pw".to_string() error=Some("Must contain a number".to_string())>
                        <input id="demo_pw" type="password" placeholder="************"/>
                    </FormGroup>
                    <FormGroup label="Bio".to_string() for_id="demo_bio".to_string()>
                        <textarea id="demo_bio" rows="3" placeholder="Tell us about yourself"></textarea>
                    </FormGroup>
                    <FormGroup label="Role".to_string() for_id="demo_role".to_string()>
                        <select id="demo_role">
                            <option>"Admin"</option>
                            <option>"Editor"</option>
                            <option>"Viewer"</option>
                        </select>
                    </FormGroup>
                </Card>

                <Card title=Some("Input group".to_string())>
                    <FormGroup label="Domain".to_string() for_id="demo_domain".to_string()>
                        <InputGroup prefix=Some("https://".to_string()) suffix=Some(".catablast.dev".to_string())>
                            <input id="demo_domain" type="text" placeholder="myproject"/>
                        </InputGroup>
                    </FormGroup>
                    <FormGroup label="Amount".to_string() for_id="demo_amt".to_string()>
                        <InputGroup prefix=Some("$".to_string()) suffix=Some("USD".to_string())>
                            <input id="demo_amt" type="number" value="42"/>
                        </InputGroup>
                    </FormGroup>
                    <FormGroup label="Search".to_string() for_id="demo_search".to_string()>
                        <input id="demo_search" type="search" placeholder="Search anything…"/>
                    </FormGroup>
                </Card>

                <Card title=Some("Standalone helpers".to_string())>
                    <p class=style::note>"FieldError + HelpText — drop them anywhere a form needs feedback."</p>
                    <FieldError message="Field is required.".to_string()/>
                    <HelpText>"Use lowercase and dashes."</HelpText>
                </Card>
            </div>
        </section>
    }
}

#[component]
fn FeedbackSection() -> impl IntoView {
    view! {
        <section id="feedback" class=style::section>
            <header class=style::section_head>
                <h2 class=style::section_title>"Feedback"</h2>
                <p class=style::section_lede>"Inline status, banners, skeletons, empty states."</p>
            </header>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Alerts"</h3>
                <Alert kind=AlertKind::Info dismissible=true>
                    <strong>"Heads up. "</strong>"This release reshuffled how flows declare retry policies."
                </Alert>
                <Alert kind=AlertKind::Success>
                    <strong>"Saved. "</strong>"Your changes are live."
                </Alert>
                <Alert kind=AlertKind::Warning>
                    <strong>"Take care. "</strong>"You're editing a published resource."
                </Alert>
                <Alert kind=AlertKind::Danger>
                    <strong>"Failed. "</strong>"Could not reach the upstream service."
                </Alert>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Status"</h3>
                <div class=style::row>
                    <StatusDot kind=StatusKind::Online label="Online".to_string()/>
                    <StatusDot kind=StatusKind::Pending label="Pending".to_string()/>
                    <StatusDot kind=StatusKind::Offline label="Offline".to_string()/>
                    <StatusDot kind=StatusKind::Error label="Error".to_string()/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Skeletons"</h3>
                <div class=style::demo_panel>
                    <div class=style::row>
                        <Skeleton variant=SkeletonVariant::Avatar/>
                        <Skeleton variant=SkeletonVariant::Button/>
                    </div>
                    <Skeleton variant=SkeletonVariant::Line/>
                    <Skeleton variant=SkeletonVariant::Card/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Empty state"</h3>
                <div class=style::demo_panel>
                    <EmptyState title="No projects yet".to_string() message="When you create one, it'll show up here.".to_string()/>
                </div>
            </div>
        </section>
    }
}

#[component]
fn LayoutSection() -> impl IntoView {
    let breadcrumbs = vec![
        BreadcrumbItem::linked("Home", RouteName::Welcome),
        BreadcrumbItem::linked("Settings", RouteName::Profile),
        BreadcrumbItem::current("UI Kit"),
    ];
    let tabs = vec![
        TabItem::new("overview", "Overview"),
        TabItem::new("activity", "Activity"),
        TabItem::new("members", "Members"),
        TabItem::new("billing", "Billing"),
    ];
    let steps = vec![
        StepItem::new("Account", StepStatus::Done),
        StepItem::new("Profile", StepStatus::Done),
        StepItem::new("Workspace", StepStatus::Active),
        StepItem::new("Invite", StepStatus::Pending),
        StepItem::new("Done", StepStatus::Pending),
    ];
    let filters = vec![
        FilterDef::text("name", "Name").with_placeholder("Search by name"),
        FilterDef::select("role", "Role", vec![
            ("admin".to_string(), "Admin".to_string()),
            ("editor".to_string(), "Editor".to_string()),
            ("viewer".to_string(), "Viewer".to_string()),
        ]),
        FilterDef::bool("active", "Active"),
    ];
    view! {
        <section id="layout" class=style::section>
            <header class=style::section_head>
                <h2 class=style::section_title>"Layout"</h2>
                <p class=style::section_lede>"Cards, breadcrumbs, tabs, steppers, pagination, filter bars."</p>
            </header>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Cards"</h3>
                <div class=style::grid>
                    <Card title=Some("Untitled card".to_string())>
                        <p>"Cards are flexbox columns with token spacing — drop anything inside."</p>
                    </Card>
                    <Card title=Some("Stats".to_string())>
                        <div class=style::row>
                            <strong>"42K"</strong>
                            <span class=style::dim>"users"</span>
                        </div>
                    </Card>
                    <Card>
                        <p>"No-title variant."</p>
                    </Card>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Breadcrumb"</h3>
                <div class=style::demo_panel>
                    <Breadcrumb items=breadcrumbs/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Tabs"</h3>
                <div class=style::demo_panel>
                    <Tabs items=tabs/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Stepper"</h3>
                <div class=style::demo_panel>
                    <Stepper items=steps/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Pagination"</h3>
                <div class=style::demo_panel>
                    <Pagination total_pages=10 current_page=3/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Filter bar"</h3>
                <div class=style::demo_panel>
                    <FilterBar filters=filters/>
                </div>
            </div>
        </section>
    }
}

#[component]
fn CellsSection() -> impl IntoView {
    let now = Utc::now();
    let earlier = now - Duration::hours(3);
    let json_value = json!({
        "id": 42,
        "name": "Catablast",
        "tags": ["rust", "leptos"],
        "active": true
    });
    view! {
        <section id="cells" class=style::section>
            <header class=style::section_head>
                <h2 class=style::section_title>"Cells"</h2>
                <p class=style::section_lede>"Display primitives for table rows / detail views. Date, money, percent, bool, badge, JSON, links, avatars."</p>
            </header>

            <table class=style::cell_table>
                <thead>
                    <tr>
                        <th>"Cell"</th>
                        <th>"Output"</th>
                        <th>"Notes"</th>
                    </tr>
                </thead>
                <tbody>
                    <tr><td>"DateCell"</td><td><DateCell value=now format=DateFormat::Long/></td><td class=style::dim>"chrono::DateTime<Utc>"</td></tr>
                    <tr><td>"DateCell — Short"</td><td><DateCell value=now format=DateFormat::Short/></td><td class=style::dim>"YYYY-MM-DD"</td></tr>
                    <tr><td>"TimeCell"</td><td><TimeCell value=now/></td><td class=style::dim>"HH:MM:SS"</td></tr>
                    <tr><td>"RelativeDateCell"</td><td><RelativeDateCell value=earlier/></td><td class=style::dim>"3 hours ago"</td></tr>
                    <tr><td>"DurationCell"</td><td><DurationCell ms=754_321/></td><td class=style::dim>"humanised ms"</td></tr>
                    <tr><td>"NumberCell"</td><td><NumberCell value=12_345.67 decimals=2/></td><td class=style::dim>"thousands separator"</td></tr>
                    <tr><td>"PercentCell"</td><td><PercentCell value=87.4 decimals=1/></td><td class=style::dim>""</td></tr>
                    <tr><td>"MoneyCell — USD"</td><td><MoneyCell amount=4242 currency=Currency::Usd/></td><td class=style::dim>"i64 minor units"</td></tr>
                    <tr><td>"MoneyCell — EUR"</td><td><MoneyCell amount=199_900 currency=Currency::Eur/></td><td class=style::dim>""</td></tr>
                    <tr><td>"BoolCell — Check"</td><td><BoolCell value=true variant=BoolVariant::Check/></td><td class=style::dim>""</td></tr>
                    <tr><td>"BoolCell — YesNo"</td><td><BoolCell value=false variant=BoolVariant::YesNo/></td><td class=style::dim>""</td></tr>
                    <tr><td>"BoolCell — Badge"</td><td><BoolCell value=true variant=BoolVariant::Badge/></td><td class=style::dim>""</td></tr>
                    <tr><td>"EmptyCell"</td><td><EmptyCell/></td><td class=style::dim>"em-dash placeholder"</td></tr>
                    <tr><td>"JsonCell"</td><td><JsonCell value=json_value collapsed=true/></td><td class=style::dim>"collapsed"</td></tr>
                    <tr><td>"LinkCell"</td><td><LinkCell to=RouteName::Dashboard text="Dashboard".to_string()/></td><td class=style::dim>""</td></tr>
                    <tr><td>"EnumCell"</td><td>
                        <EnumCell value=StatusKind::Online color=StatusKind::variant/>
                        " "
                        <EnumCell value=StatusKind::Pending color=StatusKind::variant/>
                        " "
                        <EnumCell value=StatusKind::Offline color=StatusKind::variant/>
                        " "
                        <EnumCell value=StatusKind::Error color=StatusKind::variant/>
                    </td><td class=style::dim>"generic Display + color fn"</td></tr>
                </tbody>
            </table>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Badges"</h3>
                <div class=style::row>
                    <BadgeCell text="Default".to_string() color=BadgeColor::Default/>
                    <BadgeCell text="Info".to_string() color=BadgeColor::Info/>
                    <BadgeCell text="Success".to_string() color=BadgeColor::Success/>
                    <BadgeCell text="Warning".to_string() color=BadgeColor::Warning/>
                    <BadgeCell text="Danger".to_string() color=BadgeColor::Danger/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Avatars"</h3>
                <div class=style::row>
                    <AvatarCell name="Ada Lovelace".to_string()/>
                    <AvatarCell name="Grace Hopper".to_string()/>
                    <AvatarCell name="Alan Turing".to_string()/>
                </div>
            </div>

            <div class=style::subsection>
                <h3 class=style::subsection_title>"Sort header"</h3>
                <div class=style::demo_panel>
                    <table class=style::cell_table>
                        <thead>
                            <tr>
                                <SortHeader col="name" label="Name"/>
                                <SortHeader col="created" label="Created"/>
                                <SortHeader col="status" label="Status"/>
                            </tr>
                        </thead>
                        <tbody>
                            <tr><td>"Catablast"</td><td><DateCell value=now format=DateFormat::Short/></td><td><BadgeCell text="Active".to_string() color=BadgeColor::Success/></td></tr>
                            <tr><td>"Powerplant"</td><td><DateCell value=earlier format=DateFormat::Short/></td><td><BadgeCell text="Pending".to_string() color=BadgeColor::Warning/></td></tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </section>
    }
}

#[component]
fn DialogsSection() -> impl IntoView {
    let on_confirm = Callback::new(|_| {});
    view! {
        <section id="dialogs" class=style::section>
            <header class=style::section_head>
                <h2 class=style::section_title>"Dialogs"</h2>
                <p class=style::section_lede>"URL-state driven — confirm/drawer mount permanently, body shows when "<span class=style::code>"?dialog=name"</span>" is set."</p>
            </header>

            <div class=style::demo_panel>
                <div class=style::row>
                    <a class={format!("{} {}", style::btn, style::btn_danger)} href="?dialog=demo_confirm">"Open confirm"</a>
                    <a class={format!("{} {}", style::btn, style::btn_primary)} href="?dialog=demo_drawer">"Open drawer"</a>
                </div>
                <p class=style::note>"Try the links — the confirm dialog and drawer mount below this section, hidden until "<span class=style::code>"?dialog=…"</span>" is in the URL."</p>
            </div>

            <ConfirmDialog
                name="demo_confirm"
                title="Delete this thing?".to_string()
                message="This action is permanent. We don't soft-delete in canonical.".to_string()
                confirm_label="Yes, delete".to_string()
                on_confirm=on_confirm
            />

            <Drawer name="demo_drawer" side=DrawerSide::Right title="Drawer demo".to_string()>
                <p>"Drawer body. Slide-in panel for inline detail / settings without a route change."</p>
                <p class=style::note>"Close by removing "<span class=style::code>"?dialog"</span>" from URL or clicking the overlay."</p>
            </Drawer>
        </section>
    }
}
