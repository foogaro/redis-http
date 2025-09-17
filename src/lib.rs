use redis_module::{redis_module, Context, RedisError, RedisResult, RedisString, RedisValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::runtime::Runtime;
use url::Url;
use warp::Filter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use redis::Client;
use quick_xml::Writer;
use std::io::Cursor;

#[derive(Debug, Serialize, Deserialize)]
struct HttpRequest {
    url: String,
    method: String,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisResponse {
    success: bool,
    result: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HashFieldResponse {
    success: bool,
    value: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HashAllResponse {
    success: bool,
    fields: Option<HashMap<String, String>>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthRequest {
    username: Option<String>,
    password: String,
}

#[derive(Debug, Clone)]
pub enum ResponseFormat {
    Json,
    Xml,
    Text,
}

// Global state for the HTTP server
static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static RUNTIME: Mutex<Option<Runtime>> = Mutex::new(None);
static REDIS_CLIENT: Mutex<Option<Client>> = Mutex::new(None);

/// Detect response format from Accept header
pub fn detect_response_format(accept_header: Option<String>) -> ResponseFormat {
    if let Some(accept) = accept_header {
        let accept_lower = accept.to_lowercase();
        if accept_lower.contains("application/xml") || accept_lower.contains("text/xml") {
            return ResponseFormat::Xml;
        } else if accept_lower.contains("text/plain") {
            return ResponseFormat::Text;
        } else if accept_lower.contains("application/json") {
            return ResponseFormat::Json;
        }
    }
    
    // Default to JSON
    ResponseFormat::Json
}

/// Format RedisResponse as XML
pub fn format_redis_response_xml(response: &RedisResponse) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    
    writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("response"))).unwrap();
    
    writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("success"))).unwrap();
    writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(&response.success.to_string()))).unwrap();
    writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("success"))).unwrap();
    
    if let Some(ref result) = response.result {
        writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("result"))).unwrap();
        writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(result))).unwrap();
        writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("result"))).unwrap();
    }
    
    if let Some(ref error) = response.error {
        writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("error"))).unwrap();
        writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(error))).unwrap();
        writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("error"))).unwrap();
    }
    
    writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("response"))).unwrap();
    
    String::from_utf8(writer.into_inner().into_inner()).unwrap()
}

/// Format RedisResponse as plain text
pub fn format_redis_response_text(response: &RedisResponse) -> String {
    if response.success {
        if let Some(ref result) = response.result {
            result.clone()
        } else {
            "OK".to_string()
        }
    } else {
        if let Some(ref error) = response.error {
            format!("ERROR: {}", error)
        } else {
            "ERROR: Unknown error".to_string()
        }
    }
}

/// Format HashFieldResponse as XML
pub fn format_hash_field_response_xml(response: &HashFieldResponse) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    
    writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("response"))).unwrap();
    
    writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("success"))).unwrap();
    writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(&response.success.to_string()))).unwrap();
    writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("success"))).unwrap();
    
    if let Some(ref value) = response.value {
        writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("value"))).unwrap();
        writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(value))).unwrap();
        writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("value"))).unwrap();
    }
    
    if let Some(ref error) = response.error {
        writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("error"))).unwrap();
        writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(error))).unwrap();
        writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("error"))).unwrap();
    }
    
    writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("response"))).unwrap();
    
    String::from_utf8(writer.into_inner().into_inner()).unwrap()
}

/// Format HashFieldResponse as plain text
pub fn format_hash_field_response_text(response: &HashFieldResponse) -> String {
    if response.success {
        if let Some(ref value) = response.value {
            value.clone()
        } else {
            "OK".to_string()
        }
    } else {
        if let Some(ref error) = response.error {
            format!("ERROR: {}", error)
        } else {
            "ERROR: Unknown error".to_string()
        }
    }
}

/// Format HashAllResponse as XML
pub fn format_hash_all_response_xml(response: &HashAllResponse) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    
    writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("response"))).unwrap();
    
    writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("success"))).unwrap();
    writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(&response.success.to_string()))).unwrap();
    writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("success"))).unwrap();
    
    if let Some(ref fields) = response.fields {
        writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("fields"))).unwrap();
        for (key, value) in fields {
            writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("field"))).unwrap();
            writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("key"))).unwrap();
            writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(key))).unwrap();
            writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("key"))).unwrap();
            writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("value"))).unwrap();
            writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(value))).unwrap();
            writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("value"))).unwrap();
            writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("field"))).unwrap();
        }
        writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("fields"))).unwrap();
    }
    
    if let Some(ref error) = response.error {
        writer.write_event(quick_xml::events::Event::Start(quick_xml::events::BytesStart::new("error"))).unwrap();
        writer.write_event(quick_xml::events::Event::Text(quick_xml::events::BytesText::new(error))).unwrap();
        writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("error"))).unwrap();
    }
    
    writer.write_event(quick_xml::events::Event::End(quick_xml::events::BytesEnd::new("response"))).unwrap();
    
    String::from_utf8(writer.into_inner().into_inner()).unwrap()
}

/// Format HashAllResponse as plain text
pub fn format_hash_all_response_text(response: &HashAllResponse) -> String {
    if response.success {
        if let Some(ref fields) = response.fields {
            if fields.is_empty() {
                "OK".to_string()
            } else {
                let mut result = String::new();
                for (key, value) in fields {
                    result.push_str(&format!("{}: {}\n", key, value));
                }
                result.trim_end().to_string()
            }
        } else {
            "OK".to_string()
        }
    } else {
        if let Some(ref error) = response.error {
            format!("ERROR: {}", error)
        } else {
            "ERROR: Unknown error".to_string()
        }
    }
}

/// Validate credentials against Redis instance
async fn validate_redis_credentials(username: Option<&str>, password: &str) -> Result<bool, String> {
    let redis_client = {
        let client_guard = REDIS_CLIENT.lock().unwrap();
        client_guard.clone()
    };
    
    if let Some(_client) = redis_client {
        // Try to connect with the provided credentials
        let connection_result = if let Some(user) = username {
            // Use username and password - create connection string
            let conn_str = format!("redis://{}:{}@127.0.0.1:6379/0", user, password);
            Client::open(conn_str).and_then(|c| c.get_connection())
        } else {
            // Use password only - create connection string
            let conn_str = format!("redis://:{}@127.0.0.1:6379/0", password);
            Client::open(conn_str).and_then(|c| c.get_connection())
        };
        
        match connection_result {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("NOAUTH") || e.to_string().contains("WRONGPASS") {
                    Ok(false)
                } else {
                    Err(format!("Redis connection error: {}", e))
                }
            }
        }
    } else {
        Err("Redis client not initialized".to_string())
    }
}

/// Authentication middleware that validates against Redis
fn auth_middleware() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::header::optional::<String>("authorization")
        .and_then(|auth_header: Option<String>| async move {
            if let Some(header) = auth_header {
                if header.starts_with("Basic ") {
                    // Decode Basic auth
                    let encoded = &header[6..];
                    if let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded) {
                        if let Ok(credentials) = String::from_utf8(decoded) {
                            if let Some((username, password)) = credentials.split_once(':') {
                                match validate_redis_credentials(Some(username), password).await {
                                    Ok(true) => return Ok(()),
                                    Ok(false) => return Err(warp::reject::custom(AuthError)),
                                    Err(_) => return Err(warp::reject::custom(AuthError)),
                                }
                            }
                        }
                    }
                }
            }
            Err(warp::reject::custom(AuthError))
        })
        .untuple_one()
}

#[derive(Debug)]
struct AuthError;

impl warp::reject::Reject for AuthError {}


/// Execute Redis GET command
async fn redis_get(key: String, accept_header: Option<String>) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    let redis_client = {
        let client_guard = REDIS_CLIENT.lock().unwrap();
        client_guard.clone()
    };
    
    if let Some(client) = redis_client {
        match client.get_connection() {
            Ok(mut conn) => {
                match redis::cmd("GET").arg(&key).query::<Option<String>>(&mut conn) {
                    Ok(value) => {
                        let response = RedisResponse {
                            success: true,
                            result: value,
                            error: None,
                        };
                        
                        let format = detect_response_format(accept_header);
                        match format {
                            ResponseFormat::Json => Ok(Box::new(warp::reply::json(&response))),
                            ResponseFormat::Xml => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_redis_response_xml(&response)),
                                "content-type",
                                "application/xml"
                            ))),
                            ResponseFormat::Text => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_redis_response_text(&response)),
                                "content-type",
                                "text/plain"
                            ))),
                        }
                    }
                    Err(e) => {
                        let response = RedisResponse {
                            success: false,
                            result: None,
                            error: Some(format!("Redis error: {}", e)),
                        };
                        
                        let format = detect_response_format(accept_header);
                        match format {
                            ResponseFormat::Json => Ok(Box::new(warp::reply::json(&response))),
                            ResponseFormat::Xml => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_redis_response_xml(&response)),
                                "content-type",
                                "application/xml"
                            ))),
                            ResponseFormat::Text => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_redis_response_text(&response)),
                                "content-type",
                                "text/plain"
                            ))),
                        }
                    }
                }
            }
            Err(e) => {
                let response = RedisResponse {
                    success: false,
                    result: None,
                    error: Some(format!("Connection error: {}", e)),
                };
                Ok(Box::new(warp::reply::json(&response)))
            }
        }
    } else {
        let response = RedisResponse {
            success: false,
            result: None,
            error: Some("Redis client not initialized".to_string()),
        };
        Ok(Box::new(warp::reply::json(&response)))
    }
}

/// Execute Redis HGET command (MGET/{key}/{field})
async fn redis_hget(key: String, field: String, accept_header: Option<String>) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    let redis_client = {
        let client_guard = REDIS_CLIENT.lock().unwrap();
        client_guard.clone()
    };
    
    if let Some(client) = redis_client {
        match client.get_connection() {
            Ok(mut conn) => {
                match redis::cmd("HGET").arg(&key).arg(&field).query::<Option<String>>(&mut conn) {
                    Ok(value) => {
                        let response = HashFieldResponse {
                            success: true,
                            value,
                            error: None,
                        };
                        
                        let format = detect_response_format(accept_header);
                        match format {
                            ResponseFormat::Json => Ok(Box::new(warp::reply::json(&response))),
                            ResponseFormat::Xml => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_field_response_xml(&response)),
                                "content-type",
                                "application/xml"
                            ))),
                            ResponseFormat::Text => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_field_response_text(&response)),
                                "content-type",
                                "text/plain"
                            ))),
                        }
                    }
                    Err(e) => {
                        let response = HashFieldResponse {
                            success: false,
                            value: None,
                            error: Some(format!("Redis error: {}", e)),
                        };
                        
                        let format = detect_response_format(accept_header);
                        match format {
                            ResponseFormat::Json => Ok(Box::new(warp::reply::json(&response))),
                            ResponseFormat::Xml => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_field_response_xml(&response)),
                                "content-type",
                                "application/xml"
                            ))),
                            ResponseFormat::Text => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_field_response_text(&response)),
                                "content-type",
                                "text/plain"
                            ))),
                        }
                    }
                }
            }
            Err(e) => {
                let response = HashFieldResponse {
                    success: false,
                    value: None,
                    error: Some(format!("Connection error: {}", e)),
                };
                Ok(Box::new(warp::reply::json(&response)))
            }
        }
    } else {
        let response = HashFieldResponse {
            success: false,
            value: None,
            error: Some("Redis client not initialized".to_string()),
        };
        Ok(Box::new(warp::reply::json(&response)))
    }
}

/// Execute Redis HGETALL command (MGETALL/{key})
async fn redis_hgetall(key: String, accept_header: Option<String>) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    let redis_client = {
        let client_guard = REDIS_CLIENT.lock().unwrap();
        client_guard.clone()
    };
    
    if let Some(client) = redis_client {
        match client.get_connection() {
            Ok(mut conn) => {
                match redis::cmd("HGETALL").arg(&key).query::<HashMap<String, String>>(&mut conn) {
                    Ok(fields) => {
                        let response = HashAllResponse {
                            success: true,
                            fields: Some(fields),
                            error: None,
                        };
                        
                        let format = detect_response_format(accept_header);
                        match format {
                            ResponseFormat::Json => Ok(Box::new(warp::reply::json(&response))),
                            ResponseFormat::Xml => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_all_response_xml(&response)),
                                "content-type",
                                "application/xml"
                            ))),
                            ResponseFormat::Text => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_all_response_text(&response)),
                                "content-type",
                                "text/plain"
                            ))),
                        }
                    }
                    Err(e) => {
                        let response = HashAllResponse {
                            success: false,
                            fields: None,
                            error: Some(format!("Redis error: {}", e)),
                        };
                        
                        let format = detect_response_format(accept_header);
                        match format {
                            ResponseFormat::Json => Ok(Box::new(warp::reply::json(&response))),
                            ResponseFormat::Xml => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_all_response_xml(&response)),
                                "content-type",
                                "application/xml"
                            ))),
                            ResponseFormat::Text => Ok(Box::new(warp::reply::with_header(
                                warp::reply::html(format_hash_all_response_text(&response)),
                                "content-type",
                                "text/plain"
                            ))),
                        }
                    }
                }
            }
            Err(e) => {
                let response = HashAllResponse {
                    success: false,
                    fields: None,
                    error: Some(format!("Connection error: {}", e)),
                };
                Ok(Box::new(warp::reply::json(&response)))
            }
        }
    } else {
        let response = HashAllResponse {
            success: false,
            fields: None,
            error: Some("Redis client not initialized".to_string()),
        };
        Ok(Box::new(warp::reply::json(&response)))
    }
}

/// Start the HTTP server on port 4887
fn start_http_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if SERVER_STARTED.load(Ordering::Relaxed) {
        return Ok(()); // Server already started
    }

    let rt = Runtime::new()?;
    
    // Store the runtime globally
    {
        let mut runtime_guard = RUNTIME.lock().unwrap();
        *runtime_guard = Some(rt);
    }

    // Start the HTTP server in a background task
    let binding = RUNTIME.lock().unwrap();
    let rt_clone = binding.as_ref().unwrap();
    rt_clone.spawn(async {
        // Protected routes that require Redis authentication
        let auth_middleware = auth_middleware();
        
        // GET /GET/{key} - Redis GET command (protected)
        let get_route = warp::path!("GET" / String)
            .and(warp::get())
            .and(warp::header::optional::<String>("accept"))
            .and(auth_middleware.clone())
            .and_then(redis_get);

        // GET /MGET/{key}/{field} - Redis HGET command (protected)
        let hget_route = warp::path!("MGET" / String / String)
            .and(warp::get())
            .and(warp::header::optional::<String>("accept"))
            .and(auth_middleware.clone())
            .and_then(redis_hget);

        // GET /MGETALL/{key} - Redis HGETALL command (protected)
        let hgetall_route = warp::path!("MGETALL" / String)
            .and(warp::get())
            .and(warp::header::optional::<String>("accept"))
            .and(auth_middleware)
            .and_then(redis_hgetall);

        let routes = get_route
            .or(hget_route)
            .or(hgetall_route)
            .with(warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["content-type", "authorization"])
                .allow_methods(vec!["GET", "POST", "PUT", "DELETE"]));

        println!("Starting HTTP server on port 4887");
        println!("Available endpoints (all require Basic Auth with Redis credentials):");
        println!("  GET /GET/{{key}} - Redis GET command");
        println!("  GET /MGET/{{key}}/{{field}} - Redis HGET command");
        println!("  GET /MGETALL/{{key}} - Redis HGETALL command");
        println!("Response formats: JSON (default), XML (Accept: application/xml), Text (Accept: text/plain)");
        
        warp::serve(routes)
            .run(([0, 0, 0, 0], 4887))
            .await;
    });

    SERVER_STARTED.store(true, Ordering::Relaxed);
    Ok(())
}

/// Stop the HTTP server
fn stop_http_server() {
    SERVER_STARTED.store(false, Ordering::Relaxed);
    // Note: In a real implementation, you'd need a way to gracefully shutdown the server
}

/// HTTP.SERVER.START command implementation
fn http_server_start(_ctx: &Context, _args: Vec<RedisString>) -> RedisResult {
    match start_http_server() {
        Ok(_) => Ok(RedisValue::SimpleString("HTTP server started on port 4887".to_string())),
        Err(e) => Err(RedisError::String(format!("Failed to start HTTP server: {}", e))),
    }
}

/// HTTP.SERVER.STOP command implementation
fn http_server_stop(_ctx: &Context, _args: Vec<RedisString>) -> RedisResult {
    stop_http_server();
    Ok(RedisValue::SimpleString("HTTP server stopped".to_string()))
}

/// HTTP.SERVER.STATUS command implementation
fn http_server_status(_ctx: &Context, _args: Vec<RedisString>) -> RedisResult {
    let status = if SERVER_STARTED.load(Ordering::Relaxed) {
        "running"
    } else {
        "stopped"
    };
    Ok(RedisValue::SimpleString(format!("HTTP server status: {}", status)))
}


/// HTTP GET command implementation
fn http_get(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 2 {
        return Err(RedisError::WrongArity);
    }

    let url = args[1].to_string();
    
    // Validate URL
    if Url::parse(&url).is_err() {
        return Err(RedisError::String("Invalid URL format".to_string()));
    }

    let rt = Runtime::new().map_err(|e| RedisError::String(format!("Failed to create runtime: {}", e)))?;
    
    let response = rt.block_on(async {
        let client = reqwest::Client::new();
        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let headers: HashMap<String, String> = resp.headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                let body = resp.text().await.unwrap_or_default();
                
                Ok(HttpResponse { status, headers, body })
            }
            Err(e) => {
                Err(RedisError::String(format!("HTTP request failed: {}", e)))
            }
        }
    })?;

    let json_response = serde_json::to_string(&response)
        .map_err(|e| RedisError::String(format!("Failed to serialize response: {}", e)))?;

    Ok(RedisValue::SimpleString(json_response))
}

/// HTTP POST command implementation
fn http_post(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() < 2 || args.len() > 4 {
        return Err(RedisError::WrongArity);
    }

    let url = args[1].to_string();
    let body = if args.len() > 2 { Some(args[2].to_string()) } else { None };
    let content_type = if args.len() > 3 { Some(args[3].to_string()) } else { Some("application/json".to_string()) };
    
    // Validate URL
    if Url::parse(&url).is_err() {
        return Err(RedisError::String("Invalid URL format".to_string()));
    }

    let rt = Runtime::new().map_err(|e| RedisError::String(format!("Failed to create runtime: {}", e)))?;
    
    let response = rt.block_on(async {
        let client = reqwest::Client::new();
        let mut request = client.post(&url);
        
        if let Some(content_type) = content_type {
            request = request.header("Content-Type", content_type);
        }
        
        if let Some(body) = body {
            request = request.body(body);
        }
        
        match request.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let headers: HashMap<String, String> = resp.headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                let body = resp.text().await.unwrap_or_default();
                
                Ok(HttpResponse { status, headers, body })
            }
            Err(e) => {
                Err(RedisError::String(format!("HTTP request failed: {}", e)))
            }
        }
    })?;

    let json_response = serde_json::to_string(&response)
        .map_err(|e| RedisError::String(format!("Failed to serialize response: {}", e)))?;

    Ok(RedisValue::SimpleString(json_response))
}

/// HTTP PUT command implementation
fn http_put(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() < 2 || args.len() > 4 {
        return Err(RedisError::WrongArity);
    }

    let url = args[1].to_string();
    let body = if args.len() > 2 { Some(args[2].to_string()) } else { None };
    let content_type = if args.len() > 3 { Some(args[3].to_string()) } else { Some("application/json".to_string()) };
    
    // Validate URL
    if Url::parse(&url).is_err() {
        return Err(RedisError::String("Invalid URL format".to_string()));
    }

    let rt = Runtime::new().map_err(|e| RedisError::String(format!("Failed to create runtime: {}", e)))?;
    
    let response = rt.block_on(async {
        let client = reqwest::Client::new();
        let mut request = client.put(&url);
        
        if let Some(content_type) = content_type {
            request = request.header("Content-Type", content_type);
        }
        
        if let Some(body) = body {
            request = request.body(body);
        }
        
        match request.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let headers: HashMap<String, String> = resp.headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                let body = resp.text().await.unwrap_or_default();
                
                Ok(HttpResponse { status, headers, body })
            }
            Err(e) => {
                Err(RedisError::String(format!("HTTP request failed: {}", e)))
            }
        }
    })?;

    let json_response = serde_json::to_string(&response)
        .map_err(|e| RedisError::String(format!("Failed to serialize response: {}", e)))?;

    Ok(RedisValue::SimpleString(json_response))
}

/// HTTP DELETE command implementation
fn http_delete(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 2 {
        return Err(RedisError::WrongArity);
    }

    let url = args[1].to_string();
    
    // Validate URL
    if Url::parse(&url).is_err() {
        return Err(RedisError::String("Invalid URL format".to_string()));
    }

    let rt = Runtime::new().map_err(|e| RedisError::String(format!("Failed to create runtime: {}", e)))?;
    
    let response = rt.block_on(async {
        let client = reqwest::Client::new();
        match client.delete(&url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let headers: HashMap<String, String> = resp.headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                let body = resp.text().await.unwrap_or_default();
                
                Ok(HttpResponse { status, headers, body })
            }
            Err(e) => {
                Err(RedisError::String(format!("HTTP request failed: {}", e)))
            }
        }
    })?;

    let json_response = serde_json::to_string(&response)
        .map_err(|e| RedisError::String(format!("Failed to serialize response: {}", e)))?;

    Ok(RedisValue::SimpleString(json_response))
}

/// Initialize Redis client for HTTP server authentication
fn initialize_redis_client() {
    match Client::open("redis://127.0.0.1:6379/") {
        Ok(client) => {
            let mut redis_client_guard = REDIS_CLIENT.lock().unwrap();
            *redis_client_guard = Some(client);
            println!("Redis client initialized for HTTP authentication");
        }
        Err(e) => {
            eprintln!("Warning: Failed to initialize Redis client: {}", e);
        }
    }
}

/// Initialize HTTP server
fn initialize_http_server() {
    // Automatically start the HTTP server when the module loads
    if let Err(e) = start_http_server() {
        eprintln!("Warning: Failed to start HTTP server: {}", e);
    }
}

/// Module initialization function
fn module_init(_ctx: &Context, _args: &Vec<RedisString>) -> redis_module::raw::Status {
    initialize_redis_client();
    initialize_http_server();
    redis_module::raw::Status::Ok
}

#[cfg(not(test))]
redis_module! {
    name: "redis-http",
    version: 1,
    allocator: (redis_module::alloc::RedisAlloc, redis_module::alloc::RedisAlloc),
    data_types: [],
    init: module_init,
    commands: [
        ["HTTP.GET", http_get, "readonly", 1, 1, 1],
        ["HTTP.POST", http_post, "write", 1, 1, 1],
        ["HTTP.PUT", http_put, "write", 1, 1, 1],
        ["HTTP.DELETE", http_delete, "write", 1, 1, 1],
        ["HTTP.SERVER.START", http_server_start, "write", 1, 1, 1],
        ["HTTP.SERVER.STOP", http_server_stop, "write", 1, 1, 1],
        ["HTTP.SERVER.STATUS", http_server_status, "readonly", 1, 1, 1],
    ],
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_response_format_detection() {
        // Test JSON format detection
        let format = detect_response_format(Some("application/json".to_string()));
        assert!(matches!(format, ResponseFormat::Json));

        // Test XML format detection
        let format = detect_response_format(Some("application/xml".to_string()));
        assert!(matches!(format, ResponseFormat::Xml));

        // Test plain text format detection
        let format = detect_response_format(Some("text/plain".to_string()));
        assert!(matches!(format, ResponseFormat::Text));

        // Test default format (no Accept header)
        let format = detect_response_format(None);
        assert!(matches!(format, ResponseFormat::Json));

        // Test case insensitive detection
        let format = detect_response_format(Some("APPLICATION/XML".to_string()));
        assert!(matches!(format, ResponseFormat::Xml));
    }

    #[test]
    fn test_redis_response_json_formatting() {
        let response = RedisResponse {
            success: true,
            result: Some("test_value".to_string()),
            error: None,
        };

        let json_str = serde_json::to_string(&response).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"], "test_value");
        assert_eq!(parsed["error"], Value::Null);
    }

    #[test]
    fn test_redis_response_xml_formatting() {
        let response = RedisResponse {
            success: true,
            result: Some("test_value".to_string()),
            error: None,
        };

        let xml_str = format_redis_response_xml(&response);
        assert!(xml_str.contains("<success>true</success>"));
        assert!(xml_str.contains("<result>test_value</result>"));
        assert!(!xml_str.contains("<error>"));
    }

    #[test]
    fn test_redis_response_text_formatting() {
        let response = RedisResponse {
            success: true,
            result: Some("test_value".to_string()),
            error: None,
        };

        let text_str = format_redis_response_text(&response);
        assert_eq!(text_str, "test_value");
    }

    #[test]
    fn test_redis_response_error_formatting() {
        let response = RedisResponse {
            success: false,
            result: None,
            error: Some("Test error".to_string()),
        };

        // Test JSON
        let json_str = serde_json::to_string(&response).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["result"], Value::Null);
        assert_eq!(parsed["error"], "Test error");

        // Test XML
        let xml_str = format_redis_response_xml(&response);
        assert!(xml_str.contains("<success>false</success>"));
        assert!(xml_str.contains("<error>Test error</error>"));

        // Test Text
        let text_str = format_redis_response_text(&response);
        assert_eq!(text_str, "ERROR: Test error");
    }

    #[test]
    fn test_hash_field_response_formatting() {
        let response = HashFieldResponse {
            success: true,
            value: Some("field_value".to_string()),
            error: None,
        };

        // Test JSON
        let json_str = serde_json::to_string(&response).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["value"], "field_value");
        assert_eq!(parsed["error"], Value::Null);

        // Test XML
        let xml_str = format_hash_field_response_xml(&response);
        assert!(xml_str.contains("<success>true</success>"));
        assert!(xml_str.contains("<value>field_value</value>"));

        // Test Text
        let text_str = format_hash_field_response_text(&response);
        assert_eq!(text_str, "field_value");
    }

    #[test]
    fn test_hash_all_response_formatting() {
        let mut fields = std::collections::HashMap::new();
        fields.insert("key1".to_string(), "value1".to_string());
        fields.insert("key2".to_string(), "value2".to_string());

        let response = HashAllResponse {
            success: true,
            fields: Some(fields),
            error: None,
        };

        // Test JSON
        let json_str = serde_json::to_string(&response).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["fields"]["key1"], "value1");
        assert_eq!(parsed["fields"]["key2"], "value2");

        // Test XML
        let xml_str = format_hash_all_response_xml(&response);
        assert!(xml_str.contains("<success>true</success>"));
        assert!(xml_str.contains("<key>key1</key>"));
        assert!(xml_str.contains("<value>value1</value>"));
        assert!(xml_str.contains("<key>key2</key>"));
        assert!(xml_str.contains("<value>value2</value>"));

        // Test Text
        let text_str = format_hash_all_response_text(&response);
        assert!(text_str.contains("key1: value1"));
        assert!(text_str.contains("key2: value2"));
    }

    #[test]
    fn test_empty_hash_all_response() {
        let response = HashAllResponse {
            success: true,
            fields: Some(std::collections::HashMap::new()),
            error: None,
        };

        let text_str = format_hash_all_response_text(&response);
        assert_eq!(text_str, "OK");
    }

    #[test]
    fn test_nonexistent_key_response() {
        let response = RedisResponse {
            success: true,
            result: None,
            error: None,
        };

        let text_str = format_redis_response_text(&response);
        assert_eq!(text_str, "OK");
    }

}