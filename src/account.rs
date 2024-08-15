use std::{collections::HashMap, vec};

use bitcoin_hashes::{sha256d, Hash};

use crate::{
    errors::RustifyError,
    script::Script,
    serialized_block::SerializedBlock,
    txn::Txn,
    txn_info::{TxnInfo, TxnType},
    txout::TxOut,
};

const OP_DUP: u8 = 0x76;
const OP_HASH160: u8 = 0xa9;
const OP_EQUALVERIFY: u8 = 0x88;
const OP_CHECKSIG: u8 = 0xac;

type TrxKey = (String, u32);
type TrxHashMap<T> = HashMap<TrxKey, T>;

#[derive(Debug, Clone)]
pub struct Account {
    pub public_address: String,
    pub private_address: String,
    pub balance: f64,
    pub pending_balance: f64,
    pub utxo_transaction: TrxHashMap<Txn>,
    pub sending_txn: Vec<TxnInfo>,
    pub sent_txn: Vec<TxnInfo>,
    pub receiving_txn: Vec<TxnInfo>,
    pub saved_received_txn: Vec<TxnInfo>,
}
impl Account {
    pub fn new(public: String, private: String) -> Account {
        Account {
            public_address: public,
            private_address: private,
            balance: 0.0,
            pending_balance: 0.0,
            utxo_transaction: HashMap::new(),
            sending_txn: vec![],
            sent_txn: vec![],
            receiving_txn: vec![],
            saved_received_txn: vec![],
        }
    }
    pub fn new_str(public: &str, private: &str) -> Account {
        Account {
            public_address: public.to_owned(),
            private_address: private.to_owned(),
            balance: 0.0,
            pending_balance: 0.0,
            utxo_transaction: HashMap::new(),
            sending_txn: vec![],
            sent_txn: vec![],
            receiving_txn: vec![],
            saved_received_txn: vec![],
        }
    }

    /// Obtiene el pubkeyHash del Bitcoin Address
    /// Usos: comparar con la pubkeyHash preexistentes
    /// en los outputs de las UTXOs
    pub fn decode_bitcoin_adress(&self) -> Result<Vec<u8>, RustifyError> {
        let b58 = bs58::decode(self.public_address.clone()).into_vec()?;
        let b58_checksum = &b58[b58.len() - 4..b58.len()];
        let b58_hashversion = &b58[0..b58.len() - 4];
        if b58_checksum != &sha256d::Hash::hash(b58_hashversion)[0..4] {
            Err(RustifyError::ValidacionChecksumB58Invalida)
        } else {
            Ok(b58_hashversion[1..].to_vec())
        }
    }

    pub fn encode_bitcoin_adress(mut pubkey_hash: Vec<u8>) -> String {
        // Aca tengo b58_hashversion[1..]
        let mut v = vec![0x6f];
        v.append(&mut pubkey_hash);
        let hashed = sha256d::Hash::hash(&(v.clone())).to_byte_array();
        let mut checksum = vec![];
        checksum.append(&mut hashed[0..4].to_vec());
        v.append(&mut checksum);
        bs58::encode(v).into_string()
    }

    /// Obtiene el balance de una cuenta (sus utxo) y guarda las transacciones UTXO de la misma
    pub fn obtain_account_balance(&mut self, utxos: &TrxHashMap<Txn>) {
        // Obtener el pubkeyHash de la dirección de Bitcoin
        let pk_hash = match self.decode_bitcoin_adress() {
            Ok(hash) => hash,
            Err(_) => return,
        };

        // Calcular el saldo total
        let mut saldo = 0.0;
        let mut transacciones: TrxHashMap<Txn> = HashMap::new();
        let mut tx_out: &TxOut;

        for (trxkey, txn) in utxos {
            tx_out = &txn.tx_out[trxkey.1 as usize];
            let tx_out_pk_hash = obtain_pubkey_hash(tx_out);
            if tx_out_pk_hash == pk_hash {
                let satoshis = amount_of_satoshis(tx_out);
                saldo += satoshis;
                transacciones.insert(trxkey.clone(), txn.clone());
            }
        }

        self.balance = saldo;
        self.utxo_transaction = transacciones;
    }

    /// En base a la clave publica dada, genera la
    /// clave p2pkh a colocar en el TxOut de las Txn.
    pub fn obtain_pk_script(&self) -> Vec<u8> {
        let mut pk_script = vec![];
        pk_script.push(OP_DUP);
        pk_script.push(OP_HASH160);
        let mut pubkeyhash = self.decode_bitcoin_adress().unwrap_or_default();
        pk_script.push(pubkeyhash.len().try_into().unwrap_or_default());
        pk_script.append(&mut pubkeyhash);
        pk_script.push(OP_EQUALVERIFY);
        pk_script.push(OP_CHECKSIG);

        pk_script
    }

    /// Obtiene el formato Private Key Hexadecimal Format (64 characters [0-9A-F])
    pub fn obtain_hex_privatekey(&self) -> String {
        let wif = self.private_address.as_bytes();
        let bs58 = bs58::decode(wif).into_vec().unwrap_or_default();

        let bs58_str: String = bs58.iter().map(|byte| format!("{:02X}", byte)).collect();

        bs58_str[2..bs58_str.len() - 10].to_string()
    }

    /// En base a los atributos de transacciones pendientes
    /// de esta wallet, se actualiza el balance
    pub fn update_pending_balance(&mut self) {
        let mut balance_pending = 0f64;

        for txn_info in &self.sending_txn {
            balance_pending -= txn_info.amount;
        }

        self.pending_balance = balance_pending;
    }

    /// Dado un txid, revisa la lista de transaccion pendientes de enviar,
    /// elimina la que no es mas "pendiente" y las coloca en transacciones
    /// realizadas
    pub fn update_sending_txn(&mut self, txid: String, bloque: &SerializedBlock) {
        for i in 0..self.sending_txn.len() {
            let txid_pending = Txn::obtain_tx_id(self.sending_txn[i].txn.as_bytes());
            if txid_pending == txid {
                let mut made_txn = self.sending_txn[i].clone();
                made_txn.txn_type = TxnType::Sent;
                made_txn.bloque = SerializedBlock::obtain_blockhash(bloque.block_header.as_bytes());
                self.sent_txn.push(made_txn);
                self.sending_txn.remove(i);
                break;
            }
        }
    }

    /// Dado un txid, revisa la lista de transacciones pendientes de recibir,
    /// elimina la que no es mas "pendiente" y las coloca en transacciones
    /// recibidas
    pub fn update_receiving_txn(&mut self, txid: String, txn: &Txn) {
        for i in 0..self.receiving_txn.len() {
            let txid_receiving = Txn::obtain_tx_id(self.receiving_txn[i].txn.as_bytes());
            if txid_receiving == txid {
                let mut received_txn = self.receiving_txn[i].clone();
                if received_txn.address == "-" {
                    received_txn.address =
                        match Script::obtain_public_adress(txn.tx_in[0].signature_script.clone()) {
                            Ok(s) => s,
                            Err(_) => "-".to_owned(),
                        }
                }
                received_txn.txn_type = TxnType::Received;
                self.saved_received_txn.push(received_txn);
                self.receiving_txn.remove(i);
                break;
            }
        }
    }

    /// Transforma el hashmap de utxos en vector de txn_info,
    /// mergeando con los datos historicos guardados por archivo
    pub fn obtain_utxo_info(&self) -> Vec<TxnInfo> {
        let txn_info: Vec<TxnInfo> = self.transform_utxo_in_info();
        let mut txn_info_final = self.saved_received_txn.clone();
        let mut duplicacion = false;

        for utxo in &txn_info {
            for received_saved in &self.saved_received_txn {
                if received_saved.txn == utxo.txn {
                    duplicacion = true;
                    break;
                }
            }
            //Caso que no hubo nunca coincidencia y se salio dl for, guardo la utxo
            if !duplicacion {
                txn_info_final.push(utxo.clone());
            } else {
                duplicacion = false;
            }
        }

        txn_info_final
    }
    /// Obtiene un vector de TxnInfo que combina
    /// las transacciones pendientes (sending y receiving)
    pub fn pending_txn(&self) -> Vec<TxnInfo> {
        let mut v = vec![];
        v.append(&mut self.sending_txn.clone());
        v.append(&mut self.receiving_txn.clone());
        v
    }

    /// Obtiene en base a un hashmap de UTXOs una
    /// lista de UTXO_info, para usar en la interfaz
    fn transform_utxo_in_info(&self) -> Vec<TxnInfo> {
        let mut txn_info: Vec<TxnInfo> = vec![];
        let mut info: TxnInfo;
        for (k, v) in &self.utxo_transaction {
            let address = match Script::obtain_public_adress(v.tx_in[0].signature_script.clone()) {
                Ok(s) => s,
                Err(_) => "-".to_owned(),
            };
            let mut label = "-".to_owned();
            if address == self.public_address {
                label = "Change".to_owned();
            }
            info = TxnInfo::new(
                v.clone(),
                TxnType::Received,
                label,
                amount_of_satoshis(&v.tx_out[k.1 as usize]),
                address,
                '-'.to_string(),
            );
            txn_info.push(info);
        }
        txn_info
    }
}

// Determina la cantidad de satoshis a gastar del output
pub fn amount_of_satoshis(output: &TxOut) -> f64 {
    output.value_amount_satoshis as f64 / 100000000.0
}

/// Obtiene el p2pkh del output. Si la transaccion no esta firmada con este tipo de dato,
/// entendemos que no matcheara con ninguna de las wallets que se cargarán.
///
/// El formato que tenemos en cuenta es:
///
/// OP_DUP OP_HASH160 push_bytes [pubkeyHash] OP_EQUALVERIFY OP_CHECKSIG
pub fn obtain_pubkey_hash(output: &TxOut) -> Vec<u8> {
    let raw_pk_script = &output.pk_script;
    let raw_pk_script_bytes = output.pk_script_bytes.value() as usize;
    if is_p2pkh(raw_pk_script, raw_pk_script_bytes) {
        output.pk_script[3..(output.pk_script.len() - 2)].to_vec()
    } else {
        [0_u8; 16].to_vec()
    }
}

/// Verifica que el pkhash sea del tipo que acepta el proyecto (P2PKH).
///
/// Si no es de dicho tipo, se ignoran
pub fn is_p2pkh(raw_pk_script: &[u8], raw_pk_script_bytes: usize) -> bool {
    raw_pk_script_bytes != 0
        && raw_pk_script[0] == OP_DUP
        && raw_pk_script[1] == OP_HASH160
        && raw_pk_script[raw_pk_script_bytes - 1] == OP_CHECKSIG
        && raw_pk_script[raw_pk_script_bytes - 2] == OP_EQUALVERIFY
}

#[cfg(test)]
mod tests {
    use crate::{
        account::Account, compactsize::CompactSize, outpoint::OutPoint, txin::TxIn, txn::Txn,
        txout::TxOut,
    };
    use std::collections::HashMap;

    type TrxKey = (String, u32);
    type TrxHashMap<T> = HashMap<TrxKey, T>;

    #[test]
    fn test_decode_bitcoin_adress() {
        let pubkey_hash: Vec<u8> = [
            0x3e, 0xc0, 0x6a, 0x65, 0x5a, 0x14, 0x94, 0x5b, 0x2d, 0x18, 0x96, 0xda, 0x6b, 0xed,
            0x6d, 0xd5, 0x5f, 0xf4, 0x26, 0x9e,
        ]
        .to_vec();
        let account = Account::new_str("mmEkhDcx6xt28zTXvvNjBjCCQCXUwrKXBi", "");
        assert_eq!(pubkey_hash, account.decode_bitcoin_adress().unwrap());
        assert_eq!(
            Account::encode_bitcoin_adress(pubkey_hash),
            "mmEkhDcx6xt28zTXvvNjBjCCQCXUwrKXBi"
        );
    }

    #[test]
    fn obtain_hex_privatekey_test() {
        let emisor = Account::new_str("", "cRCLe18WvER3JYsfpGvNDncbsZhdecFwQmiVGBcRcC5EJLz7jRaG");
        assert_eq!(emisor.obtain_hex_privatekey().len(), 64);
        assert_eq!(
            emisor.obtain_hex_privatekey(),
            "6BD8798493D1734F8287847C11319299C21ECD6E962E37707BC5A1615A5A1C00"
        );
    }

    #[test]
    fn test_obtain_account_balance_() {
        let test_utxos = generar_utxos_for_test();

        test_obtain_account_balance_empty(&test_utxos);
        test_obtain_account_balance_one_utxo(&test_utxos);
        test_obtain_account_balance_multi_utxo(&test_utxos);
    }

    fn test_obtain_account_balance_empty(utxos: &TrxHashMap<Txn>) {
        let mut account = Account::new_str("mx9RxvB9bFVqRUXAjjTDDiJmZkVWEaDj6J", "");
        account.obtain_account_balance(&utxos);
        assert_eq!(account.balance, 0 as f64);
    }

    fn test_obtain_account_balance_one_utxo(utxos: &TrxHashMap<Txn>) {
        let mut account = Account::new_str("mkbyF2EZNjAADM7aLCfHAHtxJ9B6cn7FKm", "");
        account.obtain_account_balance(&utxos);
        assert_eq!(account.balance, 0.01668227 as f64);
    }

    fn test_obtain_account_balance_multi_utxo(utxos: &TrxHashMap<Txn>) {
        let mut account = Account::new_str("mremfsNt32NAqPodczJQcY9sfKbcFk33ge", "");
        account.obtain_account_balance(&utxos);
        assert_eq!(account.balance, 0.05744412 as f64);
    }

    fn generar_utxos_for_test() -> HashMap<(String, u32), Txn> {
        let mut test_utxos = HashMap::new();
        test_utxos.insert(
            (
                "cb4a869cae9187fa664e7fcb11a0962fb205d3accac81893694a241dba24aeda".to_string(),
                1,
            ),
            Txn {
                version: 2,
                tx_in_count: CompactSize {
                    number: [1].to_vec(),
                },
                tx_in: vec![TxIn {
                    previous_output: OutPoint {
                        hash_previous_output_txid: [
                            0x20, 0x65, 0x7f, 0x48, 0xf9, 0x3e, 0xaa, 0x63, 0x21, 0x8f, 0xa1, 0xe5,
                            0x3f, 0xc4, 0xf9, 0x12, 0xd5, 0x35, 0x65, 0xdd, 0x41, 0x8f, 0xed, 0x3a,
                            0xe6, 0xca, 0x71, 0xa1, 0x0c, 0xf1, 0xda, 0xab,
                        ],
                        output_index: 0,
                    },
                    script_bytes: CompactSize {
                        number: [0].to_vec(),
                    },
                    signature_script: vec![],
                    sequence: 0xfffffffd,
                }],
                tx_out_count: CompactSize {
                    number: [2].to_vec(),
                },
                tx_out: vec![
                    TxOut {
                        value_amount_satoshis: 0x1ae030599,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0xe0, 0xc7, 0x7a, 0x49, 0xd0, 0x72, 0x4a, 0x4f, 0xcc,
                            0x72, 0xde, 0x86, 0x48, 0x40, 0x0e, 0x91, 0x89, 0x9e, 0x23, 0xe3, 0x88,
                            0xac,
                        ],
                    },
                    TxOut {
                        value_amount_satoshis: 0x247ea2,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0x7a, 0x23, 0xd7, 0xcb, 0xca, 0x2b, 0xb5, 0x41, 0xd2,
                            0x80, 0x45, 0xca, 0x9f, 0x7d, 0x2a, 0x40, 0x5f, 0xa7, 0x94, 0x9e, 0x88,
                            0xac,
                        ],
                    },
                ],
                tx_lock_time: 0x252d80,
            },
        );

        test_utxos.insert(
            (
                "28d275e502de8807998d851748b468205df9931fa042f2d5a961bc7b17acfbf0".to_string(),
                0,
            ),
            Txn {
                version: 2,
                tx_in_count: CompactSize {
                    number: [1].to_vec(),
                },
                tx_in: vec![TxIn {
                    previous_output: OutPoint {
                        hash_previous_output_txid: [
                            0x8e, 0x15, 0xce, 0x5b, 0xa6, 0xb0, 0x17, 0x06, 0x1c, 0x65, 0xe0, 0xdd,
                            0x47, 0x78, 0x56, 0x79, 0x82, 0x11, 0x35, 0x38, 0xe9, 0x41, 0x78, 0x6e,
                            0x3f, 0x70, 0x49, 0xbc, 0xf8, 0x55, 0xde, 0xf1,
                        ],
                        output_index: 1,
                    },
                    script_bytes: CompactSize {
                        number: [0].to_vec(),
                    },
                    signature_script: vec![],
                    sequence: 0xfffffffd,
                }],
                tx_out_count: CompactSize {
                    number: [2].to_vec(),
                },
                tx_out: vec![
                    TxOut {
                        value_amount_satoshis: 0x19e2d9,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0x7a, 0x23, 0xd7, 0xcb, 0xca, 0x2b, 0xb5, 0x41, 0xd2,
                            0x80, 0x45, 0xca, 0x9f, 0x7d, 0x2a, 0x40, 0x5f, 0xa7, 0x94, 0x9e, 0x88,
                            0xac,
                        ],
                    },
                    TxOut {
                        value_amount_satoshis: 0x182b3261b,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0xa2, 0xfb, 0xd6, 0x51, 0xda, 0xc6, 0x75, 0x06, 0xd3,
                            0x23, 0xbf, 0x4a, 0xd6, 0x92, 0xf0, 0x7c, 0xa9, 0xc8, 0x28, 0xd4, 0x88,
                            0xac,
                        ],
                    },
                ],
                tx_lock_time: 0x252f38,
            },
        );

        test_utxos.insert(
            (
                "21cfb94391b5822a8d955f510f3a61b206b198df2eb2d87cff83876d4a5ce484".to_string(),
                0,
            ),
            Txn {
                version: 2,
                tx_in_count: CompactSize {
                    number: [1].to_vec(),
                },
                tx_in: vec![TxIn {
                    previous_output: OutPoint {
                        hash_previous_output_txid: [
                            0x28, 0x70, 0xa6, 0x11, 0x61, 0xe4, 0x24, 0x92, 0x9a, 0xd6, 0x2e, 0x6c,
                            0x3c, 0x4d, 0x2b, 0x75, 0xac, 0x03, 0x4b, 0x1b, 0x4e, 0x0a, 0x63, 0xff,
                            0x69, 0x77, 0x8f, 0xa8, 0x18, 0xd8, 0x08, 0xce,
                        ],
                        output_index: 0,
                    },
                    script_bytes: CompactSize {
                        number: [0].to_vec(),
                    },
                    signature_script: vec![],
                    sequence: 0xfffffffd,
                }],
                tx_out_count: CompactSize {
                    number: [2].to_vec(),
                },
                tx_out: vec![
                    TxOut {
                        value_amount_satoshis: 0x197483,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0x37, 0xcb, 0x7f, 0xf6, 0x1b, 0xe2, 0x2b, 0xe2, 0x64,
                            0x44, 0x73, 0xbc, 0x3c, 0xff, 0xad, 0x63, 0xdb, 0x74, 0xc0, 0xaa, 0x88,
                            0xac,
                        ],
                    },
                    TxOut {
                        value_amount_satoshis: 0x1c3b49ea6,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0x10, 0x68, 0x5f, 0x74, 0x58, 0xff, 0x5d, 0xf4, 0xdf,
                            0x46, 0xd3, 0x08, 0xf0, 0x2c, 0x78, 0xe1, 0xab, 0x2f, 0xa4, 0x54, 0x88,
                            0xac,
                        ],
                    },
                ],
                tx_lock_time: 0x252d87,
            },
        );

        test_utxos.insert(
            (
                "273b00c69fbead9c8ca9878c5869c2397e08e7ebb773322a4c50fb331b166556".to_string(),
                1,
            ),
            Txn {
                version: 2,
                tx_in_count: CompactSize {
                    number: [1].to_vec(),
                },
                tx_in: vec![TxIn {
                    previous_output: OutPoint {
                        hash_previous_output_txid: [
                            0x76, 0xd6, 0x6e, 0xa4, 0xc1, 0xd2, 0x4e, 0xb9, 0x3a, 0xa8, 0xa1, 0x5b,
                            0x9c, 0x8e, 0x68, 0xe2, 0x1a, 0x74, 0xef, 0x0c, 0x16, 0xb1, 0xb8, 0xcb,
                            0xf8, 0x30, 0x1c, 0xb6, 0xde, 0xe1, 0xaa, 0x00,
                        ],
                        output_index: 0,
                    },
                    script_bytes: CompactSize {
                        number: [0].to_vec(),
                    },
                    signature_script: vec![],
                    sequence: 0xfffffffd,
                }],
                tx_out_count: CompactSize {
                    number: [2].to_vec(),
                },
                tx_out: vec![
                    TxOut {
                        value_amount_satoshis: 0x170c848c8,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0xe0, 0x8d, 0x07, 0xaa, 0x16, 0xcb, 0x14, 0x18, 0x20,
                            0x32, 0x3a, 0xa0, 0xec, 0xbf, 0x9d, 0xae, 0x93, 0x86, 0xc5, 0xa6, 0x88,
                            0xac,
                        ],
                    },
                    TxOut {
                        value_amount_satoshis: 0x1945a1,
                        pk_script_bytes: CompactSize {
                            number: [25].to_vec(),
                        },
                        pk_script: vec![
                            0x76, 0xa9, 0x14, 0x7a, 0x23, 0xd7, 0xcb, 0xca, 0x2b, 0xb5, 0x41, 0xd2,
                            0x80, 0x45, 0xca, 0x9f, 0x7d, 0x2a, 0x40, 0x5f, 0xa7, 0x94, 0x9e, 0x88,
                            0xac,
                        ],
                    },
                ],
                tx_lock_time: 0x252f85,
            },
        );

        test_utxos
    }
}
