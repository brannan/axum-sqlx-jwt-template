use crate::http::types::Timestamptz;
use crate::http::{Error, Result};
use crate::models::profile::Profile;
use futures::TryStreamExt;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: i64,
    pub created_at: Timestamptz,
    pub updated_at: Timestamptz,
    pub body: String,
    pub author: Profile,
}

// Same thing as `ArticleFromQuery`
pub struct CommentFromQuery {
    pub comment_id: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub body: String,
    pub author_username: String,
    pub author_bio: String,
    pub author_image: Option<String>,
    pub following_author: bool,
}

impl CommentFromQuery {
    pub fn into_comment(self) -> Comment {
        Comment {
            id: self.comment_id,
            // doing this conversion in-code does save having to use the type overrides in query
            created_at: Timestamptz(self.created_at),
            updated_at: Timestamptz(self.updated_at),
            body: self.body,
            author: Profile {
                username: self.author_username,
                bio: self.author_bio,
                image: self.author_image,
                following: self.following_author,
            },
        }
    }
}

#[derive(Clone)]
pub struct CommentController {
    pool: PgPool,
}

impl CommentController {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl CommentController {
    pub async fn get_article_comments(
        &self,
        maybe_auth_user: Option<Uuid>,
        slug: &str,
    ) -> Result<Vec<Comment>> {
        let article_id =
            sqlx::query_scalar!("select article_id from article where slug = $1", slug)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(Error::NotFound)?;

        let comments = sqlx::query_as!(
            CommentFromQuery,
            r#"
                select
                    comment_id,
                    comment.created_at,
                    comment.updated_at,
                    comment.body,
                    author.username author_username,
                    author.bio author_bio,
                    author.image author_image,
                    exists(select 1 from follow where followed_user_id = author.user_id and following_user_id = $1) "following_author!"
                from article_comment comment
                inner join "user" author using (user_id)
                where article_id = $2
                order by created_at
            "#,
            maybe_auth_user,
            article_id
        )
        .fetch(&self.pool)
        .map_ok(CommentFromQuery::into_comment)
        .try_collect()
        .await?;

        Ok(comments)
    }

    pub async fn create_comment(&self, user_id: Uuid, slug: &str, body: &str) -> Result<Comment> {
        let comment = sqlx::query_as!(
            CommentFromQuery,
            r#"
                with inserted_comment as (
                    insert into article_comment(article_id, user_id, body)
                    select article_id, $1, $2
                    from article
                    where slug = $3
                    returning comment_id, created_at, updated_at, body
                )
                select
                    comment_id,
                    comment.created_at,
                    comment.updated_at,
                    body,
                    author.username author_username,
                    author.bio author_bio,
                    author.image author_image,
                    false "following_author!"
                from inserted_comment comment
                inner join "user" author on user_id = $1
            "#,
            user_id,
            body,
            slug
        )
        .fetch_optional(&self.pool)
        .await?
        // In this case, we know a comment should have been inserted unless the article slug
        // was not found.
        .ok_or(Error::NotFound)?
        .into_comment();

        Ok(comment)
    }

    pub async fn delete_comment(&self, user_id: Uuid, slug: &str, comment_id: i64) -> Result<()> {
        let result = sqlx::query!(
            r#"
                with deleted_comment as (
                    delete from article_comment
                    where 
                        comment_id = $1
                        and article_id in (select article_id from article where slug = $2)
                        and user_id = $3
                    returning 1 
                )
                select 
                    exists(
                        select 1 from article_comment
                        inner join article using (article_id)
                        where comment_id = $1 and slug = $2
                    ) "existed!",
                    exists(select 1 from deleted_comment) "deleted!"
            "#,
            comment_id,
            slug,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        if result.deleted {
            Ok(())
        } else if result.existed {
            Err(Error::Forbidden)
        } else {
            Err(Error::NotFound)
        }
    }
}
