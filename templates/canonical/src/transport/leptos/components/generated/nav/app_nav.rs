// AUTO-GENERATED from storage/blast/state/app.ron @ c196c9b8f01e3089f6efceef8e0b434aa0652e9351ec4f2f7b42fca54207b6f1
//
// Do not edit by hand. Run `blast gen all` after mutating state.

use ::leptos::prelude::*;

#[component]
pub fn AppNav() -> impl IntoView {
    view! {
        <nav class="app-nav">
            <section class="app-nav__section" data-section-key="main">
                <h2 class="app-nav__heading">"Main"</h2>
                <ul class="app-nav__list">
                    <li class="app-nav__item"><a class="app-nav__link" href="/dashboard">"Dashboard"</a></li>
                    <li class="app-nav__item"><a class="app-nav__link" href="/profile">"Profile"</a></li>
                </ul>
            </section>
        </nav>
    }
}
