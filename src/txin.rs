use crate::{compactsize::CompactSize, errors::RustifyError, outpoint::OutPoint};

type TrxKey = (String, u32);

#[derive(Debug, Clone, PartialEq)]
pub struct TxIn {
    pub previous_output: OutPoint,
    pub script_bytes: CompactSize,
    pub signature_script: Vec<u8>,
    pub sequence: u32,
}

impl TxIn {
    ///En el new de TxIn no se firma, eso se realiza una vez creada toda la Txn.
    pub fn new(trxkey: &TrxKey, pk_script: Vec<u8>) -> TxIn {
        TxIn {
            previous_output: OutPoint::new(&trxkey.0, &trxkey.1),
            script_bytes: CompactSize::new(pk_script.len() as u64),
            signature_script: pk_script,
            sequence: 0xffffffff,
        }
    }

    pub fn from_bytes(
        raw_transaction_bytes: Vec<u8>,
        mut index: usize,
    ) -> Result<(TxIn, usize), RustifyError> {
        let previous_output =
            OutPoint::from_bytes(raw_transaction_bytes[index..index + 36].to_vec());

        index += 36;

        let (script_bytes, csize_index) =
            CompactSize::parse_from_byte_array(&raw_transaction_bytes[index..index + 10]);
        index += csize_index;
        let updated_index_sig_script = script_bytes.value() as usize;
        let signature_script: Vec<u8> =
            raw_transaction_bytes[index..index + updated_index_sig_script].to_vec();

        index += updated_index_sig_script;

        let sequence = u32::from_le_bytes(raw_transaction_bytes[index..index + 4].try_into()?);

        index += 4;

        Ok((
            TxIn {
                previous_output,
                script_bytes,
                signature_script,
                sequence,
            },
            index,
        ))
    }

    /// Obtiene el txid y el outpoint index para ubicar el output
    /// al cual esta enlazado este input
    pub fn obtain_tx_id_of_previous_output(&self) -> (String, u32) {
        let mut hex_txid: Vec<String> = self
            .previous_output
            .hash_previous_output_txid
            .iter()
            .map(|byte| format!("{:02X}", byte))
            .collect();
        hex_txid.reverse();
        (
            hex_txid.join("").to_lowercase(),
            self.previous_output.output_index,
        )
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes_transaction: Vec<u8> = vec![];

        bytes_transaction.append(&mut self.previous_output.as_bytes());
        bytes_transaction.append(&mut self.script_bytes.as_bytes());
        bytes_transaction.append(&mut self.signature_script.clone());
        bytes_transaction.append(&mut self.sequence.to_le_bytes().to_vec());
        bytes_transaction
    }
}

#[cfg(test)]
mod tests {
    use crate::{errors::RustifyError, txn::Txn};

    #[test]
    fn test_obtain_tx_id_of_previous_output() -> Result<(), RustifyError> {
        let txn: [u8; 351] = [
            1, 0, 0, 0, 1, 13, 186, 123, 196, 34, 135, 43, 162, 171, 213, 167, 43, 250, 149, 91,
            142, 65, 107, 107, 130, 206, 84, 184, 251, 220, 21, 86, 93, 37, 154, 86, 69, 3, 0, 0,
            0, 106, 71, 48, 68, 2, 32, 86, 180, 115, 42, 83, 159, 225, 122, 57, 100, 122, 139, 98,
            121, 103, 197, 130, 223, 114, 63, 82, 64, 70, 13, 148, 134, 19, 218, 243, 18, 99, 116,
            2, 32, 71, 17, 63, 213, 152, 119, 90, 182, 163, 147, 152, 225, 204, 223, 20, 252, 97,
            104, 118, 34, 135, 211, 17, 160, 2, 123, 83, 122, 111, 4, 50, 149, 1, 33, 2, 116, 122,
            46, 182, 33, 219, 139, 114, 246, 175, 11, 79, 106, 45, 126, 84, 105, 189, 116, 241,
            117, 237, 221, 251, 109, 138, 109, 92, 108, 165, 118, 214, 253, 255, 255, 255, 4, 0, 0,
            0, 0, 0, 0, 0, 0, 83, 106, 76, 80, 84, 50, 91, 218, 41, 25, 227, 142, 167, 36, 89, 14,
            177, 151, 126, 245, 10, 16, 140, 183, 69, 180, 113, 151, 21, 80, 165, 14, 123, 188,
            166, 90, 251, 184, 61, 68, 10, 111, 50, 226, 168, 6, 82, 185, 122, 138, 0, 119, 170,
            125, 40, 149, 235, 254, 162, 109, 38, 186, 159, 128, 214, 42, 142, 68, 16, 130, 130, 0,
            37, 22, 174, 0, 3, 0, 37, 14, 118, 0, 28, 52, 244, 1, 0, 0, 0, 0, 0, 0, 25, 118, 169,
            20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 136, 172, 244, 1, 0, 0,
            0, 0, 0, 0, 25, 118, 169, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 136, 172, 80, 63, 155, 6, 0, 0, 0, 0, 25, 118, 169, 20, 139, 139, 162, 224, 107,
            210, 202, 219, 218, 248, 230, 234, 6, 195, 97, 199, 250, 52, 28, 55, 136, 172, 0, 0, 0,
            0,
        ];
        let parsed_transaction = Txn::from_bytes(txn.to_vec(), 0)?;
        assert_eq!(
            parsed_transaction.0.tx_in[0]
                .obtain_tx_id_of_previous_output()
                .0,
            "45569a255d5615dcfbb854ce826b6b418e5b95fa2ba7d5aba22b8722c47bba0d"
        );
        Ok(())
    }
}
