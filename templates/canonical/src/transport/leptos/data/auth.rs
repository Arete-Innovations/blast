use crate::meltdown::*;
use crate::structs::auth::SessionContext;
use crate::structs::leptos::{LoginInput, RegisterInput};

pub async fn do_login(input: LoginInput) -> Result<SessionContext, MeltDown> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use leptos::prelude::expect_context;
        let ctx = expect_context::<crate::Ctx>();
        let out = crate::flows::auth::login::run(
            &ctx,
            crate::flows::auth::login::LoginInput {
                email: input.email,
                password: input.password,
            },
        )
        .await?;
        Ok(out.session)
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::transport::leptos::api_client::post_json("/api/auth/login", &input).await
    }
}

pub async fn do_register(input: RegisterInput) -> Result<SessionContext, MeltDown> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use leptos::prelude::expect_context;
        let ctx = expect_context::<crate::Ctx>();
        let out = crate::flows::auth::register::run(
            &ctx,
            crate::flows::auth::register::RegisterInput {
                email: input.email,
                password: input.password,
            },
        )
        .await?;
        Ok(out.session)
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::transport::leptos::api_client::post_json("/api/auth/register", &input).await
    }
}

pub async fn do_logout() -> Result<(), MeltDown> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use leptos::prelude::expect_context;
        let ctx = expect_context::<crate::Ctx>();
        let session = ctx.require_session()?;
        crate::flows::auth::logout::run(&ctx, session).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::transport::leptos::api_client::post_unit("/api/auth/logout").await
    }
}

