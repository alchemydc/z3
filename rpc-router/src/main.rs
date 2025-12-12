use std::{env, net::SocketAddr, sync::Arc};

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use http_body_util::{BodyExt, Full};
use hyper::{
    body::Bytes,
    header::{HeaderName, HeaderValue},
    server::conn::http1,
    service::service_fn,
    Request, Response, StatusCode, Uri,
};
use hyper_util::{
    client::legacy::Client as HyperClient,
    rt::{TokioExecutor, TokioIo},
};
use reqwest::Client as ReqwestClient;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

#[derive(Deserialize, Debug)]
struct RpcRequest {
    method: String,
}

#[derive(Clone)]
struct Config {
    zebra_url: String,
    zallet_url: String,
    zaino_url: String,
}

impl Config {
    fn from_env() -> Self {
        Self {
            zebra_url: env::var("ZEBRA_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:20617".to_string()),
            zallet_url: env::var("ZALLET_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:25617".to_string()),
            zaino_url: env::var("ZAINO_URL").unwrap_or_else(|_| "http://zaino:8237".to_string()),
        }
    }
}

#[derive(Clone)]
struct Z3Schema {
    zebra_methods: Vec<Value>,
    zallet_methods: Vec<Value>,
    merged: Value,
}

async fn forward_request(
    req: Request<Full<Bytes>>,
    target_url: &str,
) -> Result<Response<Full<Bytes>>> {
    let client = HyperClient::builder(TokioExecutor::new()).build_http();

    let uri_string = format!(
        "{}{}",
        target_url,
        req.uri()
            .path_and_query()
            .map(|x| x.as_str())
            .unwrap_or("/")
    );
    let uri: Uri = uri_string.parse()?;

    let (parts, body) = req.into_parts();
    let mut new_req = Request::builder()
        .method(parts.method)
        .uri(uri)
        .version(parts.version);

    // Inject auth header
    let auth = general_purpose::STANDARD.encode("zebra:zebra");
    new_req = new_req.header(
        hyper::header::AUTHORIZATION,
        hyper::header::HeaderValue::from_str(&format!("Basic {}", auth))?,
    );

    // Copy other headers
    for (k, v) in parts.headers {
        if let Some(key) = k {
            new_req = new_req.header(key, v);
        }
    }

    let new_req = new_req.body(body)?;
    let res = client.request(new_req).await?;

    let (parts, body) = res.into_parts();
    let body_bytes = body.collect().await?.to_bytes();
    let new_res = Response::from_parts(parts, Full::new(body_bytes));

    Ok(new_res)
}
fn add_cors_headers(mut resp: Response<Full<Bytes>>) -> Response<Full<Bytes>> {
    let headers = resp.headers_mut();
    for &(name, value) in &[
        (
            "access-control-allow-origin",
            "https://playground.open-rpc.org",
        ),
        ("access-control-allow-methods", "POST, OPTIONS"),
        ("access-control-allow-headers", "Content-Type"),
        ("access-control-max-age", "86400"),
    ] {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }

    resp
}

async fn handler(
    req: Request<hyper::body::Incoming>,
    config: Arc<Config>,
    z3: Z3Schema,
) -> Result<Response<Full<Bytes>>> {
    // Health check
    if req.uri().path() == "/health" {
        return Ok(Response::new(Full::new(Bytes::from("OK"))));
    }

    // Handle CORS preflight
    if req.method() == hyper::Method::OPTIONS {
        let resp = add_cors_headers(
            Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Full::new(Bytes::new()))
                .unwrap(),
        );
        return Ok(resp);
    }

    // Only handle POST requests for JSON-RPC
    if req.method() != hyper::Method::POST {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method Not Allowed")))
            .unwrap());
    }

    // Buffer the body to parse it
    let (parts, body) = req.into_parts();
    let body_bytes = body.collect().await?.to_bytes();

    // Attempt to parse method from body
    let target_url = if let Ok(rpc_req) = serde_json::from_slice::<RpcRequest>(&body_bytes) {
        if rpc_req.method == "rpc.discover" {
            info!("Routing rpc.discover to merged schema");

            return Ok(add_cors_headers(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(hyper::header::CONTENT_TYPE, "application/json")
                    .body(Full::new(Bytes::from(serde_json::to_string(&z3.merged)?)))
                    .expect("z3 merged schema response should be valid"),
            ));
        }

        if let Some(_method) = z3
            .zebra_methods
            .iter()
            .find(|m| m["name"] == rpc_req.method)
        {
            info!("Routing {} to Zebra", rpc_req.method);
            &config.zebra_url
        } else if let Some(_method) = z3
            .zallet_methods
            .iter()
            .find(|m| m["name"] == rpc_req.method)
        {
            info!("Routing {} to Zallet", rpc_req.method);
            &config.zallet_url
        } else {
            info!("Routing {} to Zaino", rpc_req.method);
            &config.zaino_url
        }
    } else {
        warn!("Failed to parse JSON-RPC body, defaulting to Zebra");
        &config.zebra_url
    };

    // Reconstruct request with buffered body
    let new_req = Request::from_parts(parts, Full::new(body_bytes));

    match forward_request(new_req, target_url).await {
        Ok(res) => Ok(add_cors_headers(res)),
        Err(e) => {
            error!("Forwarding error: {}", e);

            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!("Bad Gateway: {}", e))))
                .unwrap())
        }
    }
}

async fn call_rpc_discover(url: &str) -> Result<serde_json::Value> {
    let client = ReqwestClient::new();

    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "rpc.discover",
        "params": []
    });

    let text = client
        .post(url)
        .basic_auth("zebra", Some("zebra"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let resp = serde_json::from_str::<serde_json::Value>(&text)?;

    Ok(resp)
}

fn extract_methods_array(schema: &Value) -> Vec<Value> {
    schema["methods"].as_array().cloned().unwrap_or_default()
}

fn annotate_methods_with_server(methods: &mut Vec<Value>, server_name: &str) {
    for m in methods {
        m.as_object_mut()
            .unwrap()
            .insert("x-server".to_string(), json!(server_name));
    }
}

fn merge_components_schemas(schema: &Value, combined: &mut serde_json::Map<String, Value>) {
    if let Some(obj) = schema["components"]["schemas"].as_object() {
        for (k, v) in obj {
            combined.insert(k.clone(), v.clone());
        }
    }
}

fn merge_openrpc_schemas(zebra: Value, zallet: Value) -> Result<Z3Schema> {
    // Extract method arrays
    let mut zebra_methods = extract_methods_array(&zebra);
    let mut zallet_methods = extract_methods_array(&zallet);

    // Annotate each method with its origin
    annotate_methods_with_server(&mut zebra_methods, "zebra");
    annotate_methods_with_server(&mut zallet_methods, "zallet");

    // Merge schemas under components.schemas
    let mut combined_schemas = serde_json::Map::new();
    merge_components_schemas(&zebra, &mut combined_schemas);
    merge_components_schemas(&zallet, &mut combined_schemas);

    let mut combined_methods = Vec::new();
    combined_methods.extend(zebra_methods.clone());
    combined_methods.extend(zallet_methods.clone());

    // Build final merged schema
    let merged = json!({
        "openrpc": zebra["openrpc"].clone(),
        "info": {
            "title":  env!("CARGO_PKG_NAME"),
            "description": env!("CARGO_PKG_DESCRIPTION"),
            "version": env!("CARGO_PKG_VERSION"),
        },
        "servers": [
            { "name": "router",  "url": "http://localhost:8080/" },
        ],
        "methods": combined_methods,
        "components": {
            "schemas": combined_schemas
        }
    });

    let z3 = Z3Schema {
        zebra_methods,
        zallet_methods,
        merged: Value::Object(
            merged
                .as_object()
                .expect("merged object should be valid")
                .clone(),
        ),
    };

    Ok(z3)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = Arc::new(Config::from_env());
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    let zebra_schema = call_rpc_discover(&config.zebra_url).await?["result"].clone();
    let zallet_schema = call_rpc_discover(&config.zallet_url).await?["result"].clone();

    let z3 = merge_openrpc_schemas(zebra_schema, zallet_schema)?;

    println!("{}", serde_json::to_string_pretty(&z3.merged)?);

    let listener = TcpListener::bind(addr).await?;
    info!("RPC Router listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let config = config.clone();

        let z3 = z3.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| handler(req, config.clone(), z3.clone())),
                )
                .await
            {
                error!("Error serving connection: {:?}", err);
            }
        });
    }
}
