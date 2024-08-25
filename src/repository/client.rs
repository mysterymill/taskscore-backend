use std::{env, collections::HashMap, sync::{Arc, Mutex as StdMutex}};
use dotenv::dotenv;
use rocket::{tokio::{net::TcpStream, io::BufStream}, futures::lock::Mutex};

use bolt_client::{Client, bolt_proto::{version::{V4_3, V4_2}, Message, message::{Success, Record}, value::{Node, Relationship}, Value}, Metadata, Params};
use tokio_util::compat::*;

#[cfg(test)]
use mockall::automock;

use crate::model::entity::{Entity, Relation};

use super::repository::DbActionError;

pub type ConnectionError = String;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait DbClient {
    async fn fetch<E: Entity> (&self, statement: String, params: Params) -> Result<Vec<E>, DbActionError>;
    async fn fetch_all<E: Entity> (&self) -> Result<Vec<E>, DbActionError>;
    async fn fetch_single<E: Entity> (&self, statement: String, params: Params) -> Result<Option<E>, DbActionError>;
    async fn fetch_by_id<E: Entity> (&self, id: &E::I) -> Result<Option<E>, DbActionError>;
    async fn create<E: Entity> (&self, statement: String, params: Params) -> Result<E, DbActionError>;
    async fn update<E: Entity> (&self, statement: String, params: Params) -> Result<E, DbActionError>;
    async fn delete<E: Entity> (&self, entity: &E) -> Result<(), DbActionError>;
    async fn create_relationship<S: Entity, T: Entity> (&self, source: Arc<StdMutex<S>>, target: Arc<StdMutex<T>>, name: &String, params_opt: Option<HashMap<String, Value>>) -> Result<Relation<S, T>, DbActionError>;
    async fn fetch_relations_of_node_of_type<S: Entity, T: Entity>(&self, source: Arc<StdMutex<S>>, name: &String) -> Result<Vec<Relation<S, T>>, DbActionError>;
    async fn fetch_relations_of_type<S: Entity, T: Entity>(&self, name: &String) -> Result<Vec<Relation<S, T>>, DbActionError>;
    async fn fetch_single_relation<S: Entity, T: Entity>(&self, source: Arc<StdMutex<S>>, target: Arc<StdMutex<T>>, name: &String) -> Result<Relation<S, T>, DbActionError>;
    async fn delete_relation<S: Entity, T: Entity>(&self, source: &S, target: &T, name: &String) -> Result<(), DbActionError>;
}

pub struct Neo4JClient {
    client: Mutex<Client<Compat<BufStream<TcpStream>>>>,
}

impl Neo4JClient {

    pub async fn connect() -> Result<Neo4JClient, ConnectionError> {
        dotenv().ok();
        let db_addr = env::var("TS_DATABASE_ADDRESS").or(Err("Database address not configured".to_owned()))?;

        info!("Connect to DB triggered; DB address: {}", db_addr);

        let stream = TcpStream::connect(db_addr).await.or(Err("unable to create TCP connection to database".to_owned()))?;
        let stream = BufStream::new(stream).compat();
    
        // Create a new connection to the server and perform a handshake to establish a
        // protocol version. This example demonstrates usage of the v4.3 or v4.2 protocol.
        debug!("Creating connection...");
        let result = Client::new(stream, &[V4_3, V4_2, 0, 0]).await;
        if (result.is_err()) {
            let error = format!("Connecting to database failed; error: {}", result.unwrap_err());
            error!("{}", error);
            return Err(error);
        }

        let mut client = result.unwrap();
         
        // Send a HELLO message with authentication details to the server to initialize
        // the session.
        let response: Message = client.hello(
            Metadata::from_iter(vec![
                ("user_agent", "my-client-name/1.0"),
                ("scheme", "basic"),
                ("principal", &env::var("TS_DATABASE_PRINCIPAL").unwrap()),
                ("credentials", &env::var("TS_DATABASE_PASSWORD").unwrap()),
            ])).await.or(Err("Error sending authentication info to database".to_owned()))?;

        Success::try_from(response).or(Err("DB responded with error on login".to_owned()))?;

        Ok(Neo4JClient { client: Mutex::new(client) })
    }

    async fn discard(client: &mut Client<Compat<BufStream<TcpStream>>>) {
        let discard_result = client.discard(Some(Metadata::from_iter(vec![("n", -1)]))).await;

        if discard_result.is_err() {
            let err_msg = discard_result.unwrap_err();
            println!("{}", err_msg);
        }
    }

    async fn begin(client: &mut Client<Compat<BufStream<TcpStream>>>) -> Result<(), DbActionError> {
        let commit_result = client.begin(None).await;

        if commit_result.is_err() {
            let err_msg = commit_result.unwrap_err();
            println!("{}", err_msg);
            Err(format!("Unable to begin transaction: {}", err_msg))
        } else {
            Ok(())
        }
    }

    async fn commit(client: &mut Client<Compat<BufStream<TcpStream>>>) {
        let commit_result = client.commit().await;

        if commit_result.is_err() {
            let err_msg = commit_result.unwrap_err();
            println!("{}", err_msg);
        }
    }

    async fn rollback(client: &mut Client<Compat<BufStream<TcpStream>>>) {
        let rollback_result = client.rollback().await;

        if rollback_result.is_err() {
            let err_msg = rollback_result.unwrap_err();
            println!("{}", err_msg);
        }
    }

    async fn pull_records(&self, client: &mut Client<Compat<BufStream<TcpStream>>>, metadata: Option<Metadata>) -> Result<Vec<Record>, DbActionError> {
        let pull_result = client.pull(metadata).await;
        if pull_result.is_err() {
            let com_err = pull_result.unwrap_err();
            let err_msg = format!("{}", com_err);
            println!("{}", err_msg);
            return Err(err_msg);
        }

        let (records, _response) = pull_result.unwrap();

        Ok(records)
    }

    async fn pull_entities<E: Entity>(&self, client: &mut Client<Compat<BufStream<TcpStream>>>, metadata: Option<Metadata>) -> Result<Vec<E>, DbActionError> {
        let records_result = self.pull_records(client, metadata).await;
        if records_result.is_err() {
            return Err(records_result.unwrap_err());
        }

        let records = records_result.unwrap();

        if records.len() == 0 {
            return Ok(vec![]);
        }

        let entities = records.into_iter().map(|record| {
            let mut node_map = HashMap::new();
            let mut relationship_map = HashMap::new();
            for field in record.fields() {
                match field {
                    Value::Node(node) => {
                            let name = node.labels()[0].clone();
                            node_map.entry(name).or_insert_with(Vec::new).push(node.clone());
                        },
                    Value::Relationship(relationship) => {
                            
                        let name = relationship.rel_type().to_owned();
                        relationship_map.entry(name).or_insert_with(Vec::new).push(relationship.clone());
                        }
                    _ => (),
                }

            }

            let node_result = E::try_from((node_map, relationship_map))
                .map_err(|err| format!("Unable to create entity object of type {}", E::get_node_type_name()));
            node_result
            
        }).collect::<Result<Vec<E>,_>>(); // Collecting into a result, in case a map fails. See: https://www.reddit.com/r/rust/comments/omsukl/falliable_iterators_why_no_try_map_for_iterator/

        entities
    }

    async fn pull_relations_with_predefined_nodes<S: Entity, T: Entity>(&self, client: &mut Client<Compat<BufStream<TcpStream>>>, metadata: Option<Metadata>, source_node: Arc<StdMutex<S>>, target_node: Arc<StdMutex<T>>) -> Result<Vec<Relation<S, T>>, DbActionError> {
        let records_result = self.pull_records(client, metadata).await;
        if records_result.is_err() {
            return Err(records_result.unwrap_err());
        }

        let records = records_result.unwrap();

        if records.len() == 0 {
            return Ok(vec![]);
        }

        let entities = records.into_iter().map(|record| {
            let relationship_result = Relationship::try_from(record.fields()[0].clone());

            if relationship_result.is_ok() {
                let relationship = relationship_result.unwrap();
                let relation_res = Relation::new(source_node.clone(), target_node.clone(), relationship.rel_type().to_string(), None);
                match relation_res {
                    Ok(r) => Ok(r),
                    Err(e) => Err(e)
                }
            } else {
                Err("Unable to create relation from record".to_owned())
            }
            
        }).collect::<Result<Vec<Relation<S, T>>,_>>();

        entities
    }


    async fn pull_relations<S: Entity, T: Entity>(&self, client: &mut Client<Compat<BufStream<TcpStream>>>, metadata: Option<Metadata>) -> Result<Vec<Relation<S, T>>, DbActionError> {
        let records_result = self.pull_records(client, metadata).await;
        if records_result.is_err() {
            return Err(records_result.unwrap_err());
        }

        let records = records_result.unwrap();

        if records.len() == 0 {
            return Ok(vec![]);
        }

        let mut relationship_records = Vec::new();
        let mut source_entities = HashMap::new();
        let mut target_entities = HashMap::new();

        let entities = records.into_iter().for_each(|record| {
            let value = &record.fields()[0];
            let relationship_result = Relationship::try_from(value.clone());

            if relationship_result.is_ok() {
                relationship_records.push(relationship_result.unwrap());
                return;
            }

            let node_result = Node::try_from(value.clone());

            if node_result.is_ok() {
                let node = &node_result.unwrap();
                
                if node.labels()[0].eq(S::get_node_type_name()) {
                    let source_entity = S::from(node.clone());
                    let source_id: i64 = source_entity.get_id().unwrap().into();
                    source_entities.insert(source_id, Arc::new(StdMutex::new(source_entity)));
                }

                // No else here; a node can be both start and target
                if node.labels()[0].eq(T::get_node_type_name()) {
                    let target_entity = T::from(node.clone());
                    let target_id: i64 = target_entity.get_id().unwrap().into();
                    target_entities.insert(target_id, Arc::new(StdMutex::new(target_entity)));
                }
            }
            
            println!("Selected value is neither a relationship, nor a source or target node");
        });

        let result = relationship_records.into_iter().map(|relationship| {
            let source_entity = source_entities.get(&relationship.start_node_identity()).unwrap().clone(); // unwrap should be sage here (famous last words, I know)
            let target_entity = target_entities.get(&relationship.end_node_identity()).unwrap().clone();
            let relationship_type = relationship.rel_type().to_string();
            let relation_res = Relation::new(source_entity, target_entity, relationship_type, None);
            relation_res
        }).collect::<Result<Vec<Relation<S, T>>,_>>();

        result
    }

    async fn run(&self, statement: String, params_opt: Option<Params>, is_write_action: bool) -> Result<(), DbActionError> {
        let mut client = self.client.lock().await;

        if is_write_action {
            let begin_result = Neo4JClient::begin(&mut client).await;
            if begin_result.is_err() {
                return Err(begin_result.unwrap_err());
            }
        }

        let inter = client.run(statement, params_opt, None);
        let run_result = inter.await;
        
        if run_result.is_err() {
            let com_err = run_result.unwrap_err();
            let err_msg = format!("{}", com_err);
            println!("{}", err_msg);

            if is_write_action {
                Neo4JClient::rollback(&mut client).await;
            }
            
            Err(err_msg)
        } else {

            Ok(())
        }
    }

    async fn perform_action_return_nothing(&self, statement: String, params_opt: Option<Params>, is_write_action: bool) -> Result<(), DbActionError> {
        let run_result = self.run(statement, params_opt, is_write_action).await;
        
        if run_result.is_err() {
            return Err(run_result.unwrap_err());
        }

        let mut client = self.client.lock().await;

        let pull_result = self.pull_records(&mut client, Some(Metadata::from_iter(vec![("n", -1)]))).await;
        
        let result = pull_result.map(|_| ());

        if is_write_action {
            Neo4JClient::commit(&mut client).await;
        }

        result
    }

    async fn perform_action_returning_one_entity<E: Entity>(&self, action_name: &str, statement: String, params_opt: Option<Params>, is_write_action: bool) -> Result<E, DbActionError> {
        let run_result = self.run(statement, params_opt, is_write_action).await;
        
        if run_result.is_err() {
            return Err(run_result.unwrap_err());
        }

        let mut client = self.client.lock().await;
        let metadata = Some(Metadata::from_iter(vec![("n", 1)]));

        // this pull actually reads the new node we just created on the DB. It is not neccessary in order to complete the create
        let pull_result = self.pull_entities::<E>(&mut client, metadata).await;
        
        let result = pull_result.and_then(|mut entity_vec| entity_vec.pop().ok_or(format!("{} did not return entity", action_name)));

        if is_write_action {
            Neo4JClient::commit(&mut client).await;
        }

        result
    }

    async fn perform_action_returning_one_relation<S: Entity, T: Entity>(&self, action_name: &str, statement: String, params_opt: Option<Params>, source_node: Arc<StdMutex<S>>, target_node: Arc<StdMutex<T>>, is_write_action: bool) -> Result<Relation<S, T>, DbActionError> {
        let run_result = self.run(statement, params_opt, is_write_action).await;
        
        if run_result.is_err() {
            return Err(run_result.unwrap_err());
        }

        let mut client = self.client.lock().await;
        let metadata = Some(Metadata::from_iter(vec![("n", 1)]));

        let pull_result = self.pull_relations_with_predefined_nodes::<S, T>(&mut client, metadata, source_node, target_node).await;
        
        let result = pull_result.and_then(|mut entity_vec| entity_vec.pop().ok_or(format!("{} did not return relation", action_name)));

        if is_write_action {
            Neo4JClient::commit(&mut client).await;
        }
        
        result
    }

    async fn perform_action_returning_relations<S: Entity, T: Entity>(&self, action_name: &str, statement: String, params_opt: Option<Params>, is_write_action: bool) -> Result<Vec<Relation<S, T>>, DbActionError> {
        let run_result = self.run(statement, params_opt, is_write_action).await;
        
        if run_result.is_err() {
            return Err(run_result.unwrap_err());
        }

        let mut client = self.client.lock().await;
        let metadata = Some(Metadata::from_iter(vec![("n", -1)]));

        let pull_result = self.pull_relations::<S, T>(&mut client, metadata).await;

        if is_write_action {
            Neo4JClient::commit(&mut client).await;
        }
        
        pull_result
    }
}

#[async_trait]
impl DbClient for Neo4JClient {

    async fn fetch_by_id<E: Entity> (&self, id: &E::I) -> Result<Option<E>, DbActionError> {
        let statement = format!("MATCH (e: {}) WHERE id(e) = $id RETURN e", E::get_node_type_name());
        let params = Params::from_iter(vec![("id", Value::Integer(id.clone().into()))]);

        self.fetch_single(statement, params).await
    }

    async fn fetch_all<E: Entity> (&self) -> Result<Vec<E>, DbActionError> {
        let statement = format!("MATCH (e: {}) RETURN e", E::get_node_type_name());
        let params = Params::from_iter::<Vec<(&str, &str)>>(vec![]);

        self.fetch(statement, params).await
    }

    async fn fetch<E: Entity> (&self, statement: String, params: Params) -> Result<Vec<E>, DbActionError> {
        let run_result = self.run(statement, Some(params), false).await;
        
        if run_result.is_err() {
            return Err(run_result.unwrap_err());
        }

        let mut client = self.client.lock().await;

        let entities = self.pull_entities(&mut client, Some(Metadata::from_iter(vec![("n", -1)]))).await;
        //Neo4JClient::discard(&mut client).await;
        entities
    }

    async fn fetch_single<E: Entity> (&self, statement: String, params: Params) -> Result<Option<E>, DbActionError> {
        let fetch_result = self.fetch::<E>(statement, params).await;
        
        let result = fetch_result.and_then(|mut entity_vec| Ok(entity_vec.pop()));

        result
    }

    async fn create<E: Entity> (&self, statement: String, params: Params) -> Result<E, DbActionError> {
        
        let result = self.perform_action_returning_one_entity("Create", statement, Some(params), true).await;

        result

    }

    async fn update<E: Entity> (&self, statement: String, params: Params) -> Result<E, DbActionError> {
        
        let result = self.perform_action_returning_one_entity("Update", statement, Some(params), true).await;

        result
    }

    async fn delete<E: Entity> (&self, entity: &E) -> Result<(), DbActionError> {
        if entity.get_id().is_none() {
            return Err(format!("Entity {} is unpersisted and cannot be deleted", entity));
        }

        let statement = format!("MATCH (p:{}) WHERE id(p) = $id DETACH DELETE p", E::get_node_type_name());
        let params = Params::from_iter(vec![("id", Value::Integer(entity.get_id().as_ref().unwrap().clone().into()))]);

        let run_result = self.perform_action_return_nothing(statement, Some(params), true).await;
        
        run_result
    }

    async fn create_relationship<S: Entity, T: Entity> (&self, source: Arc<StdMutex<S>>, target: Arc<StdMutex<T>>, name: &String, params_opt: Option<HashMap<String, Value>>) -> Result<Relation<S, T>, DbActionError> {

        let relation_res = Relation::new(source.clone(), target.clone(), name.clone(), params_opt);
        if relation_res.is_err() {
            return Err(relation_res.unwrap_err());
        }

        let relation = relation_res.unwrap();
        let statement = relation.get_create_statement();

        let result = self.perform_action_returning_one_relation("Create relation", statement, None, source, target, true).await;

        result
    }

    async fn fetch_relations_of_node_of_type<S: Entity, T: Entity>(&self, source: Arc<StdMutex<S>>, name: &String) -> Result<Vec<Relation<S, T>>, DbActionError> {
        let src_id: i64 = source.lock().unwrap().get_id().as_ref().unwrap().clone().into();
        let statement = format!("MATCH (s:{})-[r:{}]-(t:{}) WHERE id(s) = $id RETURN s,t,r", S::get_node_type_name(), name, T::get_node_type_name(), );
        let params = Params::from_iter(vec![("id", Value::Integer(src_id))]);

        let result = self.perform_action_returning_relations("Match relations of node of type", statement, Some(params), false).await;

        result
    }

    async fn fetch_relations_of_type<S: Entity, T: Entity>(&self, name: &String) -> Result<Vec<Relation<S, T>>, DbActionError> {
        let statement = format!("MATCH (s:{})-[r:{}]-(t:{}) RETURN s,t,r", S::get_node_type_name(), name, T::get_node_type_name());
        let params = Params::from_iter::<Vec<(&str, &str)>>(vec![]);
        
        let result = self.perform_action_returning_relations("Match relations of type", statement, Some(params), false).await;

        result
    }

    async fn fetch_single_relation<S: Entity, T: Entity>(&self, source: Arc<StdMutex<S>>, target: Arc<StdMutex<T>>, name: &String) -> Result<Relation<S, T>, DbActionError> {
        let statement = format!("MATCH (s:{})-[r:{}]-(t:{}) RETURN s,t,r", S::get_node_type_name(), name, T::get_node_type_name());
        let params = Params::from_iter::<Vec<(&str, &str)>>(vec![]);
        
        let result = self.perform_action_returning_one_relation("Match one relation of type", statement, Some(params), source, target, false).await;

        result
    }

    async fn delete_relation<S: Entity, T: Entity>(&self, source: &S, target: &T, name: &String) -> Result<(), DbActionError> {

        if source.get_id().is_none() {
            return Err(format!("Relationship cannot be deleted; Source entity {} is unpersisted", source));
        }

        if target.get_id().is_none() {
            return Err(format!("Relationship cannot be deleted; Target entity {} is unpersisted", target));
        }

        let src_id: i64 = source.get_id().as_ref().unwrap().clone().into();
        let target_id: i64 = target.get_id().as_ref().unwrap().clone().into();

        let statement = format!("MATCH (s:{})-[r:{}]-(t:{}) WHERE id(s) = {} AND id(t) = {} DELETE r",
            S::get_node_type_name(), name, T::get_node_type_name(), src_id, target_id);

        let params = Params::from_iter::<Vec<(&str, &str)>>(vec![]);

        let run_result = self.perform_action_return_nothing(statement, Some(params), true).await;
        
        run_result
    }
}