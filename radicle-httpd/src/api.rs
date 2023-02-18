pub mod auth;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::Method;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::{Extension, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use tower_http::cors::{self, CorsLayer};

use radicle::cob::issue::Issues;
use radicle::cob::patch::Patches;
use radicle::identity::Id;
use radicle::storage::{ReadRepository, ReadStorage};
use radicle::Profile;

mod axum_extra;
mod error;
mod json;
mod v1;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Identifier for sessions
type SessionId = String;

#[derive(Clone)]
pub struct Context {
    profile: Arc<Profile>,
    sessions: Arc<RwLock<HashMap<SessionId, auth::Session>>>,
}

impl Context {
    pub fn new(profile: Arc<Profile>) -> Self {
        Self {
            profile,
            sessions: Default::default(),
        }
    }

    pub fn project_info(&self, id: Id) -> Result<project::Info, error::Error> {
        let storage = &self.profile.storage;
        let repo = storage.repository(id)?;
        let (_, head) = repo.head()?;
        let doc = repo.identity_of(self.profile.id())?;
        let payload = doc.project()?;
        let delegates = doc.delegates;
        let issues = (Issues::open(self.profile.public_key, &repo)?).count()?;
        let patches = (Patches::open(self.profile.public_key, &repo)?).count()?;

        Ok(project::Info {
            payload,
            delegates,
            head,
            issues,
            patches,
            id,
        })
    }

    #[cfg(test)]
    pub fn profile(&self) -> &Arc<Profile> {
        &self.profile
    }

    #[cfg(test)]
    pub fn sessions(&self) -> &Arc<RwLock<HashMap<SessionId, auth::Session>>> {
        &self.sessions
    }
}

pub fn router(ctx: Context) -> Router {
    let root_router = Router::new()
        .route("/", get(root_handler))
        .layer(Extension(ctx.clone()));

    Router::new()
        .merge(root_router)
        .merge(v1::router(ctx))
        .layer(
            CorsLayer::new()
                .max_age(Duration::from_secs(86400))
                .allow_origin(cors::Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PATCH,
                    Method::PUT,
                    Method::DELETE,
                ])
                .allow_headers([CONTENT_TYPE, AUTHORIZATION]),
        )
}

async fn root_handler(Extension(ctx): Extension<Context>) -> impl IntoResponse {
    let response = json!({
        "message": "Welcome!",
        "service": "radicle-httpd",
        "version": format!("{}-{}", VERSION, env!("GIT_HEAD")),
        "node": { "id": ctx.profile.public_key },
        "path": "/",
        "links": [
            {
                "href": "/v1/projects",
                "rel": "projects",
                "type": "GET"
            },
            {
                "href": "/v1/node",
                "rel": "node",
                "type": "GET"
            },
            {
                "href": "/v1/delegates/:did/projects",
                "rel": "projects",
                "type": "GET"
            },
            {
                "href": "/v1/stats",
                "rel": "stats",
                "type": "GET"
            }
        ]
    });

    Json(response)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PaginationQuery {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

mod project {
    use nonempty::NonEmpty;
    use serde::Serialize;

    use radicle::git::Oid;
    use radicle::identity::project::Project;
    use radicle::identity::Id;
    use radicle::prelude::Did;

    /// Project info.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Info {
        /// Project metadata.
        #[serde(flatten)]
        pub payload: Project,
        pub delegates: NonEmpty<Did>,
        pub head: Oid,
        pub patches: usize,
        pub issues: usize,
        pub id: Id,
    }
}
