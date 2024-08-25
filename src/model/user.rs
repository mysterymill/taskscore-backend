
use std::{sync::{Arc, Mutex}, hash::Hash, collections::{HashSet, HashMap}, fmt::Display};

use bcrypt::{DEFAULT_COST};
use bolt_client::bolt_proto::{value::Node, Value};
use rocket::{Request, request::Outcome, http::Status, request::{ FromRequest}};
use rocket_okapi::OpenApiFromRequest;
use schemars::JsonSchema;

use crate::logic::logic::{Logic, ApplicationLogic};

use super::{Task, Score, util::{self, get_string, get_bool, get_u16, try_get_string}, entity::{Entity, FromInput}};

#[derive(serde::Serialize, Clone, JsonSchema, OpenApiFromRequest, Debug)]
pub struct User {
    pub id: Option<u32>,
    pub username: String,
    pub display_name: String,
    pub is_admin: bool,
    pub points: u16,
    
    #[serde(skip_serializing)]
    pub scores: Vec<Arc<Score>>,
    
    #[serde(skip_serializing)]
    pub pwd_hash: Option<String>,
}

impl User {
    pub fn new(id: Option<u32>, username: String, display_name: String, is_admin: bool) -> User {
        User {id, username: username, display_name: display_name, points: 0, scores: vec![], pwd_hash: None, is_admin}
    }

    pub fn get_default_user() -> User {
        User {id: None, display_name: "Guy Incognito".to_owned(), is_admin: true, points: 9, pwd_hash: None, scores: vec![], username: "guyincognito".to_owned()}
    }

    pub fn set_password(&mut self, password: String) {
        self.pwd_hash = Some(bcrypt::hash(password, DEFAULT_COST).unwrap());
    }

    pub fn verify_password(&self, password_to_verify: &Option<String>) -> bool {
        match &self.pwd_hash {
            Some(hash) => match password_to_verify {
                Some(pwd_to_verify) => bcrypt::verify(pwd_to_verify, hash.as_str()).unwrap(),
                None => false,
            }
            None => true
        }
    }
}

#[async_trait]
impl <'a> FromRequest<'a> for User {
    type Error = String;

    async fn from_request(request: &'a Request<'_>) -> Outcome<Self, Self::Error> {
        let username_opt = request.headers().get_one("username");
        let display_name_opt = request.headers().get_one("display_name");
        let password_opt = request.headers().get_one("password");
        match username_opt {
            Some(username) => {
                let mut new_user = User::new(None, username.to_owned(), display_name_opt.unwrap_or(username).to_owned(), false);
                if password_opt.is_some() {
                    new_user.set_password(password_opt.unwrap().to_owned());
                }

                Outcome::Success(new_user)
            },
            None => Outcome::Failure((Status::BadRequest, "Username is required".to_owned()))
        }
    }
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for User {
    //fn assert_receiver_is_total_eq(&self) {}
}

impl Hash for User {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.username.hash(state);
        self.display_name.hash(state);
        self.is_admin.hash(state);
    }
}

impl Entity for User {
    type I = u32;

    fn get_id(&self) -> &Option<u32>{
        &self.id
    }

    fn get_node_type_name() -> &'static str {
        "User"
    }
}

impl Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_entity(f)
    }
}

impl From<Node> for User {
    fn from(value: Node) -> Self {
        let properties = value.properties();
        let id =  Some(value.node_identity() as u32);
        let username =  get_string(properties, "username", "N/A");
        let pwd_hash = try_get_string(properties, "pwd_hash");
        let display_name = get_string(properties, "display_name", "N/A");
        let is_admin = get_bool(properties, "is_admin", false);
        let points = get_u16(properties, "points", 0);

        User{id, username, display_name, is_admin, points, scores: vec![], pwd_hash}
    }
}

impl TryFrom<FromInput> for User {
    type Error = String;
    fn try_from(input: FromInput) -> Result<Self, Self::Error> {
        let mut node_map = input.0;
        let user_node_opt = node_map.remove(User::get_node_type_name());

        if user_node_opt.is_none() {
            return Err(String::from("Unable to create user from db node; no user nodes available"))
        }
        
        let mut user_node_vec = user_node_opt.unwrap();

        if user_node_vec.len() != 1 {
            return Err(format!("Unable to create user from db node; unusual number of user nodes: {}", user_node_vec.len()));
        }

        let user_node = user_node_vec.pop().unwrap();

        let user = User::from(user_node);
        Ok(user)
    }
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Team {
    pub id: Option<u32>,
    pub name: String,
    pub manager: Arc<Mutex<User>>,
    pub members: Vec<Arc<Mutex<User>>>,
}

impl Team {
    pub fn new(id: Option<u32>, name: String, manager: Arc<Mutex<User>>) -> Team {
        let members = vec![manager.clone()];
        Team { id, name, manager, members }
    }
/*
    pub fn add_user(&mut self, new_user: Arc<Mutex<User>>, authority: &User) -> Result<(), String> {
        let new_user_locked = new_user.lock().unwrap();
        if self.contains(&new_user_locked) {
            return Err(format!("User '{}' is already member of group '{}'", new_user_locked.username, self.name));
        }

        if self.manager_id != authority.id.unwrap() && !authority.is_admin {
            return Err(format!("User '{}' is not authorized to add users to group '{}'", authority.username, self.name));
        }

        self.member_ids.insert(new_user_locked.id.unwrap());
        self.members.push(new_user.clone());

        Ok(())
    }*/

    pub fn contains(&self, user: &User) -> bool {
        let result = (&self.members).into_iter().map(|member| member.lock().unwrap())
            .filter(|member| member.get_id().is_some())
            .any(|member| member.get_id().as_ref().unwrap() == &user.id.unwrap());

        result
    }
}

impl Entity for Team {
    type I = u32;

    fn get_id(&self) -> &Option<u32>{
        &self.id
    }

    fn get_node_type_name() -> &'static str {
        "Team"
    }
}

impl Display for Team {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_entity(f)
    }
}

impl From<Node> for Team {
    fn from(value: Node) -> Self {
        !unimplemented!();
    }
}

impl TryFrom<FromInput> for Team {
    type Error = String;
    fn try_from(input: FromInput) -> Result<Self, Self::Error> {
        !unimplemented!();
    }
}

#[async_trait]
impl <'a> FromRequest<'a> for Team {
    type Error = String;

    async fn from_request(request: &'a Request<'_>) -> Outcome<Self, Self::Error> {
        let teamname_opt = request.headers().get_one("teamname");
        let user_id_opt = request.headers().get_one("userid");
        let state = request.rocket().state::<ApplicationLogic>().unwrap();  // Temporarily switched to Neo4JRepository

        if teamname_opt.is_none() {
            return Outcome::Failure((Status::BadRequest, "Team name is required".to_owned()));
        }

        if user_id_opt.is_none() {
            return Outcome::Failure((Status::BadRequest, "UserId is required".to_owned()));
        }

        let user_id = user_id_opt.unwrap();
        let user_id_parsed = str::parse::<u32>(user_id_opt.unwrap());
        if user_id_parsed.is_err() {
            return Outcome::Failure((Status::BadRequest, format!("'{}' is not a valid user id", user_id)));
        }

        let user_id = user_id_parsed.unwrap();
        let manager_opt = state.get_user(user_id).await;

        if manager_opt.is_none() {
            return Outcome::Failure((Status::NotFound, format!("UserId '{}' is unknown", user_id)));
        }

        Outcome::Success(Team::new(None, teamname_opt.unwrap().to_owned(), Arc::new(Mutex::new(manager_opt.unwrap()))))
    }
}
