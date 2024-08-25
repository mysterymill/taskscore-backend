use std::{sync::{Arc, Mutex}};
use crate::repository::{client::{Neo4JClient}, user_repository, session_repository::{SessionRepository, self}, task_repository::TaskRepository, team_repository::TeamRepository, relation_repository::RelationRepository};
use crate::repository::repository::*;

#[cfg(test)]
use mockall::automock;
use rocket::http::Status;

use crate::{model::{User, Task, Session, session::LoginRequest, user::Team}, resource::http::responder::MessageResponder, repository::{user_repository::UserRepository}};

#[cfg_attr(test, automock)] // Apply the automock macro only if you are testing
#[async_trait]
pub trait Logic {

    async fn get_user(&self, id: u32) -> Option<User>;
    async fn refresh_user(&self, user: Arc<Mutex<User>>) -> ();
    async fn find_user_by_username(&self, username: &String) -> Option<User>;
    async fn get_all_users(&self) -> Vec<User>;
    async fn get_task(&self, id: u32) -> Option<Task>;
    async fn get_all_tasks(&self) -> Vec<Task>;
    async fn get_session(&self, session_id: &String) -> Option<Session>;
    async fn score(&self, user_id: u32, task_id: u32) -> Result<u16, String>;
    async fn create_and_add_user(&self, username: String, display_name: String, password: String, is_admin: bool) -> Result<Arc<Mutex<User>>, String>;
    async fn add_team(&self, team: Team) -> Option<u32>;
    async fn add_user_to_team(&self, team_name: &String, user_id: u32, manager: User) -> Result<(), String>;
    async fn add_user(&self, session: &Session, user: User) -> MessageResponder<u32>;
    async fn login(&self, login_request: LoginRequest) -> Result<Arc<Session>, String>;
    async fn logout(&self, session_id: &String) -> Result<(), String>;
}

// This struct will become quite big (or at least its implementation) Might have to break it up sooner or later.
pub struct ApplicationLogic {
    user_repo: UserRepository,
    session_repo: SessionRepository,
    task_repo: TaskRepository,
    team_repo: TeamRepository,
    relation_repo: RelationRepository,
}
pub type ApplicationLogicError = String;

impl ApplicationLogic {
    pub async fn new() -> Result<ApplicationLogic, ApplicationLogicError> {
        /*let db_client = Arc::new(Neo4JClient::connect().await?);

        let user_repo = UserRepository::new(db_client.clone());
        let session_repo = SessionRepository::new(db_client.clone());
        let task_repo = TaskRepository::new(db_client.clone());
        let team_repo = TeamRepository::new(db_client.clone());*/

        let user_repo = UserRepository::new(Arc::new(Neo4JClient::connect().await?));
        let session_repo = SessionRepository::new(Arc::new(Neo4JClient::connect().await?));
        let task_repo = TaskRepository::new(Arc::new(Neo4JClient::connect().await?));
        let team_repo = TeamRepository::new(Arc::new(Neo4JClient::connect().await?));

        let relation_repo = RelationRepository::new(Arc::new(Neo4JClient::connect().await?));

        Ok(ApplicationLogic { user_repo, session_repo, task_repo, team_repo, relation_repo })
    }
    
}

#[async_trait]
impl Logic for ApplicationLogic {


    async fn get_user(&self, id: u32) -> Option<User> {
        let user_res = self.user_repo.find_by_id(&id).await;
        user_res.unwrap_or_else(|msg| {error!("{}", msg); None})
    }

    async fn find_user_by_username(&self, username: &String) -> Option<User> {
        let user_res = self.user_repo.find_user_by_username(&username).await;
        user_res.unwrap_or_else(|msg| {error!("{}", msg); None})
    }
    
    async fn get_all_users(&self) -> Vec<User> {
        let user_res = self.user_repo.find_all().await;
        user_res.unwrap_or_else(|msg| {error!("{}", msg); vec![]})  // TODO: Unsure about swallowing the error message
    }
    
    async fn get_task(&self, id: u32) -> Option<Task> {
        let task_result = self.task_repo.find_by_id(&id).await;
        match task_result {
            Ok(task_opt) => task_opt,
            Err(msg) => {
                println!("{}", msg);
                None
            }
        }
    }
    
    async fn get_all_tasks(&self) -> Vec<Task> {
        let task_result = self.task_repo.find_all().await;
        match task_result {
            Ok(res) => res,
            Err(msg) => {
                println!("{}", msg);
                vec![]
            }
        }
    }
    
    async fn get_session(&self, session_id: &String) -> Option<Session> {
        let session_opt_res = self.session_repo.find_session_by_session_id(session_id).await;

        if session_opt_res.is_err() {
            println!("Error getting session id: {}", session_opt_res.unwrap_err());
            None
        } else {
            session_opt_res.unwrap()
        }
    }
    
    async fn score(&self, user_id: u32, task_id: u32) -> Result<u16, String> {
        let user_opt_result = self.user_repo.find_by_id(&user_id).await;
        if user_opt_result.is_err() {
            return Err(user_opt_result.unwrap_err());
        }

        let user_opt = user_opt_result.unwrap();
        if user_opt.is_none() {
            return Err(format!("User with id '{}' not found", user_id));
        }

        let mut user = user_opt.unwrap();

        let task_opt_result = self.task_repo.find_by_id(&user_id).await;
        if task_opt_result.is_err() {
            return Err(task_opt_result.unwrap_err());
        }

        let task_opt = task_opt_result.unwrap();
        if task_opt.is_none() {
            return Err(format!("Task with id '{}' not found", task_id));
        }
        
        let task = task_opt.unwrap();
        let task_points = task.points;

        user.points += task_points;

        let update_result = self.user_repo.update(&user).await;

        let create_relation_result = self.relation_repo.create_relationship(Arc::new(Mutex::new(user)), Arc::new(Mutex::new(task)), &"SCORED".to_owned(), None).await;
        if create_relation_result.is_err() {
            let msg = create_relation_result.unwrap_err();
            println!("Error when user {} scored task {}: {}; Ignoring", user_id, task_id, msg);
        }

        match update_result {
            Ok(updated_user) => Ok(updated_user.points),
            Err(msg) => {
                println!("Error when user {} scored task {}: {}", user_id, task_id, msg);
                Err(msg)
            }
        }

    }

    async fn refresh_user(&self, user: Arc<Mutex<User>>) -> () {
        let result = self.user_repo.fill_user(user.clone()).await;
        if result.is_err() {
            println!("Unable to refresh user: {}", result.unwrap_err());
        }
    }
    
    async fn create_and_add_user(&self, username: String, display_name: String, password: String, is_admin: bool) -> Result<Arc<Mutex<User>>, String> {
        !unimplemented!();
    }
    
    async fn add_team(&self, team: Team) -> Option<u32> {
        !unimplemented!();
    }
    
    async fn add_user_to_team(&self, team_name: &String, user_id: u32, manager: User) -> Result<(), String> {
        !unimplemented!();
    }
    
    async fn add_user(&self, session: &Session, user: User) -> MessageResponder<u32> {
        let session_user_is_admin = session.user.lock().unwrap().is_admin;

        if !session_user_is_admin {
            return MessageResponder::create_with_message(Status::Forbidden, String::from("You must be an admin in order to create a user"));
        }

        let check_for_user_result = self.user_repo.find_user_by_username(&user.username).await;
        if check_for_user_result.is_err() || check_for_user_result.unwrap().is_some() {
            return MessageResponder::create_with_message(Status::Conflict, String::from("Username is not available"));
        }


        let add_user_result = self.user_repo.add(&user).await;

        match add_user_result {
            Ok(user) => match user.id {
                    Some(id) => MessageResponder::create_ok(id),
                    None => MessageResponder::create_with_message(Status::InternalServerError, String::from("For some reason the created user does not have an id")),
                },
            Err(msg) => MessageResponder::create_with_message(Status::InternalServerError, msg),
        }
    }
    
    async fn login(&self, login_request: LoginRequest) -> Result<Arc<Session>, String> {
        let username = login_request.username;
        let password = login_request.password;

        let user_opt_res = self.user_repo.find_user_by_username(&username).await;

        if user_opt_res.is_err() {
            return Err(user_opt_res.unwrap_err());
        }

        let user_opt = user_opt_res.unwrap();
        if user_opt.is_none() {
            return Err(String::from("User is unknown"))
        }

        let user = user_opt.unwrap();
        if !user.verify_password(&password) {
            return Err(String::from("Wrong password"))
        }

        let new_session = Session::new(None, Arc::new(Mutex::new(user)));
        let session_res = self.session_repo.add(&new_session).await;

        session_res
    }
    
    async fn logout(&self, session_id: &String) -> Result<(), String> {
        let session_opt_res = self.session_repo.find_session_by_session_id(session_id).await;

        if session_opt_res.is_err() {
            return Err(session_opt_res.unwrap_err());
        }

        let session_opt = session_opt_res.unwrap();
        if session_opt.is_none() {
            return Ok(());
        }

        let session = session_opt.unwrap();
        let logout_result = self.session_repo.delete(&session).await;

        logout_result
    }
    
}