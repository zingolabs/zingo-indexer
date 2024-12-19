//! Holds wallet-to-validator tests for Zaino.

#![forbid(unsafe_code)]

use std::sync::Arc;
use zaino_testutils::TestManager;
use zcash_local_net::validator::Validator;
use zingolib::testutils::lightclient::from_inputs;

mod wallet_basic {
    use super::*;

    #[tokio::test]
    async fn zcashd_connect_to_node_get_info() {
        connect_to_node_get_info("zcashd").await;
    }

    #[tokio::test]
    async fn zebrad_connect_to_node_get_info() {
        connect_to_node_get_info("zebrad").await;
    }

    async fn connect_to_node_get_info(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, true, true)
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
    async fn zcashd_send_to_orchard() {
        send_to_orchard("zcashd").await;
    }

    #[tokio::test]
    async fn zebrad_send_to_orchard() {
        send_to_orchard("zebrad").await;
    }

    async fn send_to_orchard(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        clients.faucet.do_sync(true).await.unwrap();

        if validator == "zebrad" {
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
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

    #[tokio::test]
    async fn zcashd_send_to_sapling() {
        send_to_sapling("zcashd").await;
    }

    #[tokio::test]
    async fn zebrad_send_to_sapling() {
        send_to_sapling("zebrad").await;
    }

    async fn send_to_sapling(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        clients.faucet.do_sync(true).await.unwrap();

        if validator == "zebrad" {
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
        };

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

    #[tokio::test]
    async fn zcashd_send_to_transparent() {
        send_to_transparent("zcashd").await;
    }

    #[tokio::test]
    async fn zebrad_send_to_transparent() {
        send_to_transparent("zebrad").await;
    }

    async fn send_to_transparent(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        clients.faucet.do_sync(true).await.unwrap();

        if validator == "zebrad" {
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
        };

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
        test_manager.local_net.generate_blocks(99).await.unwrap();

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

    #[tokio::test]
    async fn zcashd_send_to_all() {
        send_to_all("zcashd").await;
    }

    #[tokio::test]
    async fn zebrad_send_to_all() {
        send_to_all("zebrad").await;
    }

    async fn send_to_all(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        test_manager.local_net.generate_blocks(2).await.unwrap();
        clients.faucet.do_sync(true).await.unwrap();

        // "Create" 3 orchard notes in faucet.
        if validator == "zebrad" {
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
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

        test_manager.local_net.generate_blocks(100).await.unwrap();
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

    #[tokio::test]
    async fn zcashd_shield() {
        shield("zcashd").await;
    }

    #[tokio::test]
    async fn zebrad_shield() {
        shield("zebrad").await;
    }

    async fn shield(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .as_ref()
            .expect("Clients are not initialized");

        clients.faucet.do_sync(true).await.unwrap();

        if validator == "zebrad" {
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
        };

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

        test_manager.local_net.generate_blocks(100).await.unwrap();
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

    #[tokio::test]
    async fn zcashd_monitor_unverified_mempool() {
        monitor_unverified_mempool("zcashd").await;
    }

    #[tokio::test]
    async fn zebrad_monitor_unverified_mempool() {
        monitor_unverified_mempool("zebrad").await;
    }

    async fn monitor_unverified_mempool(validator: &str) {
        let mut test_manager = TestManager::launch(validator, None, true, true)
            .await
            .unwrap();
        let clients = test_manager
            .clients
            .take()
            .expect("Clients are not initialized");
        let recipient_client = Arc::new(clients.recipient);

        test_manager.local_net.generate_blocks(1).await.unwrap();
        clients.faucet.do_sync(true).await.unwrap();

        if validator == "zebrad" {
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(100).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
            clients.faucet.quick_shield().await.unwrap();
            test_manager.local_net.generate_blocks(1).await.unwrap();
            clients.faucet.do_sync(true).await.unwrap();
        };

        from_inputs::quick_send(
            &clients.faucet,
            vec![(
                &zingolib::get_base_address_macro!(recipient_client, "unified"),
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
        let _ = zingolib::lightclient::LightClient::start_mempool_monitor(recipient_client.clone())
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        test_manager.local_net.print_stdout();

        assert_eq!(
            recipient_client
                .do_balance()
                .await
                .unverified_orchard_balance
                .unwrap(),
            250_000
        );
        assert_eq!(
            recipient_client
                .do_balance()
                .await
                .unverified_sapling_balance
                .unwrap(),
            250_000
        );

        test_manager.local_net.generate_blocks(1).await.unwrap();
        recipient_client.do_rescan().await.unwrap();

        assert_eq!(
            recipient_client
                .do_balance()
                .await
                .verified_orchard_balance
                .unwrap(),
            250_000
        );
        assert_eq!(
            recipient_client
                .do_balance()
                .await
                .verified_sapling_balance
                .unwrap(),
            250_000
        );

        test_manager.close().await;
    }
}
