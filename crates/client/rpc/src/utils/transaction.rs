use blockifier::execution::contract_class::ClassInfo;
use blockifier::transaction::transaction_execution as btx;
use jsonrpsee::core::RpcResult;
use mc_db::storage_handler;
use mc_db::storage_handler::primitives::contract_class::StorageContractClassData;
use mc_db::storage_handler::StorageView;
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::{Transaction, TransactionHash};
use starknet_ff::FieldElement;

use crate::errors::StarknetRpcApiError;

pub(crate) fn blockifier_transactions(
    transaction_with_hash: Vec<(Transaction, FieldElement)>,
) -> RpcResult<Vec<btx::Transaction>> {
    let transactions = transaction_with_hash
            .iter()
            .filter(|(tx, _)| !matches!(tx, Transaction::Deploy(_))) // deploy transaction was not supported by blockifier
            .map(|(tx, hash)| to_blockifier_transactions(tx, &TransactionHash(StarkFelt::new_unchecked(hash.to_bytes_be()))))
            .collect::<Result<Vec<_>, _>>()?;

    Ok(transactions)
}

/// Convert an starknet-api Transaction to a blockifier Transaction
///
/// **note:** this function does not support deploy transaction
/// because it is not supported by blockifier
pub(crate) fn to_blockifier_transactions(
    transaction: &Transaction,
    tx_hash: &TransactionHash,
) -> RpcResult<btx::Transaction> {
    let paid_fee_on_l1 = match transaction {
        Transaction::L1Handler(_) => Some(starknet_api::transaction::Fee(1_000_000_000_000)),
        _ => None,
    };

    let class_info = match transaction {
        Transaction::Declare(declare_tx) => {
            let class_hash = declare_tx.class_hash();

            let Ok(Some(class_data)) = storage_handler::contract_class_data().get(&class_hash) else {
                log::error!("Failed to retrieve class from class_hash '{class_hash}'");
                return Err(StarknetRpcApiError::ContractNotFound.into());
            };

            let StorageContractClassData { contract_class, sierra_program_length, abi_length, .. } = class_data;

            Some(ClassInfo::new(&contract_class, sierra_program_length as usize, abi_length as usize).map_err(
                |_| {
                    log::error!("Mismatch between the length of the sierra program and the class version");
                    StarknetRpcApiError::InternalServerError
                },
            )?)
        }
        _ => None,
    };

    btx::Transaction::from_api(transaction.clone(), *tx_hash, class_info, paid_fee_on_l1, None, false).map_err(|_| {
        log::error!("Failed to convert transaction to blockifier transaction");
        StarknetRpcApiError::InternalServerError.into()
    })
}
