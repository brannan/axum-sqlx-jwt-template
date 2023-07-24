use sqlx::PgPool;

pub mod article;
pub mod comment;
pub mod listing;
pub mod profile;
pub mod user;

#[derive(Clone)]
pub struct Store {
    pub pool: PgPool,
    pub user: user::UserController,
    pub profile: profile::ProfileController,
    pub comment: comment::CommentController,
    pub article: article::ArticleController,
    pub listing: listing::ListingController,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        let user = user::UserController::new(pool.clone());
        let profile = profile::ProfileController::new(pool.clone());
        let comment = comment::CommentController::new(pool.clone());
        let article = article::ArticleController::new(pool.clone());
        let listing = listing::ListingController::new(pool.clone());
        Self {
            pool,
            user,
            profile,
            comment,
            article,
            listing,
        }
    }
}
