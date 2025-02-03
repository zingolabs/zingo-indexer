//! Zingo-Indexer implementation.

use std::{
    net::SocketAddr,
    process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use zaino_fetch::jsonrpc::connector::test_node_and_return_uri;
use zaino_serve::server::{config::GrpcConfig, error::ServerError, grpc::TonicServer};
use zaino_state::{
    config::FetchServiceConfig,
    fetch::FetchService,
    indexer::{IndexerService, ZcashService},
    status::{AtomicStatus, StatusType},
};

use crate::{config::IndexerConfig, error::IndexerError};

/// Holds the status of the server and all its components.
#[derive(Debug, Clone)]
pub struct IndexerStatus {
    pub indexer_status: AtomicStatus,
    service_status: AtomicStatus,
    grpc_server_status: AtomicStatus,
}

impl IndexerStatus {
    /// Creates a new IndexerStatus.
    pub fn new() -> Self {
        IndexerStatus {
            indexer_status: AtomicStatus::new(StatusType::Offline.into()),
            service_status: AtomicStatus::new(StatusType::Offline.into()),
            grpc_server_status: AtomicStatus::new(StatusType::Offline.into()),
        }
    }

    /// Returns the IndexerStatus.
    pub fn load(&self) -> IndexerStatus {
        self.indexer_status.load();
        self.service_status.load();
        self.grpc_server_status.load();
        self.clone()
    }
}

/// Zingo-Indexer.
pub struct Indexer {
    /// Indexer configuration data.
    config: IndexerConfig,
    /// GRPC server.
    server: Option<TonicServer>,
    /// Chain fetch service state process handler..
    service: Option<IndexerService<FetchService>>,
    /// Indexers status.
    status: IndexerStatus,
}

impl Indexer {
    /// Starts Indexer service.
    ///
    /// Currently only takes an IndexerConfig.
    pub async fn start(config: IndexerConfig) -> Result<(), IndexerError> {
        let online = Arc::new(AtomicBool::new(true));
        set_ctrlc(online.clone());
        startup_message();
        println!("Launching Zaino..");
        let indexer: Indexer = Indexer::new(config, online.clone()).await?;
        indexer.serve().await?.await?
    }

    /// Spawns a new Indexer server.
    pub async fn spawn(
        config: IndexerConfig,
        status: IndexerStatus,
    ) -> Result<(Self, tokio::task::JoinHandle<Result<(), IndexerError>>), IndexerError> {
        config.check_config()?;

        println!("Checking connection with node..");
        let zebrad_uri = test_node_and_return_uri(
            &config.zebrad_port,
            config.node_user.clone(),
            config.node_password.clone(),
        )
        .await?;

        println!(
            " - Connected to node using JsonRPC at address {}.",
            zebrad_uri
        );

        status.indexer_status.store(StatusType::Spawning.into());

        let chain_state_service = IndexerService::<FetchService>::spawn(
            FetchServiceConfig::new(
                SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                    config.zebrad_port,
                ),
                config.node_user.clone(),
                config.node_password.clone(),
                None,
                None,
                config.map_capacity,
                config.map_shard_amount,
                config.db_path.clone(),
                config.db_size,
                config.get_network()?,
                config.no_sync,
                config.no_db,
            ),
            status.service_status.clone(),
        )
        .await?;

        let grpc_server = TonicServer::spawn(
            chain_state_service.inner_ref().get_subscriber(),
            GrpcConfig {
                grpc_listen_address: config.grpc_listen_address,
                tls: config.tls,
                tls_cert_path: config.tls_cert_path.clone(),
                tls_key_path: config.tls_key_path.clone(),
            },
            status.grpc_server_status.clone(),
        )
        .await
        .unwrap();

        let indexer = Indexer {
            config,
            server: Some(grpc_server),
            service: Some(chain_state_service),
            status,
        };

        // NOTE: This interval may need to be reduced or removed / moved once scale testing begins.
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));
        let serve_task = tokio::task::spawn(async move {
            loop {
                indexer.status.load();
                if indexer.check_for_shutdown() {
                    indexer.shutdown().await;
                    return Ok(());
                }
                interval.tick().await;
            }
        });

        Ok((indexer, serve_task))
    }

    /// Checks indexers online status and servers internal status for closure signal.
    fn check_for_shutdown(&self) -> bool {
        if self.status() >= 4 {
            return true;
        }
        false
    }

    /// Sets the servers to close gracefully.
    pub async fn shutdown(&mut self) {
        self.status
            .grpc_server_status
            .store(StatusType::Closing.into());
        self.status.service_status.store(StatusType::Closing.into());
        self.status.indexer_status.store(StatusType::Closing.into());

        if let Some(mut server) = self.server.take() {
            server.shutdown();
            self.status
                .grpc_server_status
                .store(StatusType::Offline.into());
        }

        if let Some(service) = self.service.take() {
            service.inner().close();
            self.status.service_status.store(StatusType::Offline.into());
        }

        self.status.indexer_status.store(StatusType::Offline.into());
    }

    /// Returns the indexers current status usize.
    pub fn status(&self) -> usize {
        self.status.indexer_status.load()
    }

    /// Returns the indexers current statustype.
    pub fn statustype(&self) -> StatusType {
        StatusType::from(self.status())
    }

    /// Returns the status of the indexer and its parts.
    pub fn statuses(&mut self) -> IndexerStatus {
        self.status.load()
    }
}

fn set_ctrlc(online: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        online.store(false, Ordering::SeqCst);
        process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
}

fn startup_message() {
    let welcome_message = r#"
       ░░░░░░░▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒░░░▒▒░░░░░
       ░░░░▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒████▓░▒▒▒░░
       ░░▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒████▓▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒░▒▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓▓▓▒▒▒▒▒▒▒▒▒▒▒▒▓▓▒▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▒▒▒▒▒██▓▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▒▒██▓▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▒▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓███▓██▓▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▒▓▓▓▓▒███▓░▒▓▓████████████████▓▓▒▒▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▓▓▓▓▒▓████▓▓███████████████████▓▒▓▓▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▓▓▓▓▓▒▒▓▓▓▓████████████████████▓▒▓▓▓▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▓▓▓▓▓█████████████████████████▓▒▓▓▓▓▓▒▒▒▒▒
       ▒▒▒▒▒▒▒▓▓▓▒▓█████████████████████████▓▓▓▓▓▓▓▓▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▓▓▓████████████████████████▓▓▓▓▓▓▓▓▓▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▓▒███████████████████████▒▓▓▓▓▓▓▓▓▓▓▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▓███████████████████▓▓▓▓▓▓▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▓███████████████▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒▒▒▓██████████▓▓▒▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒███▓▒▓▓▓▓▓▒▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▓████▒▒▒▒▒▒▒▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
       ▒▒▒▒▒▒▒░▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
              Thank you for using ZingoLabs Zaino!

       - Donate to us at https://free2z.cash/zingolabs.
       - Submit any security conserns to us at zingodisclosure@proton.me.

****** Please note Zaino is currently in development and should not be used to run mainnet nodes. ******
    "#;
    println!("{}", welcome_message);
}
