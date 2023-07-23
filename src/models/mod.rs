use sqlx::PgPool;

pub mod user;
pub mod profile;

#[derive(Clone)]
pub struct Store {
    pub pool: PgPool,
    pub user: user::UserController,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        let user = user::UserController::new(pool.clone());
        Self { pool, user }
    }
}
