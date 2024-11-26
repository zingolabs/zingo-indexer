//! Holds wallet-to-validator tests for Zaino.

#![forbid(unsafe_code)]

use zaino_testutils::TestManager2;

mod wallet_basic {
    use super::*;

    #[tokio::test]
    async fn connect_to_node_get_info() {
        for validator in ["zebrad", "zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
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
}
