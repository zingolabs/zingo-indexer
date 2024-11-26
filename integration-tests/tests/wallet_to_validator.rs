//! Holds wallet-to-validator tests for Zaino.

#![forbid(unsafe_code)]

use std::sync::Arc;
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
    async fn send_to_orchard() {
        for validator in ["zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .as_ref()
                .expect("Clients are not initialized");

            clients.faucet.do_sync(true).await.unwrap();

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
                clients
                    .recipient
                    .do_balance()
                    .await
                    .orchard_balance
                    .unwrap(),
                250_000
            );

            test_manager.close().await;
        }
    }

    #[tokio::test]
    async fn send_to_sapling() {
        for validator in ["zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .as_ref()
                .expect("Clients are not initialized");

            clients.faucet.do_sync(true).await.unwrap();

            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("sapling").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();
            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.recipient.do_sync(true).await.unwrap();

            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .sapling_balance
                    .unwrap(),
                250_000
            );

            test_manager.close().await;
        }
    }

    #[tokio::test]
    async fn send_to_transparent() {
        for validator in ["zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .as_ref()
                .expect("Clients are not initialized");

            clients.faucet.do_sync(true).await.unwrap();

            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("transparent").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();

            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.recipient.do_sync(true).await.unwrap();

            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .transparent_balance
                    .unwrap(),
                250_000
            );

            test_manager.close().await;
        }
    }

    #[tokio::test]
    async fn send_to_all() {
        for validator in ["zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .as_ref()
                .expect("Clients are not initialized");

            test_manager.local_net.generate_blocks(2).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();

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
            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("sapling").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();
            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("transparent").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();

            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.recipient.do_sync(true).await.unwrap();

            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .orchard_balance
                    .unwrap(),
                250_000
            );
            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .sapling_balance
                    .unwrap(),
                250_000
            );
            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .transparent_balance
                    .unwrap(),
                250_000
            );

            test_manager.close().await;
        }
    }

    #[tokio::test]
    async fn shield() {
        for validator in ["zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .as_ref()
                .expect("Clients are not initialized");

            clients.faucet.do_sync(true).await.unwrap();

            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("transparent").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();

            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.recipient.do_sync(true).await.unwrap();

            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .transparent_balance
                    .unwrap(),
                250_000
            );

            clients.recipient.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.recipient.do_sync(true).await.unwrap();

            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .orchard_balance
                    .unwrap(),
                235_000
            );

            test_manager.close().await;
        }
    }

    #[tokio::test]
    async fn sync_full_batch() {
        for validator in ["zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .as_ref()
                .expect("Clients are not initialized");

            test_manager.local_net.generate_blocks(2).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();

            test_manager.local_net.generate_blocks(5).await.unwrap();
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
            test_manager.local_net.generate_blocks(15).await.unwrap();
            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("sapling").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();
            test_manager.local_net.generate_blocks(15).await.unwrap();
            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &clients.get_recipient_address("transparent").await,
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();
            test_manager.local_net.generate_blocks(70).await.unwrap();

            clients.recipient.do_sync(true).await.unwrap();

            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .orchard_balance
                    .unwrap(),
                250_000
            );
            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .sapling_balance
                    .unwrap(),
                250_000
            );
            assert_eq!(
                clients
                    .recipient
                    .do_balance()
                    .await
                    .transparent_balance
                    .unwrap(),
                250_000
            );

            test_manager.close().await;
        }
    }

    #[tokio::test]
    async fn monitor_unverified_mempool() {
        for validator in ["zcashd"] {
            let mut test_manager = TestManager2::launch(validator, None, true, true)
                .await
                .unwrap();
            let clients = test_manager
                .clients
                .take()
                .expect("Clients are not initialized");
            let recipient_client = Arc::new(clients.recipient);

            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();

            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &zingolib::get_base_address_macro!(recipient_client, "sapling"),
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();
            from_inputs::quick_send(
                &clients.faucet,
                vec![(
                    &zingolib::get_base_address_macro!(recipient_client, "sapling"),
                    250_000,
                    None,
                )],
            )
            .await
            .unwrap();

            recipient_client.clear_state().await;
            zingolib::lightclient::LightClient::start_mempool_monitor(recipient_client.clone());
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            assert_eq!(
                recipient_client
                    .do_balance()
                    .await
                    .unverified_sapling_balance
                    .unwrap(),
                500_000
            );

            test_manager.local_net.generate_blocks(1).await.unwrap();
            recipient_client.do_rescan().await.unwrap();

            assert_eq!(
                recipient_client
                    .do_balance()
                    .await
                    .verified_sapling_balance
                    .unwrap(),
                500_000
            );

            test_manager.close().await;
        }
    }
}
