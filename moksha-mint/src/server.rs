use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::error::MokshaMintError;
use axum::extract::{Query, State};
use axum::routing::{get_service, post};
use axum::Router;
use axum::{routing::get, Json};
use moksha_core::keyset::Keysets;

use crate::info::{MintInfoResponse, Parameter};

use crate::mint::Mint;
use crate::model::{GetMintQuery, PostMintQuery};
use hyper::http::{HeaderName, HeaderValue};
use hyper::Method;
use moksha_core::primitives::{
    CheckFeesRequest, CheckFeesResponse, PaymentRequest, PostMeltRequest, PostMeltResponse,
    PostMintRequest, PostMintResponse, PostSplitRequest, PostSplitResponse,
};
use secp256k1::PublicKey;

use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{event, info, Level};

use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub async fn run_server(
    mint: Mint,
    addr: SocketAddr,
    serve_wallet_path: Option<PathBuf>,
    api_prefix: Option<String>,
) -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();
    info!("listening on: {}", addr);
    info!("mint_info: {:?}", mint.mint_info);
    info!("lightning_backend: {}", mint.lightning_type);
    if serve_wallet_path.is_some() {
        info!(
            "serving wallet from path: {:?}",
            serve_wallet_path.clone().unwrap()
        );
    }

    axum::Server::bind(&addr)
        .serve(
            app(mint, serve_wallet_path, api_prefix)
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_headers(Any)
                        .allow_methods([Method::GET, Method::POST]),
                )
                .into_make_service(),
        )
        .await?;

    Ok(())
}

fn app(mint: Mint, serve_wallet_path: Option<PathBuf>, prefix: Option<String>) -> Router {
    let routes = Router::new()
        .route("/keys", get(get_keys))
        .route("/keysets", get(get_keysets))
        .route("/mint", get(get_mint).post(post_mint))
        .route("/checkfees", post(post_check_fees))
        .route("/melt", post(post_melt))
        .route("/split", post(post_split))
        .route("/info", get(get_info));

    let router = Router::new()
        .nest(&prefix.unwrap_or_else(|| "".to_owned()), routes)
        .with_state(mint)
        .layer(TraceLayer::new_for_http());

    if let Some(serve_wallet_path) = serve_wallet_path {
        return router.nest_service(
            "/",
            get_service(ServeDir::new(serve_wallet_path))
                .layer::<_, _, Infallible>(SetResponseHeaderLayer::if_not_present(
                    HeaderName::from_static("cross-origin-embedder-policy"),
                    HeaderValue::from_static("require-corp"),
                ))
                .layer(SetResponseHeaderLayer::if_not_present(
                    HeaderName::from_static("cross-origin-opener-policy"),
                    HeaderValue::from_static("same-origin"),
                )),
        );
    }
    router
}

async fn post_split(
    State(mint): State<Mint>,
    Json(split_request): Json<PostSplitRequest>,
) -> Result<Json<PostSplitResponse>, MokshaMintError> {
    let response = mint
        .split(&split_request.proofs, &split_request.outputs)
        .await?;

    Ok(Json(response))
}

async fn post_melt(
    State(mint): State<Mint>,
    Json(melt_request): Json<PostMeltRequest>,
) -> Result<Json<PostMeltResponse>, MokshaMintError> {
    let (paid, preimage, change) = mint
        .melt(melt_request.pr, &melt_request.proofs, &melt_request.outputs)
        .await?;

    Ok(Json(PostMeltResponse {
        paid,
        preimage,
        change,
    }))
}

async fn post_check_fees(
    State(mint): State<Mint>,
    Json(_check_fees): Json<CheckFeesRequest>,
) -> Result<Json<CheckFeesResponse>, MokshaMintError> {
    let invoice = mint.lightning.decode_invoice(_check_fees.pr).await?;

    Ok(Json(CheckFeesResponse {
        fee: mint.fee_reserve(
            invoice
                .amount_milli_satoshis()
                .ok_or_else(|| crate::error::MokshaMintError::InvalidAmount)?,
        ),
    }))
}

async fn get_info(State(mint): State<Mint>) -> Result<Json<MintInfoResponse>, MokshaMintError> {
    let mint_info = MintInfoResponse {
        name: mint.mint_info.name,
        pubkey: mint.keyset.mint_pubkey,
        version: match mint.mint_info.version {
            true => Some(env!("CARGO_PKG_VERSION").to_owned()),
            _ => None,
        },
        description: mint.mint_info.description,
        description_long: mint.mint_info.description_long,
        contact: mint.mint_info.contact,
        nuts: vec![
            "NUT-00".to_string(),
            "NUT-01".to_string(),
            "NUT-02".to_string(),
            "NUT-03".to_string(),
            "NUT-04".to_string(),
            "NUT-05".to_string(),
            "NUT-06".to_string(),
            "NUT-09".to_string(),
        ],
        motd: mint.mint_info.motd,
        parameter: Parameter {
            peg_out_only: false,
        },
    };
    Ok(Json(mint_info))
}

async fn get_mint(
    State(mint): State<Mint>,
    Query(mint_query): Query<GetMintQuery>,
) -> Result<Json<PaymentRequest>, MokshaMintError> {
    let (pr, hash) = mint.create_invoice(mint_query.amount).await?;
    Ok(Json(PaymentRequest { pr, hash }))
}

async fn post_mint(
    State(mint): State<Mint>,
    Query(mint_query): Query<PostMintQuery>,
    Json(blinded_messages): Json<PostMintRequest>,
) -> Result<Json<PostMintResponse>, MokshaMintError> {
    event!(
        Level::INFO,
        "post_mint: {mint_query:#?} {blinded_messages:#?}"
    );

    let promises = mint
        .mint_tokens(mint_query.hash, &blinded_messages.outputs)
        .await?;
    Ok(Json(PostMintResponse { promises }))
}

async fn get_keys(
    State(mint): State<Mint>,
) -> Result<Json<HashMap<u64, PublicKey>>, MokshaMintError> {
    Ok(Json(mint.keyset.public_keys))
}

async fn get_keysets(State(mint): State<Mint>) -> Result<Json<Keysets>, MokshaMintError> {
    Ok(Json(Keysets::new(vec![mint.keyset.keyset_id])))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use crate::server::app;
    use hyper::{Body, Request, StatusCode};
    use moksha_core::keyset::Keysets;
    use secp256k1::PublicKey;
    use tower::ServiceExt;

    use crate::{
        database::MockDatabase,
        info::{MintInfoResponse, MintInfoSettings},
        lightning::{LightningType, MockLightning},
        mint::{LightningFeeConfig, Mint},
    };

    #[tokio::test]
    async fn test_get_keys() -> anyhow::Result<()> {
        let app = app(create_mock_mint(Default::default()), None, None);
        let response = app
            .oneshot(Request::builder().uri("/keys").body(Body::empty())?)
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await?;
        let keys: HashMap<u64, PublicKey> = serde_json::from_slice(&body)?;
        assert_eq!(64, keys.len());
        Ok(())
    }

    #[tokio::test]
    async fn test_get_keysets() -> anyhow::Result<()> {
        let app = app(create_mock_mint(Default::default()), None, None);
        let response = app
            .oneshot(Request::builder().uri("/keysets").body(Body::empty())?)
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await?;
        let keysets = serde_json::from_slice::<Keysets>(&body)?;
        assert_eq!(Keysets::new(vec!["53eJP2+qJyTd".to_string()]), keysets);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_info() -> anyhow::Result<()> {
        let mint_info_settings = MintInfoSettings {
            name: Some("Bob's Cashu mint".to_string()),
            version: true,
            description: Some("A mint for testing".to_string()),
            description_long: Some("A mint for testing long".to_string()),
            ..Default::default()
        };
        let app = app(create_mock_mint(mint_info_settings), None, None);
        let response = app
            .oneshot(Request::builder().uri("/info").body(Body::empty())?)
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await?;
        let info = serde_json::from_slice::<MintInfoResponse>(&body)?;
        assert!(!info.parameter.peg_out_only);
        assert_eq!(info.nuts.len(), 8);
        assert_eq!(info.name, Some("Bob's Cashu mint".to_string()));
        assert_eq!(info.description, Some("A mint for testing".to_string()));
        assert_eq!(
            info.description_long,
            Some("A mint for testing long".to_string())
        );
        Ok(())
    }

    fn create_mock_mint(mint_info: MintInfoSettings) -> Mint {
        let db = Arc::new(MockDatabase::new());
        let lightning = Arc::new(MockLightning::new());

        Mint::new(
            "mytestsecret".to_string(),
            "".to_string(),
            lightning,
            LightningType::Lnbits(Default::default()),
            db,
            LightningFeeConfig::default(),
            mint_info,
        )
    }
}