/// AI-NEURON™ Build Script
/// 
/// Injects the enterprise license signing salt at compile time via
/// the NEURON_LICENSE_SALT environment variable.
///
/// For production CI/CD builds, set NEURON_LICENSE_SALT as a secret
/// environment variable. The value is baked into the binary at link
/// time and is never visible in source code.
///
/// For community/dev builds without the env var, a neutral placeholder
/// is used — community builds cannot validate commercial enterprise keys.
fn main() {
    // Rerun this build script if the env var changes
    println!("cargo:rerun-if-env-changed=NEURON_LICENSE_SALT");

    let salt = std::env::var("NEURON_LICENSE_SALT")
        .unwrap_or_else(|_| "community_dev_build_no_enterprise".to_string());

    // Pass the salt to the Rust compiler as a compile-time env var
    println!("cargo:rustc-env=NEURON_LICENSE_SALT={}", salt);
}
