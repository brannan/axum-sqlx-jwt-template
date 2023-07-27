use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::ApiContext;
use crate::http::Result;
use crate::models::comment::Comment;
use axum::extract::{Path, State};
use axum::routing::{delete, get};
use axum::{Json, Router};

pub(crate) fn router() -> Router<ApiContext> {
    // Unlike those in `listing`, these routes are fortunately all self-contained
    Router::new()
        .route(
            "/api/articles/:slug/comments",
            get(get_article_comments).post(add_comment),
        )
        .route(
            "/api/articles/:slug/comments/:comment_id",
            delete(delete_comment),
        )
}

#[derive(serde::Deserialize, serde::Serialize)]
struct CommentBody<T = Comment> {
    comment: T,
}

#[derive(serde::Serialize)]
struct MultipleCommentsBody {
    comments: Vec<Comment>,
}

#[derive(serde::Deserialize)]
struct AddComment {
    body: String,
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-comments-from-an-article
async fn get_article_comments(
    maybe_auth_user: MaybeAuthUser,
    ctx: State<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<MultipleCommentsBody>> {
    // With this, we can return 404 if the article slug was not found.
    let comments = ctx
        .store
        .comment()
        .get_article_comments(maybe_auth_user.user_id(), &slug)
        .await?;

    Ok(Json(MultipleCommentsBody { comments }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#add-comments-to-an-article
async fn add_comment(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path(slug): Path<String>,
    req: Json<CommentBody<AddComment>>,
) -> Result<Json<CommentBody>> {
    let comment = ctx
        .store
        .comment()
        .create_comment(auth_user.user_id, &slug, &req.comment.body)
        .await?;
    Ok(Json(CommentBody { comment }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#delete-comment
async fn delete_comment(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Path((slug, comment_id)): Path<(String, i64)>,
) -> Result<()> {
    ctx.store
        .comment()
        .delete_comment(auth_user.user_id, &slug, comment_id)
        .await
}
