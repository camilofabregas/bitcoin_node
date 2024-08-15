use crate::errors::RustifyError;
use crate::txn::Txn;
use crate::{block_header::BlockHeader, compactsize::CompactSize};
use bitcoin_hashes::{sha256d, Hash};

#[derive(Debug, Clone)]
pub struct SerializedBlock {
    pub block_header: BlockHeader,
    pub txn_count: CompactSize,
    pub txns: Vec<Txn>,
}
impl SerializedBlock {
    /// Obtiene un Bloque serializado usando de base la tira de bytes (sin el header)
    /// del mensaje de tipo "Block"
    pub fn from_bytes(block_bytes: &[u8]) -> Result<SerializedBlock, RustifyError> {
        let mut index = 0;
        let block_header = BlockHeader::from_bytes(&block_bytes[index..index + 80])?;
        index += 80;
        let (txn_count, csize_bytes) = CompactSize::parse_from_byte_array(&block_bytes[80..90]);

        index += csize_bytes;

        let mut txns: Vec<Txn> = vec![];
        let mut transaction: Txn;

        for _i in 0..txn_count.value() {
            (transaction, index) = Txn::from_bytes(block_bytes.to_owned(), index)?;

            txns.push(transaction);
        }

        if block_bytes[index - 1..].to_vec().len() != 1 {
            Err(RustifyError::ErrorAlParsearBloque)
        } else {
            Ok(SerializedBlock {
                block_header,
                txn_count,
                txns,
            })
        }
    }

    /// Obtiene el nombre del archivo utilizando el hash del bloque
    pub fn obtain_name_for_blockfile(bytes_block: &[u8]) -> String {
        sha256d::Hash::hash(&bytes_block[0..80]).to_string()
    }
    /// Obtiene el hash del bloque
    pub fn obtain_blockhash(header_bytes: [u8; 80]) -> String {
        sha256d::Hash::hash(&header_bytes).to_string()
    }
    /// En base a un inventario de bloque, se obtiene el hash del bloque
    pub fn obtain_blockname_from_blockhash(possible_block: Vec<u8>) -> String {
        let mut hex_blockhash: Vec<String> = possible_block
            .iter()
            .map(|byte| format!("{:02X}", byte))
            .collect();
        hex_blockhash.reverse();
        hex_blockhash.join("").to_lowercase()
    }
}

#[cfg(test)]
mod tests {

    use crate::{block_header::BlockHeader, serialized_block::SerializedBlock};

    #[test]
    fn test_obtain_blockhash() {
        let header: BlockHeader = BlockHeader {
            version: 545259520,
            previous_block_header_hash: [
                138, 99, 26, 88, 188, 120, 217, 186, 234, 37, 95, 183, 123, 88, 215, 34, 31, 183,
                102, 128, 209, 79, 186, 218, 21, 0, 0, 0, 0, 0, 0, 0,
            ],
            merkle_root_hash: [
                66, 177, 227, 124, 21, 190, 133, 88, 89, 199, 238, 143, 8, 65, 188, 64, 12, 166,
                115, 116, 128, 103, 34, 142, 163, 174, 70, 137, 126, 20, 68, 216,
            ],
            time: 1689685467,
            n_bits: 486604799,
            nonce: 2144752951,
        };
        assert_eq!(
            "0000000000002fb0badf9930d76ac097afb344ccc9d39d8b7807a3d8ae32d66d",
            SerializedBlock::obtain_blockhash(header.as_bytes())
        );
    }

    #[test]
    fn obtain_blockname_from_blockhash_test() {
        let inventory_blockhash = vec![
            55, 202, 162, 39, 41, 32, 92, 245, 75, 112, 161, 143, 14, 28, 84, 45, 133, 13, 48, 171,
            181, 193, 208, 240, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(
            "0000000000000006f0d0c1b5ab300d852d541c0e8fa1704bf55c202927a2ca37",
            SerializedBlock::obtain_blockname_from_blockhash(inventory_blockhash)
        );
    }
}
