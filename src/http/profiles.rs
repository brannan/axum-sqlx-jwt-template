use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::ApiContext;
use crate::http::Result;
use crate::models::profile::Profile;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

// The `profiles` routes are very similar to the `users` routes, except they allow looking up
// other users' data.

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/api/profiles/:username", get(get_user_profile))
        .route(
            "/api/profiles/:username/follow",
            post(follow_user).delete(unfollow_user),
        )
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/api-response-format#profile
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileBody {
    profile: Profile,
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-profile
async fn get_user_profile(
    // The Realworld spec says authentication is optional, but doesn't specify if it should be
    // validated if the `Authorization` header is present. I've chosen to do so.
    //
    // See the docs for `MaybeAuthUser` for why this isn't just `Option<AuthUser>`.
    maybe_auth_user: MaybeAuthUser,
    ctx: State<ApiContext>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    let profile = ctx
        .store
        .profile
        .get_profile_by_id(maybe_auth_user.user_id(), &username)
        .await?;

    Ok(Json(ProfileBody { profile }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#follow-user
async fn follow_user(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    let profile = ctx
        .store
        .profile
        .create_follow(&auth_user.user_id, &username)
        .await?;

    Ok(Json(ProfileBody { profile }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#unfollow-user
async fn unfollow_user(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    let profile = ctx
        .store
        .profile
        .unfollow(&auth_user.user_id, &username)
        .await?;

    Ok(Json(ProfileBody { profile }))
}
