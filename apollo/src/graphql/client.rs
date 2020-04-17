use serde_json::{Value, Map, Error};
use reqwest::blocking::{Client, ClientBuilder};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use reqwest::header::{HeaderMap, HeaderValue};
use std::vec::Vec;
use std::iter::FromIterator;
use serde::de::DeserializeOwned;
use dirs::data_dir;
use crate::graphql::types::*;

pub struct ApolloCloudClient {
    endpoint_url: String,
    client: Client,
}

pub struct GraphqlOperationError {
    message: String,
    user_error: bool,
}

#[derive(Serialize)]
struct GraphqlQuery<'a> {
    query: &'a str,
    variables: Option<&'a String>
}

impl ApolloCloudClient {
    pub fn new(endpoint_url: String, auth_token: String) -> ApolloCloudClient {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-KEY",
                       HeaderValue::from_str(&auth_token).unwrap());
        headers.insert("CONTENT-TYPE",
                       HeaderValue::from_str("application/json").unwrap());

        let client = ClientBuilder::new()
            .default_headers(headers)
            .build().unwrap();

        ApolloCloudClient {
            endpoint_url,
            client,
        }
    }

    fn send_query<T: DeserializeOwned>(&self, query: GraphqlQuery) -> Result<T, Error> {
        let query_body = serde_json::to_string(&query).unwrap();
        let res = match self.client.post(&self.endpoint_url)
            .body(query_body).send() {
            Ok(res) => res,
            Err(e) => panic!(e)
        };

        let text = String::from(res.text().unwrap());
        match serde_json::from_str::<T>(&text) {
            Ok(r) => Ok(r),
            Err(e) => {
                return Err(e);
            }
        }
    }

    fn execute_operation<T: DeserializeOwned, V: Serialize>(&self, operation_string: &str, variables: V) -> Result<T, Error> {
        let vars_string = serde_json::to_string(&variables).unwrap();
        let gql_query = GraphqlQuery { query: operation_string, variables: Some(&vars_string)};
        self.send_query::<T>(gql_query)
    }

    fn execute_operation_no_variables<T: DeserializeOwned>(&self, operation_string: &str) -> Result<T, Error> {
        let gql_query = GraphqlQuery { query: operation_string, variables: None};
        self.send_query::<T>(gql_query)
    }

    pub fn get_org_memberships(&self) -> Result<HashSet<String>, &str> {
        let result = match self.execute_operation_no_variables::<GetOrgMembershipResponse>(
            GET_ORG_MEMBERSHIPS_QUERY) {
            Ok(r) => r,
            Err(e) => {
                println!("Encountered error {}", e);
                return Err("Could not fetch organizations");
            }
        };
        match result.data.unwrap().me {
            Some(me) =>
                Ok(
                    HashSet::from_iter(
                        me.memberships.into_iter().map(
                            |it| it.account.id
                        ).collect::<Vec<String>>())),
            None => Err("Could not authenticate. Please check that your auth token is up-to-date"),
        }
    }

    pub fn create_new_graph(&self, graph_id: String, account_id: String) -> Result<String, GraphqlOperationError> {
        let variables = CreateGraphVariables {
            graphID: graph_id,
            accountID: account_id,
        };
        let result =
            match self.execute_operation::<CreateGraphResponse, CreateGraphVariables>(CREATE_GRAPH_QUERY, variables) {
                Ok(result) => result,
                Err(message) => return Err(GraphqlOperationError { message: message.to_string(), user_error: false })
            };
        if result.errors.is_some() {
            let message = result.errors.unwrap()
                .iter_mut().map(| err| err.message.clone())
                .collect::<Vec<String>>().join("\n");
            return Err(GraphqlOperationError { message, user_error: false });
        }

        let data = match result.data {
            Some(data) => data,
            None => return Err(GraphqlOperationError {
                message: String::from("Got no data????"),
                user_error: false,
            })
        };

        Ok(data.newService.apiKeys[0].token.clone())
    }
}

static GET_ORG_MEMBERSHIPS_QUERY: &'static str = "
query GetOrgMemberships {
  me {
    ...on User {
      memberships {
         account {
           id
         }
      }
    }
  }
}
";

static CREATE_GRAPH_QUERY: &'static str = "
mutation CreateGraph($accountID: ID!, $graphID: ID!) {
  newService(accountId: $accountID, id: $graphID) {
    id
    apiKeys {
      token
    }
  }
}
";