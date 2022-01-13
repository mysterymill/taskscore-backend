use rocket::http::{Cookie, CookieJar};
use rocket::response::status::NotFound;
use rocket::serde::json::Json;
use rocket::State;
use crate::model::session::LoginRequest;
use crate::repository::repository::Repository;
use crate::model::{Session};

#[post("/session/login")]
pub fn login<'a>(login_request: LoginRequest, repository: &'a State<Repository>, jar: &'a CookieJar<'_>) -> Result<Json<Session>, NotFound<String>> {
    let session_result = repository.login(login_request);
    match session_result {
        Ok(session) => {
            let session_id: &str = session.id.as_str();
            jar.add(Cookie::new("sid", session_id).into_owned());
            Ok(Json(session))
        },
        Err(error) => Err(NotFound(error))
    }
}

#[get("/session")]
pub fn get_current_session<'a>(session: Session) -> Json<Session> {
    Json(session)
}

#[delete("/session/logout")]
pub fn logout<'a>(session: Session, repository: &'a State<Repository>) -> Result<Json<()>, NotFound<String>> {
    match repository.logout(&session.id) {
        Ok(_) => Ok(Json(())),
        Err(msg) => Err(NotFound(msg))
    }
}