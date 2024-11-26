//! Utility functions for Zingo-Indexer Testing.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use once_cell::sync::Lazy;
use std::{io::Write, path::PathBuf, str::FromStr};
use tempfile::TempDir;
use zcash_local_net::validator::Validator;

static CTRL_C_ONCE: std::sync::Once = std::sync::Once::new();

/// Configuration data for Zingo-Indexer Tests.
pub struct TestManager {
    /// Temporary Directory for zcashd and lightwalletd configuration and regtest data.
    pub temp_conf_dir: tempfile::TempDir,
    /// Zingolib regtest manager.
    pub regtest_manager: zingolib::testutils::regtest::RegtestManager,
    /// Zingolib regtest network.
    pub regtest_network: zingolib::config::RegtestNetwork,
    /// Zingo-Indexer gRPC listen port.
    pub indexer_port: u16,
    /// Zebrad/Zcashd JsonRpc listen port.
    pub zebrad_port: u16,
    /// Online status of Zingo-Indexer.
    pub online: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TestManager {
    /// Launches a zingo regtest manager and zingo-indexer, created TempDir for configuration and log files.
    pub async fn launch(
        online: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> (
        Self,
        zingolib::testutils::regtest::ChildProcessHandler,
        tokio::task::JoinHandle<Result<(), zainodlib::error::IndexerError>>,
    ) {
        let lwd_port = portpicker::pick_unused_port().expect("No ports free");
        let zebrad_port = portpicker::pick_unused_port().expect("No ports free");
        let indexer_port = portpicker::pick_unused_port().expect("No ports free");

        let temp_conf_dir = create_temp_conf_files(lwd_port, zebrad_port).unwrap();
        let temp_conf_path = temp_conf_dir.path().to_path_buf();

        set_custom_drops(online.clone(), Some(temp_conf_path.clone()));

        let regtest_network = zingolib::config::RegtestNetwork::new(1, 1, 1, 1, 1, 1);

        let regtest_manager =
            zingolib::testutils::regtest::RegtestManager::new(temp_conf_path.clone());
        let regtest_handler = regtest_manager
            .launch(true)
            .expect("Failed to start regtest services");

        // NOTE: queue and workerpool sizes may need to be changed here.
        let indexer_config = zainodlib::config::IndexerConfig {
            tcp_active: true,
            listen_port: Some(indexer_port),
            lightwalletd_port: lwd_port,
            zebrad_port,
            node_user: Some("xxxxxx".to_string()),
            node_password: Some("xxxxxx".to_string()),
            max_queue_size: 512,
            max_worker_pool_size: 96,
            idle_worker_pool_size: 48,
        };
        let indexer_handler =
            zainodlib::indexer::Indexer::start_indexer_service(indexer_config, online.clone())
                .await
                .unwrap();
        // NOTE: This is required to give the server time to launch, this is not used in production code but could be rewritten to improve testing efficiency.
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        (
            TestManager {
                temp_conf_dir,
                regtest_manager,
                regtest_network,
                indexer_port,
                zebrad_port,
                online,
            },
            regtest_handler,
            indexer_handler,
        )
    }

    /// Returns zingo-indexer listen address.
    pub fn get_indexer_uri(&self) -> http::Uri {
        http::Uri::builder()
            .scheme("http")
            .authority(format!("127.0.0.1:{0}", self.indexer_port))
            .path_and_query("")
            .build()
            .unwrap()
    }

    /// Returns zebrad listen address.
    pub async fn test_and_return_zebrad_uri(&self) -> http::Uri {
        zaino_fetch::jsonrpc::connector::test_node_and_return_uri(
            &self.zebrad_port,
            Some("xxxxxx".to_string()),
            Some("xxxxxx".to_string()),
        )
        .await
        .unwrap()
    }

    /// Builds aand returns Zingolib lightclient.
    pub async fn build_lightclient(&self) -> zingolib::lightclient::LightClient {
        let mut client_builder = zingolib::testutils::scenarios::setup::ClientBuilder::new(
            self.get_indexer_uri(),
            self.temp_conf_dir.path().to_path_buf(),
        );
        client_builder
            .build_faucet(false, self.regtest_network)
            .await
    }
}

/// Closes test manager child processes, optionally cleans configuration and log files for test.
pub async fn drop_test_manager(
    temp_conf_path: Option<std::path::PathBuf>,
    child_process_handler: zingolib::testutils::regtest::ChildProcessHandler,
    online: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    online.store(false, std::sync::atomic::Ordering::SeqCst);
    drop(child_process_handler);

    let mut temp_wallet_path = temp_conf_path.clone().unwrap();
    if let Some(dir_name) = temp_wallet_path.file_name().and_then(|n| n.to_str()) {
        let new_dir_name = format!("{}_client_1", dir_name);
        temp_wallet_path.set_file_name(new_dir_name); // Update the directory name
    }

    if let Some(ref path) = temp_conf_path {
        if let Err(e) = std::fs::remove_dir_all(path) {
            eprintln!(
                "Failed to delete temporary regtest configuration directory: {:?}.",
                e
            );
        }
    }
    if let Some(ref path) = Some(temp_wallet_path) {
        if let Err(e) = std::fs::remove_dir_all(path) {
            eprintln!("Failed to delete temporary directory: {:?}.", e);
        }
    }
}

fn set_custom_drops(
    online: std::sync::Arc<std::sync::atomic::AtomicBool>,
    temp_conf_path: Option<std::path::PathBuf>,
) {
    let online_panic = online.clone();
    let online_ctrlc = online.clone();
    let temp_conf_path_panic = temp_conf_path.clone();
    let temp_conf_path_ctrlc = temp_conf_path.clone();

    let mut temp_wallet_path = temp_conf_path.unwrap();
    if let Some(dir_name) = temp_wallet_path.file_name().and_then(|n| n.to_str()) {
        let new_dir_name = format!("{}_client_1", dir_name);
        temp_wallet_path.set_file_name(new_dir_name); // Update the directory name
    }
    let temp_wallet_path_panic = Some(temp_wallet_path.clone());
    let temp_wallet_path_ctrlc = Some(temp_wallet_path.clone());

    let default_panic_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        default_panic_hook(panic_info);
        online_panic.store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(ref path) = temp_conf_path_panic {
            if let Err(e) = std::fs::remove_dir_all(path) {
                eprintln!(
                    "Failed to delete temporary regtest config directory: {:?}.",
                    e
                );
            }
        }
        if let Some(ref path) = temp_wallet_path_panic {
            if let Err(e) = std::fs::remove_dir_all(path) {
                eprintln!("Failed to delete temporary wallet directory: {:?}.", e);
            }
        }
        // Ensures tests fail on secondary thread panics.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(false);
        }
        std::process::exit(0);
    }));

    CTRL_C_ONCE.call_once(|| {
        ctrlc::set_handler(move || {
            println!("Received Ctrl+C, exiting.");
            online_ctrlc.store(false, std::sync::atomic::Ordering::SeqCst);
            if let Some(ref path) = temp_conf_path_ctrlc {
                if let Err(e) = std::fs::remove_dir_all(path) {
                    eprintln!(
                        "Failed to delete temporary regtest config directory: {:?}.",
                        e
                    );
                }
            }
            if let Some(ref path) = temp_wallet_path_ctrlc {
                if let Err(e) = std::fs::remove_dir_all(path) {
                    eprintln!("Failed to delete temporary wallet directory: {:?}.", e);
                }
            }
            // Ensures tests fail on ctrlc exit.
            #[allow(clippy::assertions_on_constants)]
            {
                assert!(false);
            }
            std::process::exit(0);
        })
        .expect("Error setting Ctrl-C handler");
    })
}

fn write_lightwalletd_yml(dir: &std::path::Path, bind_addr_port: u16) -> std::io::Result<()> {
    let file_path = dir.join("lightwalletd.yml");
    let mut file = std::fs::File::create(file_path)?;
    writeln!(file, "grpc-bind-addr: 127.0.0.1:{}", bind_addr_port)?;
    writeln!(file, "cache-size: 10")?;
    writeln!(file, "log-level: 10")?;
    writeln!(file, "zcash-conf-path: ../conf/zcash.conf")?;

    Ok(())
}

fn write_zcash_conf(dir: &std::path::Path, rpcport: u16) -> std::io::Result<()> {
    let file_path = dir.join("zcash.conf");
    let mut file = std::fs::File::create(file_path)?;
    writeln!(file, "regtest=1")?;
    writeln!(file, "nuparams=5ba81b19:1 # Overwinter")?;
    writeln!(file, "nuparams=76b809bb:1 # Sapling")?;
    writeln!(file, "nuparams=2bb40e60:1 # Blossom")?;
    writeln!(file, "nuparams=f5b9230b:1 # Heartwood")?;
    writeln!(file, "nuparams=e9ff75a6:1 # Canopy")?;
    writeln!(file, "nuparams=c2d6d0b4:1 # NU5")?;
    writeln!(file, "txindex=1")?;
    writeln!(file, "insightexplorer=1")?;
    writeln!(file, "experimentalfeatures=1")?;
    writeln!(file, "rpcuser=xxxxxx")?;
    writeln!(file, "rpcpassword=xxxxxx")?;
    writeln!(file, "rpcport={}", rpcport)?;
    writeln!(file, "rpcallowip=127.0.0.1")?;
    writeln!(file, "listen=0")?;
    writeln!(file, "minetolocalwallet=0")?;
    // writeln!(file, "mineraddress=zregtestsapling1fmq2ufux3gm0v8qf7x585wj56le4wjfsqsj27zprjghntrerntggg507hxh2ydcdkn7sx8kya7p")?; // USE FOR SAPLING.
    writeln!(file, "mineraddress=uregtest1zkuzfv5m3yhv2j4fmvq5rjurkxenxyq8r7h4daun2zkznrjaa8ra8asgdm8wwgwjvlwwrxx7347r8w0ee6dqyw4rufw4wg9djwcr6frzkezmdw6dud3wsm99eany5r8wgsctlxquu009nzd6hsme2tcsk0v3sgjvxa70er7h27z5epr67p5q767s2z5gt88paru56mxpm6pwz0cu35m")?;

    Ok(())
}

fn create_temp_conf_files(lwd_port: u16, rpcport: u16) -> std::io::Result<tempfile::TempDir> {
    let temp_dir = tempfile::Builder::new()
        .prefix("zingoindexertest")
        .tempdir()?;
    let conf_dir = temp_dir.path().join("conf");
    std::fs::create_dir(&conf_dir)?;
    write_lightwalletd_yml(&conf_dir, lwd_port)?;
    write_zcash_conf(&conf_dir, rpcport)?;
    Ok(temp_dir)
}

/// Contains zingolib::lightclient functionality used for zaino testing.
pub mod zingo_lightclient {
    /// Returns the zcash address of the Zingolib::lightclient.
    pub async fn get_address(
        zingo_client: &zingolib::lightclient::LightClient,
        pool: &str,
    ) -> String {
        zingolib::get_base_address_macro!(zingo_client, pool)
    }
}

// *** New testutils code below - Code above will be replaced and integration tests rewritten in separate PR ***

/// Path for zcashd binary.
pub static ZCASHD_BIN: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let mut workspace_root_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root_path.pop();
    Some(workspace_root_path.join("test_binaries/bins/zcashd"))
});

/// Path for zcash-cli binary.
pub static ZCASH_CLI_BIN: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let mut workspace_root_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root_path.pop();
    Some(workspace_root_path.join("test_binaries/bins/zcash-cli"))
});

/// Path for zebrad binary.
pub static ZEBRAD_BIN: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let mut workspace_root_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root_path.pop();
    Some(workspace_root_path.join("test_binaries/bins/zebrad"))
});

/// Path for lightwalletd binary.
pub static LIGHTWALLETD_BIN: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let mut workspace_root_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root_path.pop();
    Some(workspace_root_path.join("test_binaries/bins/lightwalletd"))
});

/// Path for zainod binary.
pub static ZAINOD_BIN: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let mut workspace_root_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root_path.pop();
    Some(workspace_root_path.join("target/release/zainod"))
});

/// Path for zcashd chain cache.
pub static ZCASHD_CHAIN_CACHE_BIN: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let mut workspace_root_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root_path.pop();
    Some(workspace_root_path.join("integration-tests/chain_cache/client_rpc_tests"))
});

/// Path for zebrad chain cache.
pub static ZEBRAD_CHAIN_CACHE_BIN: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let mut workspace_root_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root_path.pop();
    Some(workspace_root_path.join("integration-tests/chain_cache/client_rpc_tests_large"))
});

/// Represents the type of validator to launch.
pub enum ValidatorKind {
    /// Zcashd.
    Zcashd,
    /// Zebrad.
    Zebrad,
}

impl std::str::FromStr for ValidatorKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "zcashd" => Ok(ValidatorKind::Zcashd),
            "zebrad" => Ok(ValidatorKind::Zebrad),
            _ => Err(format!("Invalid validator kind: {}", s)),
        }
    }
}

/// Config for validators.
pub enum ValidatorConfig {
    /// Zcashd Config.
    ZcashdConfig(zcash_local_net::validator::ZcashdConfig),
    /// Zebrad Config.
    ZebradConfig(zcash_local_net::validator::ZebradConfig),
}

/// Available zcash-local-net configurations.
pub enum LocalNet {
    /// Zcash-local-net backed by Zcashd.
    Zcashd(
        zcash_local_net::LocalNet<
            zcash_local_net::indexer::Empty,
            zcash_local_net::validator::Zcashd,
        >,
    ),
    /// Zcash-local-net backed by Zebrad.
    Zebrad(
        zcash_local_net::LocalNet<
            zcash_local_net::indexer::Empty,
            zcash_local_net::validator::Zebrad,
        >,
    ),
}

impl zcash_local_net::validator::Validator for LocalNet {
    const CONFIG_FILENAME: &str = "";

    type Config = ValidatorConfig;

    #[allow(clippy::manual_async_fn)]
    fn launch(
        config: Self::Config,
    ) -> impl std::future::Future<Output = Result<Self, zcash_local_net::error::LaunchError>> + Send
    {
        async move {
            match config {
                ValidatorConfig::ZcashdConfig(cfg) => {
                    let net = zcash_local_net::LocalNet::<
                        zcash_local_net::indexer::Empty,
                        zcash_local_net::validator::Zcashd,
                    >::launch(
                        zcash_local_net::indexer::EmptyConfig {}, cfg
                    )
                    .await;
                    Ok(LocalNet::Zcashd(net))
                }
                ValidatorConfig::ZebradConfig(cfg) => {
                    let net = zcash_local_net::LocalNet::<
                        zcash_local_net::indexer::Empty,
                        zcash_local_net::validator::Zebrad,
                    >::launch(
                        zcash_local_net::indexer::EmptyConfig {}, cfg
                    )
                    .await;
                    Ok(LocalNet::Zebrad(net))
                }
            }
        }
    }

    fn stop(&mut self) {
        match self {
            LocalNet::Zcashd(net) => net.validator_mut().stop(),
            LocalNet::Zebrad(net) => net.validator_mut().stop(),
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn generate_blocks(
        &self,
        n: u32,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send {
        async move {
            match self {
                LocalNet::Zcashd(net) => net.validator().generate_blocks(n).await,
                LocalNet::Zebrad(net) => net.validator().generate_blocks(n).await,
            }
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn get_chain_height(
        &self,
    ) -> impl std::future::Future<Output = zcash_protocol::consensus::BlockHeight> + Send {
        async move {
            match self {
                LocalNet::Zcashd(net) => net.validator().get_chain_height().await,
                LocalNet::Zebrad(net) => net.validator().get_chain_height().await,
            }
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn poll_chain_height(
        &self,
        target_height: zcash_protocol::consensus::BlockHeight,
    ) -> impl std::future::Future<Output = ()> + Send {
        async move {
            match self {
                LocalNet::Zcashd(net) => net.validator().poll_chain_height(target_height).await,
                LocalNet::Zebrad(net) => net.validator().poll_chain_height(target_height).await,
            }
        }
    }

    fn config_dir(&self) -> &TempDir {
        match self {
            LocalNet::Zcashd(net) => net.validator().config_dir(),
            LocalNet::Zebrad(net) => net.validator().config_dir(),
        }
    }

    fn logs_dir(&self) -> &TempDir {
        match self {
            LocalNet::Zcashd(net) => net.validator().logs_dir(),
            LocalNet::Zebrad(net) => net.validator().logs_dir(),
        }
    }

    fn data_dir(&self) -> &TempDir {
        match self {
            LocalNet::Zcashd(net) => net.validator().data_dir(),
            LocalNet::Zebrad(net) => net.validator().data_dir(),
        }
    }

    fn network(&self) -> zcash_local_net::network::Network {
        match self {
            LocalNet::Zcashd(net) => net.validator().network(),
            LocalNet::Zebrad(net) => *net.validator().network(),
        }
    }

    fn load_chain(
        chain_cache: PathBuf,
        validator_data_dir: PathBuf,
        validator_network: zcash_local_net::network::Network,
    ) -> PathBuf {
        match validator_network {
            zcash_local_net::network::Network::Regtest => {
                zcash_local_net::validator::Zcashd::load_chain(
                    chain_cache,
                    validator_data_dir,
                    validator_network,
                )
            }
            _ => zcash_local_net::validator::Zebrad::load_chain(
                chain_cache,
                validator_data_dir,
                validator_network,
            ),
        }
    }
}

/// Holds zingo lightclients along with thier TempDir for wallet-2-validator tests.
pub struct Clients {
    /// Lightclient TempDir location.
    pub lightclient_dir: TempDir,
    /// Faucet (zingolib lightclient).
    ///
    /// Mining rewards are recieved by this client for use in tests.
    pub faucet: zingolib::lightclient::LightClient,
    /// Recipient (zingolib lightclient).
    pub recipient: zingolib::lightclient::LightClient,
}

impl Clients {
    /// Returns the zcash address of the faucet.
    pub async fn get_faucet_address(&self, pool: &str) -> String {
        zingolib::get_base_address_macro!(self.faucet, pool)
    }

    /// Returns the zcash address of the recipient.
    pub async fn get_recipient_address(&self, pool: &str) -> String {
        zingolib::get_base_address_macro!(self.recipient, pool)
    }
}

/// Configuration data for Zingo-Indexer Tests.
pub struct TestManager2 {
    /// Zcash-local-net.
    pub local_net: LocalNet,
    /// Zebrad/Zcashd JsonRpc listen port.
    pub zebrad_rpc_listen_port: u16,
    /// Zaino Indexer JoinHandle.
    pub zaino_handle: Option<tokio::task::JoinHandle<Result<(), zainodlib::error::IndexerError>>>,
    /// Zingo-Indexer gRPC listen port.
    pub zaino_grpc_listen_port: Option<u16>,
    /// Zingolib lightclients.
    pub clients: Option<Clients>,
    /// Online status of Zingo-Indexer.
    pub online: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TestManager2 {
    /// Launches zcash-local-net<Empty, Validator>.
    ///
    /// Possible validators: Zcashd, Zebrad.
    ///
    /// If chain_cache is given a path the chain will be loaded.
    ///
    /// If clients is set to active zingolib lightclients will be created for test use.
    pub async fn launch(
        validator: &str,
        chain_cache: Option<PathBuf>,
        enable_zaino: bool,
        enable_clients: bool,
    ) -> Result<Self, std::io::Error> {
        let validator_kind = ValidatorKind::from_str(validator).unwrap();
        if enable_clients && !enable_zaino {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Cannot enable clients when zaino is not enabled.",
            ));
        }
        let online = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

        // Launch LocalNet:
        let zebrad_rpc_listen_port = portpicker::pick_unused_port().expect("No ports free");
        let validator_config = match validator_kind {
            ValidatorKind::Zcashd => {
                let cfg = zcash_local_net::validator::ZcashdConfig {
                    zcashd_bin: ZCASHD_BIN.clone(),
                    zcash_cli_bin: ZCASH_CLI_BIN.clone(),
                    rpc_port: Some(zebrad_rpc_listen_port),
                    activation_heights: zcash_local_net::network::ActivationHeights::default(),
                    miner_address: Some(zingolib::testvectors::REG_O_ADDR_FROM_ABANDONART),
                    chain_cache,
                };
                ValidatorConfig::ZcashdConfig(cfg)
            }
            ValidatorKind::Zebrad => {
                let cfg = zcash_local_net::validator::ZebradConfig {
                    zebrad_bin: ZEBRAD_BIN.clone(),
                    network_listen_port: None,
                    rpc_listen_port: Some(zebrad_rpc_listen_port),
                    activation_heights: zcash_local_net::network::ActivationHeights::default(),
                    miner_address: zcash_local_net::validator::ZEBRAD_DEFAULT_MINER,
                    chain_cache,
                    network: zcash_local_net::network::Network::Regtest,
                };
                ValidatorConfig::ZebradConfig(cfg)
            }
        };
        let local_net = LocalNet::launch(validator_config).await.unwrap();

        // Launch Zaino:
        let (zaino_grpc_listen_port, zaino_handle) = if enable_zaino {
            let zaino_grpc_listen_port = portpicker::pick_unused_port().expect("No ports free");
            // NOTE: queue and workerpool sizes may need to be changed here.
            let indexer_config = zainodlib::config::IndexerConfig {
                tcp_active: true,
                listen_port: Some(zaino_grpc_listen_port),
                // NOTE: Remove field from IndexerConfig with the removal of current testutils.
                lightwalletd_port: portpicker::pick_unused_port().expect("No ports free"),
                zebrad_port: zebrad_rpc_listen_port,
                node_user: Some("xxxxxx".to_string()),
                node_password: Some("xxxxxx".to_string()),
                max_queue_size: 512,
                max_worker_pool_size: 64,
                idle_worker_pool_size: 4,
            };
            let handle = zainodlib::indexer::Indexer::new(indexer_config, online.clone())
                .await
                .unwrap()
                .serve()
                .await
                .unwrap();
            // NOTE: This is required to give the server time to launch, this is not used in production code but could be rewritten to improve testing efficiency.
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            (Some(zaino_grpc_listen_port), Some(handle))
        } else {
            (None, None)
        };

        // Launch Zingolib Lightclients:
        let clients = if enable_clients {
            let lightclient_dir = tempfile::tempdir().unwrap();
            let lightclients = zcash_local_net::client::build_lightclients(
                lightclient_dir.path().to_path_buf(),
                zaino_grpc_listen_port
                    .expect("Error launching zingo lightclients. `enable_zaino` is None."),
            )
            .await;
            Some(Clients {
                lightclient_dir,
                faucet: lightclients.0,
                recipient: lightclients.1,
            })
        } else {
            None
        };

        Ok(Self {
            local_net,
            zebrad_rpc_listen_port,
            zaino_handle,
            zaino_grpc_listen_port,
            clients,
            online,
        })
    }

    /// Closes the TestManager.
    pub async fn close(&mut self) {
        self.online
            .store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(zaino_handle) = self.zaino_handle.take() {
            if let Err(e) = zaino_handle.await {
                eprintln!("Error awaiting zaino_handle: {:?}", e);
            }
        }
    }
}

impl Drop for TestManager2 {
    fn drop(&mut self) {
        self.online
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn launch_testmanager_zebrad() {
        let mut test_manager = TestManager2::launch("zebrad", None, false, false)
            .await
            .unwrap();
        assert_eq!(
            1,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zcashd() {
        let mut test_manager = TestManager2::launch("zcashd", None, false, false)
            .await
            .unwrap();
        assert_eq!(
            1,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zebrad_generate_blocks() {
        let mut test_manager = TestManager2::launch("zebrad", None, false, false)
            .await
            .unwrap();
        assert_eq!(
            1,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.local_net.generate_blocks(1).await.unwrap();
        assert_eq!(
            2,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zcashd_generate_blocks() {
        let mut test_manager = TestManager2::launch("zcashd", None, false, false)
            .await
            .unwrap();
        assert_eq!(
            1,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.local_net.generate_blocks(1).await.unwrap();
        assert_eq!(
            2,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zebrad_with_chain() {
        let mut test_manager =
            TestManager2::launch("zebrad", ZEBRAD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();
        assert_eq!(
            52,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zcashd_with_chain() {
        let mut test_manager =
            TestManager2::launch("zcashd", ZCASHD_CHAIN_CACHE_BIN.clone(), false, false)
                .await
                .unwrap();
        assert_eq!(
            10,
            u32::from(test_manager.local_net.get_chain_height().await)
        );
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zebrad_zaino() {
        let mut test_manager = TestManager2::launch("zebrad", None, true, false)
            .await
            .unwrap();
        let mut grpc_client =
            zcash_local_net::client::build_client(zcash_local_net::network::localhost_uri(
                test_manager
                    .zaino_grpc_listen_port
                    .expect("Zaino listen port not available but zaino is active."),
            ))
            .await
            .unwrap();
        grpc_client
            .get_lightd_info(tonic::Request::new(
                zcash_client_backend::proto::service::Empty {},
            ))
            .await
            .unwrap();
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zcashd_zaino() {
        let mut test_manager = TestManager2::launch("zcashd", None, true, false)
            .await
            .unwrap();
        let mut grpc_client =
            zcash_local_net::client::build_client(zcash_local_net::network::localhost_uri(
                test_manager
                    .zaino_grpc_listen_port
                    .expect("Zaino listen port is not available but zaino is active."),
            ))
            .await
            .unwrap();
        grpc_client
            .get_lightd_info(tonic::Request::new(
                zcash_client_backend::proto::service::Empty {},
            ))
            .await
            .unwrap();
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zebrad_zaino_clients() {
        let mut test_manager = TestManager2::launch("zebrad", None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");
        clients.faucet.do_info().await;
        clients.recipient.do_info().await;
        test_manager.close().await;
    }

    #[tokio::test]
    async fn launch_testmanager_zcashd_zaino_clients() {
        let mut test_manager = TestManager2::launch("zcashd", None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");
        clients.faucet.do_info().await;
        clients.recipient.do_info().await;
        test_manager.close().await;
    }
}
