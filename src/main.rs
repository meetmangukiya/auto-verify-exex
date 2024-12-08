use alloy_consensus::BlockBody;
use alloy_primitives::{Address, Bytes, FixedBytes, TxKind, address};
use futures::{Future, TryStreamExt};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_primitives::{EthPrimitives, TransactionSigned};
use reth_tracing::tracing::info;

mod matcher;
use matcher::Matcher;

/// The initialization logic of the ExEx is just an async function.
///
/// During initialization you can wait for resources you need to be up for the ExEx to function,
/// like a database connection.
async fn exex_init<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    let matcher = Matcher::new(vec![(
        address!("0000000000000000000000000000000000000000"),
        vec![(
            [0xc9, 0xc6, 0x53, 0x96],
            Bytes::from(&[0x00, 0x00, 0x00, 0x00]),
            "(address,address)".into(),
        )],
    )]);
    Ok(exex(ctx, matcher))
}

/// An ExEx is just a future, which means you can implement all of it in an async function!
///
/// This ExEx just prints out whenever either a new chain of blocks being added, or a chain of
/// blocks being re-orged. After processing the chain, emits an [ExExEvent::FinishedHeight] event.
async fn exex<Node: FullNodeComponents>(
    mut ctx: ExExContext<Node>,
    matcher: Matcher,
) -> eyre::Result<()> {
    while let Some(notification) = ctx.notifications.try_next().await? {
        match &notification {
            ExExNotification::ChainCommitted { new } => {
                info!(committed_chain = ?new.range(), "Received commit");
            }
            ExExNotification::ChainReorged { old, new } => {
                info!(from_chain = ?old.range(), to_chain = ?new.range(), "Received reorg");
            }
            ExExNotification::ChainReverted { old } => {
                info!(reverted_chain = ?old.range(), "Received revert");
            }
        };

        if let Some(committed_chain) = notification.committed_chain() {
            for block_receipts in committed_chain.receipts_with_attachment() {
                for (tx_hash, tx_receipt) in block_receipts.tx_receipts {}
            }

            for (block, receipts) in committed_chain.blocks_and_receipts() {
                for tx in &block.block.body.transactions {
                    if let Some((tx_to, tx_input)) = match tx.transaction {
                        reth_primitives::Transaction::Legacy(tx) => Some((tx.to, tx.input.clone())),
                        reth_primitives::Transaction::Eip2930(tx_eip2930) => {
                            Some((tx_eip2930.to, tx_eip2930.input.clone()))
                        }
                        reth_primitives::Transaction::Eip1559(tx_eip1559) => {
                            Some((tx_eip1559.to, tx_eip1559.input.clone()))
                        }
                        reth_primitives::Transaction::Eip4844(tx_eip4844) => None,
                        reth_primitives::Transaction::Eip7702(tx_eip7702) => {
                            Some((TxKind::Call(tx_eip7702.to), tx_eip7702.input.clone()))
                        }
                    } {
                        if let TxKind::Call(tx_to) = tx_to {
                            if let Some(decoded) = matcher.get_constructor_args(tx_to, &tx_input) {
                                println!("{:?}", decoded);
                                // TODO: verify the contract on etherscan
                            }
                        }
                    }
                }
            }

            ctx.events
                .send(ExExEvent::FinishedHeight(committed_chain.tip().num_hash()))?;
        }
    }

    Ok(())
}

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(EthereumNode::default())
            .install_exex("auto-verify-exex", exex_init)
            .launch()
            .await?;

        handle.wait_for_node_exit().await
    })
}

#[cfg(test)]
mod tests {
    use reth_execution_types::{Chain, ExecutionOutcome};
    use reth_exex_test_utils::{PollOnce, test_exex_context};
    use std::pin::pin;

    #[tokio::test]
    async fn test_exex() -> eyre::Result<()> {
        // Initialize a test Execution Extension context with all dependencies
        let (ctx, mut handle) = test_exex_context().await?;

        // Save the current head of the chain to check the finished height against it later
        let head = ctx.head;

        // Send a notification to the Execution Extension that the chain has been committed
        handle
            .send_notification_chain_committed(Chain::from_block(
                handle.genesis.clone(),
                ExecutionOutcome::default(),
                None,
            ))
            .await?;

        // Initialize the Execution Extension
        let mut exex = pin!(super::exex_init(ctx).await?);

        // Check that the Execution Extension did not emit any events until we polled it
        handle.assert_events_empty();

        // Poll the Execution Extension once to process incoming notifications
        exex.poll_once().await?;

        // Check that the Execution Extension emitted a `FinishedHeight` event with the correct
        // height
        handle.assert_event_finished_height((head.number, head.hash).into())?;

        Ok(())
    }
}
