use alloc::string::{String, ToString};
use anyhow::{anyhow, Result};
use scale::Decode;

use crate::quote::Quote;
use crate::QuoteCollateralV3;

#[cfg(not(feature = "js"))]
use core::time::Duration;

fn get_header(resposne: &reqwest::Response, name: &str) -> Result<String> {
    let value = resposne
        .headers()
        .get(name)
        .ok_or(anyhow!("Missing {name}"))?
        .to_str()?;
    let value = urlencoding::decode(value)?;
    Ok(value.into_owned())
}

/// Get collateral given DCAP quote and base URL of PCCS server URL.
///
/// # Arguments
///
/// * `pccs_url` - The base URL of PCCS server. (e.g. `https://pccs.example.com/sgx/certification/v4`)
/// * `quote` - The raw quote to verify. Supported SGX and TDX quotes.
/// * `timeout` - The timeout for the request. (e.g. `Duration::from_secs(10)`)
///
/// # Returns
///
/// * `Ok(QuoteCollateralV3)` - The quote collateral
/// * `Err(Error)` - The error
pub async fn get_collateral(
    pccs_url: &str,
    mut quote: &[u8],
    #[cfg(not(feature = "js"))] timeout: Duration,
) -> Result<QuoteCollateralV3> {
    let quote = Quote::decode(&mut quote)?;
    let fmspc = hex::encode_upper(quote.fmspc().map_err(|_| anyhow!("get fmspc error"))?);
    let builder = reqwest::Client::builder();
    #[cfg(not(feature = "js"))]
    let builder = builder.danger_accept_invalid_certs(true).timeout(timeout);
    let client = builder.build()?;
    let base_url = pccs_url.trim_end_matches('/');

    let tcb_info_issuer_chain;
    let raw_tcb_info;
    {
        let resposne = client
            .get(format!("{base_url}/tcb?fmspc={fmspc}"))
            .send()
            .await?;
        tcb_info_issuer_chain = get_header(&resposne, "SGX-TCB-Info-Issuer-Chain")
            .or(get_header(&resposne, "TCB-Info-Issuer-Chain"))?;
        raw_tcb_info = resposne.text().await?;
    };
    let qe_identity_issuer_chain;
    let raw_qe_identity;
    {
        let response = client.get(format!("{base_url}/qe/identity")).send().await?;
        qe_identity_issuer_chain = get_header(&response, "SGX-Enclave-Identity-Issuer-Chain")?;
        raw_qe_identity = response.text().await?;
    };

    let tcb_info_json: serde_json::Value =
        serde_json::from_str(&raw_tcb_info).map_err(|_| anyhow!("TCB Info should a JSON"))?;
    let tcb_info = tcb_info_json["tcbInfo"].to_string();
    let tcb_info_signature = tcb_info_json
        .get("signature")
        .ok_or(anyhow!("TCB Info should has `signature` field"))?
        .as_str()
        .ok_or(anyhow!("TCB Info signature should a hex string"))?;
    let tcb_info_signature = hex::decode(tcb_info_signature)
        .map_err(|_| anyhow!("TCB Info signature should a hex string"))?;

    let qe_identity_json: serde_json::Value = serde_json::from_str(raw_qe_identity.as_str())
        .map_err(|_| anyhow!("QE Identity should a JSON"))?;
    let qe_identity = qe_identity_json
        .get("enclaveIdentity")
        .ok_or(anyhow!("QE Identity should has `enclaveIdentity` field"))?
        .to_string();
    let qe_identity_signature = qe_identity_json
        .get("signature")
        .ok_or(anyhow!("QE Identity should has `signature` field"))?
        .as_str()
        .ok_or(anyhow!("QE Identity signature should a hex string"))?;
    let qe_identity_signature = hex::decode(qe_identity_signature)
        .map_err(|_| anyhow!("QE Identity signature should a hex string"))?;

    Ok(QuoteCollateralV3 {
        tcb_info_issuer_chain,
        tcb_info,
        tcb_info_signature,
        qe_identity_issuer_chain,
        qe_identity,
        qe_identity_signature,
    })
}

/// Get collateral given DCAP quote from Intel PCS.
///
/// # Arguments
///
/// * `quote` - The raw quote to verify. Supported SGX and TDX quotes.
/// * `timeout` - The timeout for the request. (e.g. `Duration::from_secs(10)`)
///
/// # Returns
///
/// * `Ok(QuoteCollateralV3)` - The quote collateral
/// * `Err(Error)` - The error
pub async fn get_collateral_from_pcs(
    quote: &[u8],
    #[cfg(not(feature = "js"))] timeout: Duration,
) -> Result<QuoteCollateralV3> {
    const PCS_URL: &str = "https://api.trustedservices.intel.com/tdx/certification/v4";
    get_collateral(
        PCS_URL,
        quote,
        #[cfg(not(feature = "js"))]
        timeout,
    )
    .await
}
