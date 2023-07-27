#![allow(unused)]
use crate::http::{ApiContext, Result};
use crate::models::user::{LoginUser, NewUser, UpdateUser};
use crate::models::StoreTrait;
use anyhow::Context;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash};
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::http::error::{Error, ResultExt};
use crate::http::extractor::AuthUser;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/api/users", post(create_user))
        .route("/api/users/login", post(login_user))
        .route("/api/user", get(get_current_user).put(update_user))
}

/// A wrapper type for all requests/responses from these routes.
#[derive(serde::Serialize, serde::Deserialize)]
struct UserBody<T> {
    user: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UserWithToken {
    email: String,
    token: String,
    username: String,
    bio: String,
    image: Option<String>,
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#registration
async fn create_user(
    ctx: State<ApiContext>,
    Json(req): Json<UserBody<NewUser>>,
) -> Result<Json<UserBody<UserWithToken>>> {
    let user = ctx
        .store
        .user()
        .create_user(req.user)
        .await
        .on_constraint("user_username_key", |_| {
            Error::unprocessable_entity([("username", "username taken")])
        })
        .on_constraint("user_email_key", |_| {
            Error::unprocessable_entity([("email", "email taken")])
        })?;

    Ok(Json(UserBody {
        user: UserWithToken {
            email: user.email,
            token: AuthUser {
                user_id: user.user_id,
            }
            .to_jwt(&ctx.config.hmac_key),
            username: user.username,
            bio: "".to_string(),
            image: None,
        },
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#authentication
async fn login_user(
    ctx: State<ApiContext>,
    Json(req): Json<UserBody<LoginUser>>,
) -> Result<Json<UserBody<UserWithToken>>> {
    println!("login_user handler");
    let user = ctx
        .store
        .user()
        .user_by_email(&req.user.email)
        .await
        .or(Err(Error::NotFound))?;

    #[cfg(test)]
    println!("handler user: {:?}", user);

    verify_password(req.user.password, user.password_hash).await?;

    #[cfg(test)]
    println!("handler user verified");

    Ok(Json(UserBody {
        user: UserWithToken {
            email: user.email,
            token: AuthUser {
                user_id: user.user_id,
            }
            .to_jwt(&ctx.config.hmac_key),
            username: user.username,
            bio: user.bio,
            image: user.image,
        },
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-current-user
async fn get_current_user(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<UserBody<UserWithToken>>> {
    let user = ctx
        .store
        .user()
        .user_by_id(&auth_user.user_id)
        .await
        .or(Err(Error::NotFound))?;

    Ok(Json(UserBody {
        user: UserWithToken {
            email: user.email,
            // The spec doesn't state whether we're supposed to return the same token we were passed,
            // or generate a new one. Generating a new one is easier the way the code is structured.
            //
            // This has the side-effect of automatically refreshing the session if the frontend
            // updates its token based on this response.
            token: auth_user.to_jwt(&ctx.config.hmac_key),
            username: user.username,
            bio: user.bio,
            image: user.image,
        },
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#update-user
// Semantically, because this route allows a partial update it should be `PATCH`, not `PUT`.
// However, we have a spec to follow so `PUT` it is.
async fn update_user(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<UserBody<UpdateUser>>,
) -> Result<Json<UserBody<UserWithToken>>> {
    if req.user == UpdateUser::default() {
        // If there's no fields to update, these two routes are effectively identical.
        return get_current_user(auth_user, ctx).await;
    }

    // WTB `Option::map_async()`
    let password_hash = if let Some(password) = req.user.password.clone() {
        Some(hash_password(password).await?)
    } else {
        None
    };

    let user = ctx
        .store
        .user()
        .update_user(&auth_user.user_id, password_hash, req.user)
        .await
        .on_constraint("user_username_key", |_| {
            Error::unprocessable_entity([("username", "username taken")])
        })
        .on_constraint("user_email_key", |_| {
            Error::unprocessable_entity([("email", "email taken")])
        })?;

    Ok(Json(UserBody {
        user: UserWithToken {
            email: user.email,
            token: auth_user.to_jwt(&ctx.config.hmac_key),
            username: user.username,
            bio: user.bio,
            image: user.image,
        },
    }))
}

async fn hash_password(password: String) -> Result<String> {
    // Argon2 hashing is designed to be computationally intensive,
    // so we need to do this on a blocking thread.
    tokio::task::spawn_blocking(move || -> Result<String> {
        let salt = SaltString::generate(rand::thread_rng());
        Ok(PasswordHash::generate(Argon2::default(), password, &salt)
            .map_err(|e| anyhow::anyhow!("failed to generate password hash: {}", e))?
            .to_string())
    })
    .await
    .context("panic in generating password hash")?
}

async fn verify_password(password: String, password_hash: String) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let hash = PasswordHash::new(&password_hash)
            .map_err(|e| anyhow::anyhow!("invalid password hash: {}", e))?;

        hash.verify_password(&[&Argon2::default()], password)
            .map_err(|e| match e {
                argon2::password_hash::Error::Password => Error::Unauthorized,
                _ => anyhow::anyhow!("failed to verify password hash: {}", e).into(),
            })
    })
    .await
    .context("panic in verifying password hash")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        models::{
            user::{DynUserCtrl, MockUserCtrlTrait, User},
            MockStoreTrait, Store,
        },
    };
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use mockall::predicate::*;
    use rand::rngs::mock;
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn get_sample_user(email: &str) -> User {
        User {
            user_id: Uuid::new_v4(),
            email: email.to_string(),
            username: "example".to_string(),
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$wVs/VXjmiV1vAn/uuOvTgg$MSCmIjGXtDGzyYfHgy8bNaxJ2597QlIxRyNH1Wgqsao".to_string(),
            bio: "".to_string(),
            image: None,
        }
    }

    fn get_mock_user_store() -> MockStoreTrait {
        let mut mock_store = MockStoreTrait::new();
        mock_store.expect_user().returning(|| {
            // We need to instantiate the MockUserCtrl inside the closure
            // to prevent ownership issues. This could be refactored into a
            // separate function
            let mut mock_user_ctrl = MockUserCtrlTrait::new();
            let result = Ok(get_sample_user("example@example.com"));
            mock_user_ctrl
                .expect_user_by_email()
                .with(eq("example@example.com"))
                .return_once(move |_| result);
            Arc::new(mock_user_ctrl)
        });
        mock_store
    }

    /// Run with cargo watch -q -c -w src/ -x 'test --lib login_user -- --nocapture'
    #[tokio::test]
    async fn login_user() {
        let mut mock_store = get_mock_user_store();
        let api_ctx = ApiContext {
            store: Arc::new(mock_store),
            config: Arc::new(Config::default()),
        };

        let app = router().with_state(api_ctx);
        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/api/users/login")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{
                        "user": {
                            "email": "example@example.com",
                            "password": "password123"
                        }
                    }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        // Print the body
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        // println!("response: {}", String::from_utf8_lossy(&body));

        // test if user.token has a jwt
        let user: Value = serde_json::from_slice(&body).unwrap();
        assert!(user["user"]["token"].is_string());
    }
}
