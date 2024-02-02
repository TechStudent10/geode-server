use reqwest::{header::{HeaderMap, HeaderValue}, Client, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::{types::ipnetwork::Ipv4Network, PgConnection};

use crate::types::{api::ApiError, models::github_login_attempt::GithubLoginAttempt};

#[derive(Debug, Deserialize, Serialize)]
pub struct GithubStartAuth {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: i32,
    interval: i32
}

pub struct GithubClient {
    client_id: String,
    client_secret: String
}

impl GithubClient {
    pub fn new(client_id: String, client_secret: String) -> GithubClient {
        GithubClient {client_id, client_secret}
    }

    pub async fn start_auth(&self, ip: Ipv4Network, pool: &mut PgConnection) -> Result<GithubLoginAttempt, ApiError> {
        #[derive(Serialize)]
        struct GithubStartAuthBody {
            client_id: String
        }
        let found_request = GithubLoginAttempt::get_one_by_ip(sqlx::types::ipnetwork::IpNetwork::V4(ip), &mut *pool).await?;
        if found_request.is_some() {
            return Err(ApiError::BadRequest("Login attempt already running".to_string()));
        }
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        let client = Client::builder()
            .default_headers(headers)
            .build();
        if client.is_err() {
            log::error!("{}", client.err().unwrap());
            return Err(ApiError::InternalError);
        }
        let client = client.unwrap();
        let body = GithubStartAuthBody {client_id: String::from(&self.client_id)};
        let json = match serde_json::to_string(&body) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            },
            Ok(j) => j
        };
        let result = client.execute(
            client.post("https://github.com/login/device/code")
                .basic_auth(&self.client_id, Some(&self.client_secret))
                .body(json)
                .build().or(Err(ApiError::InternalError))?
        ).await;
        if result.is_err() {
            log::error!("{}", result.err().unwrap());
            return Err(ApiError::InternalError);
        }

        let result = result.unwrap();
        if result.status() != StatusCode::OK {
            log::error!("Couldn't connect to GitHub");
            return Err(ApiError::InternalError);
        }
        let body = result.json::<GithubStartAuth>().await.or(Err(ApiError::InternalError))?;
        let uuid = GithubLoginAttempt::create(sqlx::types::ipnetwork::IpNetwork::V4(ip), body.device_code, body.interval, body.expires_in, &mut *pool).await?;

        Ok(GithubLoginAttempt { uuid: uuid.to_string(), interval: body.interval, uri: body.verification_uri, code: body.user_code })
    }


    pub async fn poll_github(&self, device_code: &str) -> Result<serde_json::Value, ApiError> {
        #[derive(Serialize, Debug)]
        struct GithubPollAuthBody {
            client_id: String,
            device_code: String,
            grant_type: String
        }
        let body = GithubPollAuthBody {
            client_id: String::from(&self.client_id),
            device_code: String::from(device_code),
            grant_type: String::from("urn:ietf:params:oauth:grant-type:device_code")
        };
        log::info!("{:?}", body);
        let json = match serde_json::to_string(&body) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            },
            Ok(j) => j
        };
        let client = Client::new();
        let resp = client.post("https://github.com/login/oauth/access_token")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .body(json)
            .send()
            .await;
        if resp.is_err() {
            log::info!("{}", resp.err().unwrap());
            return Err(ApiError::InternalError);
        }
        let resp = resp.unwrap();
        let status = resp.status();
        let body = resp.json::<serde_json::Value>().await.unwrap();

        if status != StatusCode::OK {
            log::info!("{:?}", body);
        }

        Ok(body)
    }
}
