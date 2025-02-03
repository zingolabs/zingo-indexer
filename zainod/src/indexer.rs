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
use zaino_serve::server::{error::ServerError, grpc::TonicServer};
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
    indexer_status: AtomicStatus,
    service_status: AtomicStatus,
    tonic_server_status: AtomicStatus,
}

impl IndexerStatus {
    /// Creates a new IndexerStatus.
    pub fn new() -> Self {
        IndexerStatus {
            indexer_status: AtomicStatus::new(StatusType::Offline.into()),
            service_status: AtomicStatus::new(StatusType::Offline.into()),
            tonic_server_status: AtomicStatus::new(StatusType::Offline.into()),
        }
    }

    /// Returns the IndexerStatus.
    pub fn load(&self) -> IndexerStatus {
        self.indexer_status.load();
        self.service_status.load();
        self.tonic_server_status.load();
        self.clone()
    }
}

/// Zingo-Indexer.
pub struct Indexer {
    /// Indexer configuration data.
    config: IndexerConfig,
    /// GRPC server.
    server: Option<TonicServer>,
    /// Internal block cache.
    _service: IndexerService<FetchService>,
    /// Indexers status.
    status: IndexerStatus,
    /// Online status of the indexer.
    online: Arc<AtomicBool>,
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

    /// Creates a new Indexer.
    ///
    /// Currently only takes an IndexerConfig.
    pub async fn new(config: IndexerConfig, online: Arc<AtomicBool>) -> Result<Self, IndexerError> {
        config.check_config()?;
        let status = IndexerStatus::new();
        let tcp_ingestor_listen_addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            config.listen_port,
        );
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
        let service = IndexerService::<FetchService>::spawn(
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
        let server = Some(todo!());
        println!("Server Ready.");
        Ok(Indexer {
            config,
            server,
            _service: service,
            status,
            online,
        })
    }

    /// Starts Indexer Service and returns its JoinHandle
    pub async fn serve(
        mut self,
    ) -> Result<tokio::task::JoinHandle<Result<(), IndexerError>>, IndexerError> {
        Ok(tokio::task::spawn(async move {
            // NOTE: This interval may need to be reduced or removed / moved once scale testing begins.
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));
            let server_handle = if let Some(server) = self.server.take() {
                Some(server.serve().await)
            } else {
                return Err(IndexerError::MiscIndexerError(
                    "Server Missing! Fatal Error!.".to_string(),
                ));
            };

            self.status.indexer_status.store(StatusType::Ready.into());
            println!("Zaino listening on port {:?}.", self.config.listen_port);
            loop {
                self.status.load();
                // indexer.log_status();
                if self.check_for_shutdown() {
                    self.status.indexer_status.store(StatusType::Closing.into());
                    self.shutdown_components(server_handle).await;
                    self.status.indexer_status.store(StatusType::Offline.into());
                    return Ok(());
                }
                interval.tick().await;
            }
        }))
    }

    /// Checks indexers online status and servers internal status for closure signal.
    fn check_for_shutdown(&self) -> bool {
        if self.status() >= 4 {
            return true;
        }
        if !self.check_online() {
            return true;
        }
        false
    }

    /// Sets the servers to close gracefully.
    pub fn shutdown(&mut self) {
        self.status.indexer_status.store(StatusType::Closing.into())
    }

    /// Sets the server's components to close gracefully.
    async fn shutdown_components(
        &mut self,
        server_handle: Option<tokio::task::JoinHandle<Result<(), ServerError>>>,
    ) {
        if let Some(handle) = server_handle {
            self.status
                .server_status
                .server_status
                .store(StatusType::Closing.into());
            handle.await.ok();
        }
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
        self.status.load();
        self.status.clone()
    }

    /// Check the online status on the indexer.
    fn check_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
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
