use leptos::prelude::*;

use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, PageLayout, PageShell};

#[component]
pub fn LoginPage() -> impl IntoView {
    view! {
        <AuthGuard mode=AuthGuardMode::Public>
            <PageShell layout=PageLayout::Cards>
                <h1>"Login"</h1>
                <form>
                    <label>
                        "Email "
                        <input type="email" name="email" required=true/>
                    </label>
                    <label>
                        "Password "
                        <input type="password" name="password" required=true/>
                    </label>
                    <button type="submit">"Sign in"</button>
                </form>
                <p><a href="/register">"Need an account? Register"</a></p>
            </PageShell>
        </AuthGuard>
    }
}
