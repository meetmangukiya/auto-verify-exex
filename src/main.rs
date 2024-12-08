use alloy_primitives::{Bytes, address, hex};
use foundry_block_explorers::verify::CodeFormat;
use futures::{Future, TryStreamExt};
use reth::rpc::types::TransactionTrait;
use reth_chainspec::Chain;
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_tracing::tracing::info;

mod matcher;
use matcher::{ContractDeployArgs, Matcher};

/// The initialization logic of the ExEx is just an async function.
///
/// During initialization you can wait for resources you need to be up for the ExEx to function,
/// like a database connection.
async fn exex_init<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    let matcher = Matcher::new(vec![(
        address!("0000000000000000000000000000000000000000"),
        vec![ContractDeployArgs {
            factory_selector: [0x00, 0x00, 0x00, 0x00],
            init_code_without_args: Bytes::from(&hex!("00")),
            source: todo!(),
            code_format: CodeFormat::SingleFile,
            param_types: "(address,address)".to_string(),
            contract_name: "UniswapV2Pair".to_string(),
            compiler_version: "".to_string(),
            optimizations_used: None,
            runs: None,
            evm_version: None,
            via_ir: None,
        }],
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
    let client = foundry_block_explorers::Client::new(
        Chain::mainnet(),
        std::env::var("ETHERSCAN_API_KEY")?,
    )?;

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
            for block in committed_chain.blocks_iter() {
                // for tx in &block.block.body.transactions {
                for tx in block.transactions() {
                    let tx_input = tx.input();
                    let to = tx.to();

                    if let Some(to) = to {
                        if let Some(verify_args) = matcher.get_verification_args(to, tx_input) {
                            println!("trying to verify contract {:?}", verify_args);
                            let response =
                                client.submit_contract_verification(&verify_args).await?;
                            println!("contract verified successfully: {:?}", response);
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
