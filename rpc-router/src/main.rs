use anyhow::Result;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, warn};
use hyper::Uri;

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
            zebra_url: env::var("ZEBRA_URL").unwrap_or_else(|_| "http://zebra:18232".to_string()),
            zallet_url: env::var("ZALLET_URL").unwrap_or_else(|_| "http://zallet:28232".to_string()),
            zaino_url: env::var("ZAINO_URL").unwrap_or_else(|_| "http://zaino:8237".to_string()),
        }
    }
}

async fn forward_request(req: Request<Full<Bytes>>, target_url: &str) -> Result<Response<Full<Bytes>>> {
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();

    let uri_string = format!("{}{}", target_url, req.uri().path_and_query().map(|x| x.as_str()).unwrap_or("/"));
    let uri: Uri = uri_string.parse()?;

    let (parts, body) = req.into_parts();
    let mut new_req = Request::builder()
        .method(parts.method)
        .uri(uri)
        .version(parts.version);

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

async fn handler(req: Request<hyper::body::Incoming>, config: Arc<Config>) -> Result<Response<Full<Bytes>>> {
    // Health check
    if req.uri().path() == "/health" {
        return Ok(Response::new(Full::new(Bytes::from("OK"))));
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
    
    // todo: move the map of methods to service config
    // Attempt to parse method from body
    let target_url = if let Ok(rpc_req) = serde_json::from_slice::<RpcRequest>(&body_bytes) {
        match rpc_req.method.as_str() {
            // Zallet (Wallet) methods
            "z_sendmany" | "getnewaddress" | "z_listreceivedbyaddress" | "z_getbalance" | "z_gettotalbalance" | "z_getoperationresult" | "z_getoperationstatus" | "z_listoperationids" | "z_validateaddress" => {
                info!("Routing {} to Zallet", rpc_req.method);
                &config.zallet_url
            },
            // Zaino (Indexer) methods
            "getaddressbalance" | "getaddresstxids" | "getaddressutxos" | "getaddressdeltas" | "getaddressmempool" => {
                info!("Routing {} to Zaino", rpc_req.method);
                &config.zaino_url
            },
            // Default to Zebra for everything else (getblock, sendrawtransaction, getinfo, etc.)
            method => {
                info!("Routing {} to Zebra", method);
                &config.zebra_url
            }
        }
    } else {
        warn!("Failed to parse JSON-RPC body, defaulting to Zebra");
        &config.zebra_url
    };

    // Reconstruct request with buffered body
    let new_req = Request::from_parts(parts, Full::new(body_bytes));
    
    match forward_request(new_req, target_url).await {
        Ok(res) => Ok(res),
        Err(e) => {
            error!("Forwarding error: {}", e);
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!("Bad Gateway: {}", e))))
                .unwrap())
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = Arc::new(Config::from_env());
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    let listener = TcpListener::bind(addr).await?;
    info!("RPC Router listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let config = config.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| handler(req, config.clone())))
                .await
            {
                error!("Error serving connection: {:?}", err);
            }
        });
    }
}
