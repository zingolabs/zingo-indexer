//! Holds wallet-to-validator tests for Zaino.

#![forbid(unsafe_code)]

use zaino_testutils::TestManager2;
use zcash_local_net::validator::Validator;
use zingolib::testutils::lightclient::from_inputs;

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

    #[tokio::test]
    async fn send_mining_reward() {
        for validator in ["zebrad", "zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .as_ref()
                .expect("Clients are not initialized");
            clients.faucet.do_sync(true).await.unwrap();
            assert!(
                clients.faucet.do_balance().await.orchard_balance > Some(0)
                    || clients.faucet.do_balance().await.transparent_balance > Some(0),
                "No mining reward recieved from {}. Faucet Orchard Balance: {:?}. Faucet Transparent Balance: {:?}.",
                validator,
                clients.faucet.do_balance().await.orchard_balance, 
                clients.faucet.do_balance().await.transparent_balance
            );
            // Shield funds if coming from zebra.
            if clients.faucet.quick_shield().await.is_ok() {
                test_manager.local_net.generate_blocks(1).await.unwrap();
                clients.faucet.do_sync(true).await.unwrap();
            };
            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("unified").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();
            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.recipient.do_sync(true).await.unwrap();
            assert_eq!(
                clients.recipient.do_balance().await.orchard_balance,
                Some(250_000)
            );
            test_manager.close().await;
        }
    }
}
