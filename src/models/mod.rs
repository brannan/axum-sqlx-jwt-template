use sqlx::PgPool;
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

pub mod article;
pub mod comment;
pub mod listing;
pub mod profile;
pub mod user;

pub type DynStore = Arc<dyn StoreTrait + Send + Sync>;

#[derive(Clone)]
pub struct Store {
    pub pool: PgPool,
}
#[cfg_attr(test, automock)]
pub trait StoreTrait {
    fn user(&self) -> user::DynUserCtrl;
    fn profile(&self) -> profile::ProfileController;
    fn comment(&self) -> comment::CommentController;
    fn article(&self) -> article::ArticleController;
    fn listing(&self) -> listing::ListingController;
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl StoreTrait for Store {
    fn user(&self) -> user::DynUserCtrl {
        Arc::new(user::UserController::new(self.pool.clone())) as user::DynUserCtrl
    }

    fn profile(&self) -> profile::ProfileController {
        profile::ProfileController::new(self.pool.clone())
    }

    fn comment(&self) -> comment::CommentController {
        comment::CommentController::new(self.pool.clone())
    }

    fn article(&self) -> article::ArticleController {
        article::ArticleController::new(self.pool.clone())
    }

    fn listing(&self) -> listing::ListingController {
        listing::ListingController::new(self.pool.clone())
    }
}
