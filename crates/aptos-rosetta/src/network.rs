// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use crate::common::{get_latest_block_height, get_oldest_block_height};
use crate::{
    common::{check_network, handle_request, with_context, with_empty_request},
    error::ApiError,
    types::{
        Allow, MetadataRequest, NetworkListResponse, NetworkOptionsResponse, NetworkRequest,
        NetworkStatusResponse, OperationStatusType, OperationType, Version,
    },
    RosettaContext, NODE_VERSION, ROSETTA_VERSION,
};
use aptos_logger::{debug, trace};
use warp::Filter;

pub fn list_route(
    server_context: RosettaContext,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("network" / "list")
        .and(warp::post())
        .and(with_empty_request())
        .and(with_context(server_context))
        .and_then(handle_request(network_list))
}

pub fn options_route(
    server_context: RosettaContext,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("network" / "options")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_context(server_context))
        .and_then(handle_request(network_options))
}

pub fn status_route(
    server_context: RosettaContext,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("network" / "status")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_context(server_context))
        .and_then(handle_request(network_status))
}

/// List [`NetworkIdentifier`]s supported by this proxy aka [`ChainId`]s
///
/// This should be able to run without a running full node connection
///
/// [API Spec](https://www.rosetta-api.org/docs/NetworkApi.html#networklist)
async fn network_list(
    _empty: MetadataRequest,
    server_context: RosettaContext,
) -> Result<NetworkListResponse, ApiError> {
    debug!("/network/list");
    trace!(
        server_context = ?server_context,
        "network_list",
    );

    let response = NetworkListResponse {
        network_identifiers: vec![server_context.chain_id.into()],
    };

    Ok(response)
}

/// Get Network options
///
/// This lists out all errors, operations, and statuses, along with versioning information.
/// This should be able to run without a running full node connection
///
/// [API Spec](https://www.rosetta-api.org/docs/NetworkApi.html#networkoptions)
async fn network_options(
    request: NetworkRequest,
    server_context: RosettaContext,
) -> Result<NetworkOptionsResponse, ApiError> {
    debug!("/network/options");
    trace!(
        request = ?request,
        server_context = ?server_context,
        "network_options",
    );

    check_network(request.network_identifier, &server_context)?;

    let version = Version {
        rosetta_version: ROSETTA_VERSION.to_string(),
        // TODO: Get from node via REST API
        node_version: NODE_VERSION.to_string(),
        middleware_version: "0.1.0".to_string(),
    };

    let operation_statuses = OperationStatusType::all()
        .into_iter()
        .map(|status| status.into())
        .collect();
    let operation_types = OperationType::all()
        .into_iter()
        .map(|op| op.to_string())
        .collect();
    let errors = ApiError::all()
        .into_iter()
        .map(|err| err.into_error())
        .collect();

    let allow = Allow {
        operation_statuses,
        operation_types,
        errors,
        historical_balance_lookup: true,
        timestamp_start_index: 2,
        call_methods: vec![],
        balance_exemptions: vec![],
        mempool_coins: false,
    };

    let response = NetworkOptionsResponse { version, allow };

    Ok(response)
}

/// Get network status including the latest state
///
/// This should respond with the latest ledger version, timestamp, and genesis information
///
/// [API Spec](https://www.rosetta-api.org/docs/NetworkApi.html#networkoptions)
async fn network_status(
    request: NetworkRequest,
    server_context: RosettaContext,
) -> Result<NetworkStatusResponse, ApiError> {
    debug!("/network/status");
    trace!(
        request = ?request,
        server_context = ?server_context,
        "network_status",
    );

    check_network(request.network_identifier, &server_context)?;
    let chain_id = server_context.chain_id;
    let rest_client = server_context.rest_client()?;
    let block_cache = server_context.block_cache()?;
    let genesis_block_identifier = block_cache
        .get_block_info_by_height(0, chain_id)
        .await?
        .block_id;
    let response = rest_client.get_ledger_information().await?;
    let state = response.state();

    let oldest_block_height = get_oldest_block_height(&server_context, state);
    let latest_block_height = get_latest_block_height(&server_context, state);

    // Get the oldest block
    let oldest_block_identifier = block_cache
        .get_block_info_by_height(oldest_block_height, chain_id)
        .await?
        .block_id;

    // Get the latest block
    let current_block = block_cache
        .get_block_info_by_height(latest_block_height, chain_id)
        .await?;
    let current_block_identifier = current_block.block_id;

    let response = NetworkStatusResponse {
        current_block_identifier,
        current_block_timestamp: current_block.timestamp,
        genesis_block_identifier,
        oldest_block_identifier,
        sync_status: None,
        peers: vec![],
    };

    Ok(response)
}
