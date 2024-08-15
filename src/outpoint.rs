#[derive(Debug, Clone, PartialEq)]
pub struct OutPoint {
    pub hash_previous_output_txid: [u8; 32],
    pub output_index: u32,
}

impl OutPoint {
    pub fn new(txid: &str, output_index: &u32) -> OutPoint {
        OutPoint {
            hash_previous_output_txid: obtain_txid_from_str(txid),
            output_index: *output_index,
        }
    }

    pub fn from_bytes(outpoint_bytes: Vec<u8>) -> OutPoint {
        let mut hash_txid = [0u8; 32];
        hash_txid.copy_from_slice(&outpoint_bytes[0..32]);
        let mut array_outpoint: [u8; 4] = [0u8; 4];
        array_outpoint.copy_from_slice(&outpoint_bytes[32..36]);
        let index = u32::from_le_bytes(array_outpoint);

        OutPoint {
            hash_previous_output_txid: hash_txid,
            output_index: index,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes_transaction: Vec<u8> = vec![];
        bytes_transaction.append(&mut self.hash_previous_output_txid.to_vec());
        bytes_transaction.append(&mut self.output_index.to_le_bytes().to_vec());
        bytes_transaction
    }
}

/// Obtiene TXID a partir de un string dado, que ya se sabe que es un txid
pub fn obtain_txid_from_str(txid: &str) -> [u8; 32] {
    let hex_txid = txid.chars().collect::<Vec<_>>();
    let hex_txid_bytes = hex_txid
        .chunks(2)
        .map(|chunk| {
            let hex_byte: String = chunk.iter().collect();
            u8::from_str_radix(&hex_byte, 16).unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let mut hash_previous_output_txid = [0u8; 32];
    let hex_bytes_len = hex_txid_bytes.len();
    hash_previous_output_txid[..hex_bytes_len].clone_from_slice(&hex_txid_bytes);
    hash_previous_output_txid.reverse();
    hash_previous_output_txid
}

#[cfg(test)]
mod tests {
    use crate::outpoint::obtain_txid_from_str;

    #[test]
    fn test_obtain_txid_from_str() {
        let txid_array: [u8; 32] = [
            160, 151, 218, 122, 156, 40, 141, 128, 85, 9, 204, 191, 131, 10, 201, 252, 20, 159, 54,
            81, 227, 36, 210, 62, 2, 241, 79, 179, 42, 153, 210, 89,
        ];
        assert_eq!(
            obtain_txid_from_str(
                "59d2992ab34ff1023ed224e351369f14fcc90a83bfcc0955808d289c7ada97a0"
            ),
            txid_array
        );
    }
}
