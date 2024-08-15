use std::{collections::HashMap, vec};

use bitcoin_hashes::{sha256d, Hash};

use crate::{
    account::Account, compactsize::CompactSize, errors::RustifyError, locktime::LockTime,
    txin::TxIn, txout::TxOut,
};

type TrxKey = (String, u32);
type TrxHashMap<T> = HashMap<TrxKey, T>;

#[derive(Debug, Clone, PartialEq)]
pub struct Txn {
    pub version: i32,
    pub tx_in_count: CompactSize,
    pub tx_in: Vec<TxIn>,
    pub tx_out_count: CompactSize,
    pub tx_out: Vec<TxOut>,
    pub tx_lock_time: u32,
}

impl Txn {
    /// Realiza la transacción, dados unos utxos para asociar en el input
    /// y dado un vuelto, un emisor y un receptor
    pub fn new(
        emisor: &Account,
        receptor: Account,
        importe: f64,
        vuelto: f64,
        input_utxos: &TrxHashMap<Txn>,
    ) -> Result<Txn, RustifyError> {
        let mut tx_in: Vec<TxIn> = vec![];
        for trxkey in input_utxos.keys() {
            tx_in.push(TxIn::new(trxkey, emisor.obtain_pk_script()));
        }

        let mut tx_out: Vec<TxOut> = vec![];
        if vuelto > 0f64 {
            tx_out.push(TxOut::new(&receptor, importe));
            tx_out.push(TxOut::new(emisor, vuelto));
        } else {
            tx_out.push(TxOut::new(&receptor, importe));
        }

        Ok(Txn {
            version: 1,
            tx_in_count: CompactSize::new(tx_in.len() as u64),
            tx_in,
            tx_out_count: CompactSize::new(tx_out.len() as u64),
            tx_out,
            tx_lock_time: LockTime::create(),
        })
    }

    /// Obtiene la transaccion parseada, en base a una cadena de bytes recibida
    pub fn from_bytes(
        raw_transaction_bytes: Vec<u8>,
        mut index: usize,
    ) -> Result<(Txn, usize), RustifyError> {
        let version = i32::from_le_bytes(raw_transaction_bytes[index..index + 4].try_into()?);
        index += 4;
        let (tx_in_count, csize_index) =
            CompactSize::parse_from_byte_array(&raw_transaction_bytes[index..index + 10]);
        index += csize_index;
        let mut tx_in: Vec<TxIn> = vec![];
        let mut transaction_input: TxIn;

        for _i in 0..tx_in_count.value() {
            (transaction_input, index) = TxIn::from_bytes(raw_transaction_bytes.clone(), index)?;
            tx_in.push(transaction_input);
        }

        let (tx_out_count, csize_index) =
            CompactSize::parse_from_byte_array(&raw_transaction_bytes[index..index + 10]);

        index += csize_index;

        let mut tx_out: Vec<TxOut> = vec![];
        let mut transaction_ouput: TxOut;

        for _i in 0..tx_out_count.value() {
            (transaction_ouput, index) = TxOut::from_bytes(raw_transaction_bytes.clone(), index)?;
            tx_out.push(transaction_ouput);
        }

        let tx_lock_time = LockTime::from_bytes(raw_transaction_bytes[index..index + 4].to_vec());
        index += 4;

        Ok((
            Txn {
                version,
                tx_in_count,
                tx_in,
                tx_out_count,
                tx_out,
                tx_lock_time: tx_lock_time.value,
            },
            index,
        ))
    }

    /// Obtiene el TXID de la transaccion, en tipo String
    /// permitiendo que sea facil buscarlo en la blockchain
    pub fn obtain_tx_id(buffer: Vec<u8>) -> String {
        sha256d::Hash::hash(&buffer).to_string()
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes_transaction: Vec<u8> = vec![];
        bytes_transaction.append(&mut self.version.to_le_bytes().to_vec());

        bytes_transaction.append(&mut self.tx_in_count.as_bytes());
        for index in 0..self.tx_in_count.value() {
            bytes_transaction.append(&mut self.tx_in[index as usize].as_bytes());
        }

        bytes_transaction.append(&mut self.tx_out_count.as_bytes());
        for index in 0..self.tx_out_count.value() {
            bytes_transaction.append(&mut self.tx_out[index as usize].as_bytes());
        }

        bytes_transaction.append(&mut self.tx_lock_time.to_le_bytes().to_vec());

        bytes_transaction
    }

    /// Obtiene el TXID de la transaccion, en tipo String,
    /// desde un inventario (mensajes Inv)
    pub fn obtain_txid_from_inventory(mut inventory: Vec<u8>) -> String {
        let hex_chars: Vec<char> = "0123456789abcdef".chars().collect();
        let mut hex_string = String::with_capacity(inventory.len() * 2);
        inventory.reverse();

        for byte in inventory {
            hex_string.push(hex_chars[(byte >> 4) as usize]);
            hex_string.push(hex_chars[(byte & 0xF) as usize]);
        }

        hex_string
    }
}

#[cfg(test)]
mod tests {

    use super::Txn;
    #[test]
    fn test_obtain_txid() {
        let raw_txn = "020000000181ebdb2d1140794034dff51b184c9e0ffd51bc9644be5cdd750d0173888c30ff0100000000fdffffff0217751000000000001976a914a7165cba93aeec181da155e04680d3bf84f960cb88aca219719d000000001976a914bdd785fe75fb2ead304f5e66adf05af8b9fcc1a388ac5a3f2500";
        let txn_vec = (0..raw_txn.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw_txn[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .unwrap();
        let tx_id = Txn::obtain_tx_id(txn_vec.clone());
        assert_eq!(
            tx_id,
            "dc717cad242917f8caaceceae3ed0cba6fd7ced5285efe05661d919507618845"
        )
    }

    #[test]
    fn test_obtain_txid_from_inventory() {
        let vect: Vec<u8> = vec![
            57, 142, 17, 235, 132, 133, 100, 71, 150, 159, 7, 224, 14, 186, 121, 233, 167, 183,
            206, 108, 38, 188, 161, 96, 26, 11, 49, 159, 112, 222, 79, 68,
        ];
        let txid = Txn::obtain_txid_from_inventory(vect);

        assert_eq!(
            txid,
            "444fde709f310b1a60a1bc266cceb7a7e979ba0ee0079f9647648584eb118e39".to_owned()
        )
    }
}
