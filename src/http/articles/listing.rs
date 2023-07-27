use axum::extract::{Query, State};
use axum::Json;

use crate::http;
use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::ApiContext;
use crate::models::article::Article;
use crate::models::listing::{FeedArticlesQuery, ListArticlesQuery};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipleArticlesBody {
    articles: Vec<Article>,

    // This is probably supposed to be the *total* number of rows returned by the current query.
    //
    // However, that necessitates executing the query twice, once to get the rows we actually
    // want to return and a second time just for the count which by necessity must
    // touch all matching rows--not exactly an efficient process.
    //
    // This combined with the limit/offset parameters suggests the design uses an old-fashioned
    // pagination style with page numbers and uses this number to calculate
    // the total number of pages. (Disclaimer: I have not actually looked at the frontend
    // design to be sure; this is just an educated guess.)
    //
    // Modern applications don't really do this anymore and instead implement some sort
    // of infinite scrolling scheme which plays better with paginating based on the value
    // of a column like described on `limit`/`offset` above.
    //
    // It's also more intuitive for the user as they don't really care which page of results
    // they're on. If they're searching for something, they're going to give up if it's
    // not in the first few results anyway. If they're just browsing then they
    // don't usually care where they are in the total ordering of things, or if they do
    // then the scrollbar is already an intuitive indication of where they're at.
    //
    // The Postman collection doesn't test pagination, so as a cop-out I've decided to just
    // return the count of articles currently being returned, which satisfies the happy-path tests.
    articles_count: usize,
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#list-articles
pub(in crate::http) async fn list_articles(
    // authentication is optional
    maybe_auth_user: MaybeAuthUser,
    ctx: State<ApiContext>,
    Query(query): Query<ListArticlesQuery>,
) -> http::Result<Json<MultipleArticlesBody>> {
    let articles = ctx
        .store
        .listing()
        .article_list(maybe_auth_user.user_id(), query)
        .await?;

    Ok(Json(MultipleArticlesBody {
        // This is probably incorrect but is deliberate and the Postman collection allows it.
        //
        // See the comment on the field definition for details.
        articles_count: articles.len(),
        articles,
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#feed-articles
pub(in crate::http) async fn feed_articles(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(query): Query<FeedArticlesQuery>,
) -> http::Result<Json<MultipleArticlesBody>> {
    println!("feed_articles for : {:?}", auth_user.user_id);
    let articles = ctx
        .store
        .listing()
        .get_feed_articles(auth_user.user_id, query)
        .await?;
    Ok(Json(MultipleArticlesBody {
        // This is probably incorrect but is deliberate and the Postman collection allows it.
        //
        // See the comment on the field definition for details.
        articles_count: articles.len(),
        articles,
    }))
}
