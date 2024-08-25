use std::sync::Arc;

use futures::executor::block_on;
use rocket::response::status::NotFound;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;
use crate::logic::logic::{Logic, ApplicationLogic};
use crate::model::{Score, Session};
use crate::repository;

#[openapi(tag = "Score")]
#[post("/score/<task_id>")]
pub async fn score<'a>(session: Session, task_id: u32, repository: &State<ApplicationLogic>) -> Result<Json<u16>, NotFound<String>> {
    let user = session.user;
    let user_id = user.lock().unwrap().id.unwrap();

    match block_on(repository.score(user_id, task_id)) {
        Ok(new_score) => Ok(Json(new_score)),
        Err(msg) => Err(NotFound(msg))
    }
}

#[openapi(tag = "Score")]
#[get("/score/<user_id>")]
pub async fn get_score_of_user<'a>(user_id: u32, repository: &State<ApplicationLogic>) -> Option<Json<Vec<Arc<Score>>>> {
    repository.get_user(user_id).await.and_then(|user| Some(Json(user.scores)))
}

#[openapi(tag = "Score")]
#[get("/score")]
pub async fn get_score_of_current_user<'a>(session: Session, repository: &State<ApplicationLogic>) -> Json<Vec<Arc<Score>>> {
    let user = session.user;
    repository.refresh_user(user.clone()).await;

    Json(user.clone().lock().unwrap().scores.iter().map(|s| s.clone()).collect())
}