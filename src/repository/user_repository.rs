use std::{sync::{Arc, Mutex}};

use bolt_client::{Params, bolt_proto::Value};

use crate::{model::{User, Task, entity::Relation}};
use crate::model::entity::{Entity};

use super::{client::{Neo4JClient, DbClient}, repository::{ReadRepository, DbActionError, ModifyRepository, WriteRepository, ReadAllRepository}};

pub struct UserRepository {
    client: Arc<Neo4JClient>,
}

impl  UserRepository {
    pub fn new(client: Arc<Neo4JClient>) -> UserRepository {
        UserRepository { client }
    }

    pub async fn find_user_by_username(&self, username: &String) -> Result<Option<crate::model::User>, DbActionError> {
        let statement = format!("MATCH (u:{} {{username: $username}}) RETURN u", User::get_node_type_name());
        let params = Params::from_iter(vec![("username", username.clone())]);
        // storing result for debug reasons
        let result = self.client.fetch_single::<User>(statement, params).await;

        result
    }

    pub async fn fill_user(&self, user: Arc<Mutex<User>>) -> Result<(), DbActionError> {
        let fetch_result = self.client.fetch_relations_of_node_of_type(user.clone(), &"SCORED".to_owned()).await;

        if (fetch_result.is_err()) {
            return Err(fetch_result.unwrap_err());
        }

        let scores_relations: Vec<Relation<User, Task>> = fetch_result.unwrap();
        user.lock().unwrap().scores = scores_relations.into_iter().map(|relation| Arc::new(relation.into())).collect();

        Ok(())
    }
}

#[async_trait]
impl  ReadRepository<User> for UserRepository {
    async fn find_by_id(&self, id: &u32) -> Result<Option<crate::model::User>, DbActionError> {
        let result = self.client.fetch_by_id::<User>(&id).await;

        result
    }
}

#[async_trait]
impl  ModifyRepository<User> for UserRepository {
    async fn update(&self, entity_with_update_values: &User) -> Result<User, DbActionError> {
        if entity_with_update_values.get_id().is_none() {
            return Err(format!("Id of entity {} is unknown; entity cannot be modified", entity_with_update_values));
        }

        let statement = format!("MATCH (u:{}) WHERE id(u) = $id SET u.display_name = '$display_name', u.pwd_hash = '$pwd_hash', u.username = '$username' RETURN u", User::get_node_type_name());
        let params = Params::from_iter(vec![("id", Value::Integer(entity_with_update_values.get_id().as_ref().unwrap().clone().into())), ("display_name", Value::String(entity_with_update_values.display_name.clone())),
            ("pwd_hash", Value::String(entity_with_update_values.pwd_hash.clone().unwrap_or("".to_owned()))), ("username", Value::String(entity_with_update_values.username.clone()))]);
        
        let result = self.client.update::<User>(statement, params).await;

        result
    }
}

#[async_trait]
impl  WriteRepository<User> for UserRepository {
    async fn add(&self, new_entity: &User) -> Result<Arc<User>, DbActionError> {
        let statement = format!("CREATE (u:{} {{username: $username, display_name: $display_name, pwd_hash: $pwd_hash, is_admin: $is_admin }}) RETURN u", User::get_node_type_name());
        let params = Params::from_iter(vec![("username", new_entity.username.clone()), ("display_name", new_entity.display_name.clone()),
            ("pwd_hash", new_entity.pwd_hash.clone().unwrap_or("".to_owned())), ("is_admin", new_entity.is_admin.to_string())]);
        
        let result = self.client.create::<User>(statement, params).await;

        result.map(|u| Arc::new(u))
    }

    async fn delete(&self, entity_to_delete: &User) -> Result<(), DbActionError> {
        let result = self.client.delete::<User>(entity_to_delete).await;

        result
    }
}

#[async_trait]
impl  ReadAllRepository<User> for UserRepository {
    async fn find_all(&self) -> Result<Vec<User>, DbActionError> {
        let result = self.client.fetch_all::<User>().await;

        result
    }
}

