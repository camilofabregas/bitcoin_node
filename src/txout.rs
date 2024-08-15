use crate::{account::Account, compactsize::CompactSize, errors::RustifyError};

#[derive(Debug, Clone, PartialEq)]
pub struct TxOut {
    pub value_amount_satoshis: i64,
    pub pk_script_bytes: CompactSize,
    pub pk_script: Vec<u8>,
}

impl TxOut {
    pub fn new(receptor: &Account, amount: f64) -> TxOut {
        let pk_script = receptor.obtain_pk_script();
        let pk_script_bytes = CompactSize::new(pk_script.len() as u64);
        TxOut {
            value_amount_satoshis: (amount * 100000000.0) as i64,
            pk_script_bytes,
            pk_script,
        }
    }

    pub fn from_bytes(
        raw_transaction_bytes: Vec<u8>,
        mut index: usize,
    ) -> Result<(TxOut, usize), RustifyError> {
        let value_amount_satoshis =
            i64::from_le_bytes(raw_transaction_bytes[index..index + 8].try_into()?);
        index += 8;

        let (pk_script_bytes, csize_index) =
            CompactSize::parse_from_byte_array(&raw_transaction_bytes[index..index + 10]);
        index += csize_index;

        let updated_index_pk_script = pk_script_bytes.value() as usize;
        let pk_script: Vec<u8> =
            raw_transaction_bytes[index..index + updated_index_pk_script].to_vec();
        index += updated_index_pk_script;

        Ok((
            TxOut {
                value_amount_satoshis,
                pk_script_bytes,
                pk_script,
            },
            index,
        ))
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes_transaction: Vec<u8> = vec![];
        bytes_transaction.append(&mut self.value_amount_satoshis.to_le_bytes().to_vec());
        bytes_transaction.append(&mut self.pk_script_bytes.as_bytes());
        bytes_transaction.append(&mut self.pk_script.clone());
        bytes_transaction
    }
}
