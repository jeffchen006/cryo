use crate::*;
use ethers::prelude::*;
use polars::prelude::*;
use std::collections::HashMap;

/// columns for balances
#[cryo_to_df::to_df(Datatype::Storages)]
#[derive(Default)]
pub struct Storages {
    n_rows: usize,
    block_number: Vec<u32>,
    address: Vec<Vec<u8>>,
    slot: Vec<Vec<u8>>,
    value: Vec<Vec<u8>>,
    chain_id: Vec<u64>,
}

#[async_trait::async_trait]
impl Dataset for Storages {
    fn name() -> &'static str {
        "storages"
    }

    fn default_sort() -> Vec<String> {
        vec!["block_number".to_string(), "address".to_string(), "slot".to_string()]
    }

    fn required_parameters() -> Vec<Dim> {
        vec![Dim::Address, Dim::Slot]
    }
}

type Result<T> = ::core::result::Result<T, CollectError>;
type BlockTxAddressOutput = (u32, Option<Vec<u8>>, Vec<u8>, Vec<u8>, Vec<u8>);

#[async_trait::async_trait]
impl CollectByBlock for Storages {
    type Response = BlockTxAddressOutput;

    async fn extract(
        request: Params,
        source: Arc<Source>,
        _schemas: Schemas,
    ) -> Result<Self::Response> {
        let address = request.address()?;
        let block_number = request.block_number()? as u32;
        let slot = request.slot()?;
        let output = source
            .fetcher
            .get_storage_at(
                H160::from_slice(&address),
                H256::from_slice(&slot),
                block_number.into(),
            )
            .await?;
        Ok((block_number, None, address, slot, output.as_bytes().to_vec()))
    }

    fn transform(response: Self::Response, columns: &mut Self, schemas: &Schemas) -> Result<()> {
        let schema = schemas.get(&Datatype::Storages).ok_or(err("schema not provided"))?;
        process_nonce(columns, response, schema)
    }
}

#[async_trait::async_trait]
impl CollectByTransaction for Storages {
    type Response = ();
}

fn process_nonce(columns: &mut Storages, data: BlockTxAddressOutput, schema: &Table) -> Result<()> {
    let (block, _tx, address, slot, output) = data;
    columns.n_rows += 1;
    store!(schema, columns, block_number, block);
    store!(schema, columns, address, address);
    store!(schema, columns, slot, slot);
    store!(schema, columns, value, output);
    Ok(())
}
