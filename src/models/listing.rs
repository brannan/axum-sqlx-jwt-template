use crate::http::types::Timestamptz;
use crate::http::Result;
use crate::models::article::{Article, ArticleFromQuery};
use futures::TryStreamExt;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize, Default)]
#[serde(default)]
pub struct ListArticlesQuery {
    // Theoretically we could allow filtering by multiple tags, e.g. `/api/articles?tag=Rust&tag=SQL`
    // But the Realworld spec doesn't mention that so we're not doing it.
    pub tag: Option<String>,
    pub author: Option<String>,
    pub favorited: Option<String>,

    // `limit` and `offset` are not the optimal way to paginate SQL queries, because the query
    // planner essentially has to fetch the whole dataset first and then cull it afterwards.
    //
    // It's a much better idea to paginate using the value of an indexed column.
    // For articles, that could be `created_at`, keeping `limit` and then repeatedly querying
    // for `created_at < oldest_created_at_of_previous_query`.
    //
    // Since the spec doesn't return a JSON array at the top level, you could have a `next`
    // field after `articles` that is the URL that the frontend should fetch to get the next page in
    // the ordering, so the frontend doesn't even need to care what column you're using to paginate.
    //
    // However, this is what the Realworld spec calls for.
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
//
// This is technically a subset of `ListArticlesQuery` so we could do some composition
// but it doesn't really save any lines of code and would make these fields slightly less intuitive
// to access in `list_articles()`.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
pub struct FeedArticlesQuery {
    // See comment on these fields in `ListArticlesQuery` above.
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Clone)]
pub struct ListingController {
    pool: PgPool,
}

impl ListingController {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ListingController {
    pub async fn article_list(
        &self,
        user_id: Option<Uuid>,
        query: ListArticlesQuery,
    ) -> Result<Vec<Article>> {
        let articles: Vec<_> = sqlx::query_as!(
        ArticleFromQuery,
        // language=PostgreSQL
        r#"
            select
                slug,
                title,
                description,
                body,
                tag_list,
                article.created_at "created_at: Timestamptz",
                article.updated_at "updated_at: Timestamptz",
                exists(select 1 from article_favorite where user_id = $1) "favorited!",
                coalesce(
                    -- `count(*)` returns `NULL` if the query returned zero columns
                    -- not exactly a fan of that design choice but whatever
                    (select count(*) from article_favorite fav where fav.article_id = article.article_id),
                    0
                ) "favorites_count!",
                author.username author_username,
                author.bio author_bio,
                author.image author_image,
                exists(select 1 from follow where followed_user_id = author.user_id and following_user_id = $1) "following_author!"
            from article
            inner join "user" author using (user_id)
            -- the current way to do conditional filtering in SQLx
            where (
                -- check if `query.tag` is null or contains the given tag
                -- PostgresSQL doesn't have an "array contains element" operator
                -- so instead we check if the tag_list contains an array of just the given tag
                $2::text is null or tag_list @> array[$2]
            )
              and
            (
                $3::text is null or author.username = $3
            )
              and
            (
                $4::text is null or exists(
                    select 1
                    from "user"
                    inner join article_favorite af using (user_id)
                    where username = $4
                )
            )
            order by article.created_at desc
            limit $5
            offset $6
        "#,
        user_id,
        query.tag,
        query.author,
        query.favorited,
        query.limit.unwrap_or(20),
        query.offset.unwrap_or(0)
    )
    .fetch(&self.pool)
    .map_ok(ArticleFromQuery::into_article)
    .try_collect()
    .await?;
        Ok(articles)
    }

    pub async fn get_feed_articles(
        &self,
        user_id: Uuid,
        query: FeedArticlesQuery,
    ) -> Result<Vec<Article>> {
        let articles: Vec<_> = sqlx::query_as!(
        ArticleFromQuery,
        // As a rule of thumb, you always want the most specific dataset to be your outermost
        // `SELECT` so the query planner does as little extraneous work as possible, and then
        // your joins are just fetching data related to rows you already know you're returning.
        // 
        // In this case, our primary table is the `follow` table so we select from that first
        // and join the `article` and `user` tables from there.
        //
        // The structure is otherwise very similar to other queries returning `Article`s, so you'd
        // think that SQLx should provide some way to deduplicate them. However, I think that
        // would ultimately just make each query harder to understand on its own.
        //
        // language=PostgreSQL
        r#"
            select
                slug,
                title,
                description,
                body,
                tag_list,
                article.created_at "created_at: Timestamptz",
                article.updated_at "updated_at: Timestamptz",
                exists(select 1 from article_favorite where user_id = $1) "favorited!",
                coalesce(
                    (select count(*) from article_favorite fav where fav.article_id = article.article_id),
                    0
                ) "favorites_count!",
                author.username author_username,
                author.bio author_bio,
                author.image author_image,
                -- we wouldn't be returning this otherwise
                true "following_author!"
            from follow
            inner join article on followed_user_id = article.user_id
            inner join "user" author using (user_id)
            where following_user_id = $1
            limit $2
            offset $3
        "#,
        user_id,
        query.limit.unwrap_or(20),
        query.offset.unwrap_or(0)
    )
        .fetch(&self.pool)
        .map_ok(ArticleFromQuery::into_article)
        .try_collect()
        .await?;

        Ok(articles)
    }
}
