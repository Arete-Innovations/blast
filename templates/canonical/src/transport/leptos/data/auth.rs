use crate::meltdown::*;
use crate::structs::auth::SessionContext;
use crate::structs::leptos::{AuthOutput, LoginInput, RegisterInput};

pub async fn do_login(input: LoginInput) -> Result<AuthOutput, MeltDown> {
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
        Ok(AuthOutput { session: out.session })
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::transport::leptos::api_client::post_json("/api/auth/login", &input).await
    }
}

pub async fn do_register(input: RegisterInput) -> Result<AuthOutput, MeltDown> {
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
        Ok(AuthOutput { session: out.session })
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

pub async fn load_session() -> Result<Option<SessionContext>, MeltDown> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use leptos::prelude::expect_context;
        let ctx = expect_context::<crate::Ctx>();
        Ok(ctx.session().cloned())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let resp = crate::transport::leptos::api_client::get_json::<SessionContext, _>("/api/auth/me", &()).await;
        match resp {
            Ok(s) => Ok(Some(s)),
            Err(err) => {
                if matches!(err.melt_type, MeltType::SessionMissing | MeltType::AuthRejected | MeltType::SessionInvalid | MeltType::SessionExpired) {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }
}
