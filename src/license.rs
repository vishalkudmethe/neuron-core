//! Enterprise licensing and registration engine for corporate environments.
//! Supports offline cryptographic verification of seat licenses.

use anyhow::{bail, Result};
use std::fs;
use std::path::PathBuf;
use sha2::{Sha256, Digest};

const SALT: &str = "neuron_enterprise_secret_salt_2026_xyz";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LicenseInfo {
    pub company: String,
    pub expiry: String,
    pub tier: String,
}

/// Retrieve the path to the stored license key file.
fn get_license_file_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".neuron").join("license.key")
}

/// Register a new enterprise license key.
pub fn register_key(key: &str) -> Result<LicenseInfo> {
    let info = verify_license_key(key)?;
    
    let path = get_license_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    fs::write(&path, key.trim())?;
    Ok(info)
}

/// Load and verify the currently installed license.
/// Returns Ok(LicenseInfo) if valid, or a default community tier info if missing/invalid.
pub fn get_active_license() -> LicenseInfo {
    let path = get_license_file_path();
    if !path.exists() {
        return LicenseInfo {
            company: "Community User".to_string(),
            expiry: "Never".to_string(),
            tier: "Community (AGPL-3.0)".to_string(),
        };
    }
    
    match fs::read_to_string(&path) {
        Ok(key) => match verify_license_key(&key) {
            Ok(info) => info,
            Err(_) => LicenseInfo {
                company: "Community User".to_string(),
                expiry: "Never".to_string(),
                tier: "Community (AGPL-3.0) - [Invalid Key]".to_string(),
            },
        },
        Err(_) => LicenseInfo {
            company: "Community User".to_string(),
            expiry: "Never".to_string(),
            tier: "Community (AGPL-3.0)".to_string(),
        },
    }
}

/// Validate a license key format and signature.
/// Key format: NEURON-ENT-<COMPANY>-<EXPIRY>-<SIGNATURE>
pub fn verify_license_key(key: &str) -> Result<LicenseInfo> {
    let key = key.trim();
    if !key.starts_with("NEURON-ENT-") {
        bail!("Invalid license prefix. Must start with 'NEURON-ENT-'");
    }
    
    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() < 5 {
        bail!("Invalid license key format structure");
    }
    
    // Structure:
    // parts[0]: NEURON
    // parts[1]: ENT
    // parts[2]: Company Name (base64 encoded or raw if split-safe, let's assume raw string but replacing spaces with '_')
    // parts[3]: Expiry Date (YYYYMMDD)
    // parts[4]: Hex-encoded SHA-256 signature of "company|expiry|salt"
    let company = parts[2].replace('_', " ");
    let expiry_str = parts[3];
    let signature = parts[4];
    
    // Compute signature
    let input = format!("{}|{}|{}", company, expiry_str, SALT);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let computed_sig = hex::encode(hasher.finalize());
    
    if computed_sig != signature {
        bail!("Cryptographic verification signature mismatch");
    }
    
    // Format expiry date string
    let formatted_expiry = if expiry_str.len() == 8 {
        format!("{}-{}-{}", &expiry_str[0..4], &expiry_str[4..6], &expiry_str[6..8])
    } else {
        expiry_str.to_string()
    };
    
    Ok(LicenseInfo {
        company,
        expiry: formatted_expiry,
        tier: "Enterprise (Commercial)".to_string(),
    })
}

/// Utility to generate a valid license key for test/demonstration.
#[allow(dead_code)]
pub fn generate_test_license_key(company: &str, expiry_yyyymmdd: &str) -> String {
    let comp_param = company.replace(' ', "_");
    let input = format!("{}|{}|{}", company, expiry_yyyymmdd, SALT);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let sig = hex::encode(hasher.finalize());
    format!("NEURON-ENT-{}-{}-{}", comp_param, expiry_yyyymmdd, sig)
}
