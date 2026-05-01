use leptos::prelude::*;

use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, PageLayout, PageShell};

#[component]
pub fn RegisterPage() -> impl IntoView {
    view! {
        <AuthGuard mode=AuthGuardMode::Public>
            <PageShell layout=PageLayout::Cards>
                <h1>"Register"</h1>
                <form>
                    <label>
                        "Email "
                        <input type="email" name="email" required=true/>
                    </label>
                    <label>
                        "Password "
                        <input type="password" name="password" required=true/>
                    </label>
                    <label>
                        "Confirm password "
                        <input type="password" name="confirm" required=true/>
                    </label>
                    <button type="submit">"Create account"</button>
                </form>
                <p><a href="/login">"Already have an account? Login"</a></p>
            </PageShell>
        </AuthGuard>
    }
}
