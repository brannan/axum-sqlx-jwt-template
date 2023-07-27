use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::{ApiContext, Result};
use crate::models::article::{Article, CreateArticle, UpdateArticle};

use crate::http::articles::comments::router as comments_router;
use crate::http::articles::listing;

pub fn router() -> Router<ApiContext> {
    // I would prefer `listing` to have its own `router()` method and keep the handler
    // functions private, however that doesn't really work here as we need to list all the
    // verbs under the same path exactly once.
    Router::new()
        .route(
            "/api/articles",
            post(create_article).get(listing::list_articles),
        )
        // `feed_articles` could be private technically, but meh
        .route("/api/articles/feed", get(listing::feed_articles))
        .route(
            "/api/articles/:slug",
            get(get_article).put(update_article).delete(delete_article),
        )
        .route(
            "/api/articles/:slug/favorite",
            post(favorite_article).delete(unfavorite_article),
        )
        // This route isn't technically grouped with articles but it makes sense to include it
        // here since it touches the `article` table.
        .route("/api/tags", get(get_tags))
        .merge(comments_router())
}

#[derive(serde::Deserialize, serde::Serialize)]
// Just trying this out to avoid the tautology of `ArticleBody<Article>`
struct ArticleBody<T = Article> {
    article: T,
}

#[derive(serde::Serialize)]
struct TagsBody {
    tags: Vec<String>,
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#create-article
async fn create_article(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ArticleBody<CreateArticle>>,
) -> Result<Json<ArticleBody>> {
    let article = ctx
        .store
        .article()
        .create_article(auth_user.user_id, req.article)
        .await?;
    Ok(Json(ArticleBody { article }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#update-article
async fn update_article(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path(slug): Path<String>,
    Json(req): Json<ArticleBody<UpdateArticle>>,
) -> Result<Json<ArticleBody>> {
    let article = ctx
        .store
        .article()
        .update_article(auth_user.user_id, &slug, req.article)
        .await?;
    Ok(Json(ArticleBody { article }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#delete-article
async fn delete_article(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path(slug): Path<String>,
) -> Result<()> {
    ctx.store
        .article()
        .delete_article(auth_user.user_id, &slug)
        .await
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-article
async fn get_article(
    // The spec states "no authentication required" but should probably state
    // "authentication optional" because we still need to check if the user is following the author.
    maybe_auth_user: MaybeAuthUser,
    ctx: State<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<ArticleBody>> {
    let article = ctx
        .store
        .article()
        .get_article(maybe_auth_user.user_id(), &slug)
        .await?;
    Ok(Json(ArticleBody { article }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#favorite-article
async fn favorite_article(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<ArticleBody>> {
    let article = ctx
        .store
        .article()
        .favorite_article(auth_user.user_id, &slug)
        .await?;
    Ok(Json(ArticleBody { article }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#unfavorite-article
async fn unfavorite_article(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<ArticleBody>> {
    let article = ctx
        .store
        .article()
        .unfavorite_article(auth_user.user_id, &slug)
        .await?;
    Ok(Json(ArticleBody { article }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-tags
async fn get_tags(ctx: State<ApiContext>) -> Result<Json<TagsBody>> {
    let tags = ctx.store.article().get_tags().await?;
    Ok(Json(TagsBody { tags }))
}

// End handler functions.
