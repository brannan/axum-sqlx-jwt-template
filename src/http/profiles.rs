use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::ApiContext;
use crate::http::Result;
use crate::models::profile::{DynProfileCtrl, Profile};
use axum::{
    extract::{FromRef, Path, State},
    routing::{get, post},
};
use axum::{Json, Router};

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

impl FromRef<ApiContext> for DynProfileCtrl {
    fn from_ref(ctx: &ApiContext) -> DynProfileCtrl {
        ctx.store.profile()
    }
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-profile
async fn get_user_profile(
    // The Realworld spec says authentication is optional, but doesn't specify if it should be
    // validated if the `Authorization` header is present. I've chosen to do so.
    //
    // See the docs for `MaybeAuthUser` for why this isn't just `Option<AuthUser>`.
    maybe_auth_user: MaybeAuthUser,
    profile_controller: State<DynProfileCtrl>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    let profile = profile_controller
        .get_profile_by_id(maybe_auth_user.user_id(), &username)
        .await?;

    Ok(Json(ProfileBody { profile }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#follow-user
async fn follow_user(
    auth_user: AuthUser,
    profile_controller: State<DynProfileCtrl>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    let profile = profile_controller
        .create_follow(&auth_user.user_id, &username)
        .await?;

    Ok(Json(ProfileBody { profile }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#unfollow-user
async fn unfollow_user(
    auth_user: AuthUser,
    profile_controller: State<DynProfileCtrl>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    let profile = profile_controller
        .unfollow(&auth_user.user_id, &username)
        .await?;

    Ok(Json(ProfileBody { profile }))
}

// cargo watch -q -c -w src/ -x 'test --lib profile -- --nocapture'
#[cfg(test)]
mod tests {
    use crate::{
        config::Config,
        models::{profile::MockProfileCtrlTrait, MockStoreTrait},
    };

    use super::*;
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use mockall::predicate::*;
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn get_sample_profile(username: String) -> Profile {
        Profile {
            username,
            bio: "example bio".to_string(),
            image: None,
            following: false,
        }
    }

    fn get_mock_profile_store(uuid: Uuid, username: String) -> MockStoreTrait {
        //let uuid = Uuid::new_v4();
        let username = username.clone();
        let mut mock_store = MockStoreTrait::new();
        mock_store.expect_profile().returning(move || {
            // We need to instantiate the MockProfileCtrl inside the closure
            // to prevent ownership issues. This could be refactored into a
            // separate function
            let mut mock_profile_ctrl = MockProfileCtrlTrait::new();
            let result = Ok(get_sample_profile(username.clone()));
            mock_profile_ctrl
                .expect_get_profile_by_id()
                .with(eq(Some(uuid)), eq(username.clone()))
                .return_once(move |_, _| result);

            Arc::new(mock_profile_ctrl)
        });
        mock_store
    }

    #[tokio::test]
    async fn get_user_profile() {
        let hmac_key = "Yabba Dabba Doo!";
        let username = "fred".to_string();
        let auth_user = AuthUser {
            user_id: Uuid::new_v4(),
        };
        let jwt = auth_user.to_jwt(hmac_key);

        let mock_store = get_mock_profile_store(auth_user.user_id, username.clone());
        let api_context = ApiContext {
            store: Arc::new(mock_store),
            config: Arc::new(Config {
                hmac_key: hmac_key.to_string(),
                ..Default::default()
            }),
        };

        let app: Router = router().with_state(api_context);

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::GET)
                    .uri(format!("/api/profiles/{}", username))
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .header("Authorization", format!("Token {}", jwt))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        println!("response: {}", String::from_utf8_lossy(&body));
        // response: response: {"profile":{"username":"username","bio":"example bio","image":null,"following":false}}

        // check if {profile: {"username"} } is username.
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["profile"]["username"], username);

        assert_eq!(status, StatusCode::OK);
    }
}
