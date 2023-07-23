use crate::http::{Error, Result};
use anyhow::Context;
use argon2::{password_hash::SaltString, Argon2, PasswordHash};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(serde::Deserialize)]
pub struct LoginUser {
    pub email: String,
    pub password: String,
}

#[derive(serde::Deserialize, Default, PartialEq, Eq)]
#[serde(default)] // fill in any missing fields with `..UpdateUser::default()`
pub struct UpdateUser {
    pub email: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub bio: Option<String>,
    pub image: Option<String>,
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct User {
    pub user_id: Uuid,
    pub email: String,
    pub username: String,
    pub bio: String,
    pub image: Option<String>,
    pub password_hash: String,
}

#[derive(Clone)]
pub struct UserController {
    pool: PgPool,
}

impl UserController {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl UserController {
    pub async fn create_user(&self, new_user: NewUser) -> Result<User> {
        let password_hash = hash_password(new_user.password.clone()).await?;

        let user = sqlx::query_as!(
            User,
            r#" INSERT INTO "user" (username, email, password_hash) VALUES ($1, $2, $3)
            RETURNING user_id, email, username, bio, image, password_hash"#,
            new_user.username,
            new_user.email,
            password_hash,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn user_by_email(&self, email: &str) -> Result<User> {
        let user = sqlx::query_as!(
            User,
            r#"
                select user_id, email, username, bio, image, password_hash 
                from "user" where email = $1
            "#,
            email,
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::unprocessable_entity([("email", "does not exist")]))?;

        Ok(user)
    }

    pub async fn user_by_id(&self, user_id: &Uuid) -> Result<User> {
        let user = sqlx::query_as!(
            User,
            r#"
                select user_id, email, username, bio, image, password_hash 
                from "user" where user_id = $1
            "#,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::unprocessable_entity([("user_id", "does not exist")]))?;

        Ok(user)
    }

    pub async fn update_user(
        &self,
        uuid: &Uuid,
        password_hash: Option<String>,
        update_user: UpdateUser,
    ) -> Result<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            update "user"
            set email = coalesce($1, "user".email),
                username = coalesce($2, "user".username),
                password_hash = coalesce($3, "user".password_hash),
                bio = coalesce($4, "user".bio),
                image = coalesce($5, "user".image)
            where user_id = $6
            returning user_id, email, username, bio, image, password_hash
        "#,
            update_user.email,
            update_user.username,
            password_hash,
            update_user.bio,
            update_user.image,
            uuid,
        )
        .fetch_one(&self.pool)
        .await
        .or(Err(Error::unprocessable_entity([(
            "user_id",
            "update failed",
        )])))?;
        // .ok_or(Error::unprocessable_entity([("user_id", "update failed")]))?;

        Ok(user)
    }
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
