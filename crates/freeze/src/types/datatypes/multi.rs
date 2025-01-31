use crate::types::Datatype;

/// enum of possible sets of datatypes that cryo can collect
/// used when multiple datatypes are collected together
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum MultiDatatype {
    /// blocks and transactions
    BlocksAndTransactions,

    /// call trace derivatives
    CallTraceDerivatives,

    /// balance diffs, code diffs, nonce diffs, and storage diffs
    StateDiffs,
}

impl MultiDatatype {
    /// individual datatypes
    pub fn datatypes(&self) -> Vec<Datatype> {
        match &self {
            MultiDatatype::BlocksAndTransactions => vec![Datatype::Blocks, Datatype::Transactions],
            MultiDatatype::StateDiffs => vec![
                Datatype::BalanceDiffs,
                Datatype::CodeDiffs,
                Datatype::NonceDiffs,
                Datatype::StorageDiffs,
            ],
            MultiDatatype::CallTraceDerivatives => {
                vec![Datatype::Contracts, Datatype::NativeTransfers, Datatype::Traces]
            }
        }
    }

    /// return all variants of multi datatype
    pub fn variants() -> Vec<MultiDatatype> {
        vec![
            MultiDatatype::BlocksAndTransactions,
            MultiDatatype::CallTraceDerivatives,
            MultiDatatype::StateDiffs,
        ]
    }
}
