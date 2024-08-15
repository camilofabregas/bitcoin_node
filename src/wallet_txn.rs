use crate::{
    account::{amount_of_satoshis, Account},
    compactsize::CompactSize,
    errors::RustifyError,
    logger::{log, Action, Lvl},
    message_header::MessageHeader,
    node::write_to_node,
    script::Script,
    txn::Txn,
    txout::TxOut,
};
use bitcoin_hashes::{sha256d, Hash};
use secp256k1::{Message, Secp256k1, SecretKey};
use std::{collections::HashMap, net::TcpStream, str::FromStr, sync::mpsc::Sender};

// Tipo de dato de Hashmap de transacción
type TrxKey = (String, u32);
type TrxHashMap<T> = HashMap<TrxKey, T>;

/// Genera una transacción en base a los dato provistos: cuenta emisora, cuenta receptora
/// el dinero que se envía, etcetera
///
/// Nota: Es precondición tener la lista de UTXOs actualizada Y ejecutado el obtain_account_balance
pub fn generar_txn(
    logger_sender: &Sender<String>,
    emisor: &Account,
    receptor: Account,
    importe_btc: f64,
    fee_btc: f64,
) -> Result<Txn, RustifyError> {
    let importe_taxado = importe_btc + fee_btc;
    let mut transaction: Txn;

    log(
        Lvl::Info(Action::WALLET),
        "INFO: Generando TXN deseada",
        logger_sender,
    );

    if emisor.balance >= importe_taxado {
        //Es posible enviar dinero
        let (utxo_to_spend, vuelto) =
            calcular_inputs_outputs(importe_taxado, &emisor.utxo_transaction);
        transaction = Txn::new(emisor, receptor, importe_btc, vuelto, &utxo_to_spend)?;
        transaction = firmar(transaction, emisor)?;
        Ok(transaction)
    } else {
        Err(RustifyError::WalletSinFondosSuficientes)
    }
}

/// Envía la transacción en un mensaje de tipo "tx"
///  a traves del nodo bitcoin  
pub fn broadcast_txn(transaction: &Txn, socket: &mut TcpStream) -> Result<(), RustifyError> {
    let tx_message_bytes = transaction.as_bytes();
    let tx_message_header = MessageHeader::new("tx".to_string(), &tx_message_bytes);
    let tx_message_header_bytes = tx_message_header.as_bytes();
    write_to_node(socket, &tx_message_header_bytes, &tx_message_bytes)?;

    Ok(())
}

/// Determina las utxo que se van a utilizar para gastar (inputs), el vuelto hacia el emisor (outputs)
/// y además define si alcanza con una utxo o no para realizar la transaccion (esto es, uno o más inputs)
fn calcular_inputs_outputs(importe_taxado: f64, utxos: &TrxHashMap<Txn>) -> (TrxHashMap<Txn>, f64) {
    let mut utxo_txout: &TxOut;
    let mut utxo_to_spend: TrxHashMap<Txn> = HashMap::new();
    let mut alcanza_una_utxo = false;
    let mut importe_sin_vuelto = 0.0;

    //Determina si alcanza con una utxo
    for (trxkey, txn) in utxos {
        utxo_txout = &txn.tx_out[trxkey.1 as usize];

        if amount_of_satoshis(utxo_txout) >= importe_taxado {
            utxo_to_spend.insert(trxkey.clone(), txn.clone());
            importe_sin_vuelto += amount_of_satoshis(utxo_txout);

            alcanza_una_utxo = true;
            break;
        }
    }

    if !alcanza_una_utxo {
        for (trxkey, txn) in utxos {
            utxo_txout = &txn.tx_out[trxkey.1 as usize];
            utxo_to_spend.insert(trxkey.clone(), txn.clone());
            importe_sin_vuelto += amount_of_satoshis(utxo_txout);

            if importe_sin_vuelto >= importe_taxado {
                break;
            }
        }
    }

    // Calculo de vuelto
    let vuelto: f64 = importe_sin_vuelto - importe_taxado;

    (utxo_to_spend, vuelto)
}

/// Dada una transaccion, realiza el proceso de firma
/// y reemplaza dicho dato en scripts del input
pub fn firmar(mut transaction: Txn, firmante: &Account) -> Result<Txn, RustifyError> {
    for i in 0..transaction.tx_in.len() {
        let z = obtain_z(transaction.clone(), i);

        let (der_signature, sec_pubkey) = obtain_sec_der(z, firmante)?;

        let mut sigscript =
            Script::new(der_signature, sec_pubkey, firmante.decode_bitcoin_adress()?)?;

        transaction.tx_in[i].signature_script = sigscript.clone().as_vec();
        transaction.tx_in[i].script_bytes = CompactSize::new(sigscript.as_vec().len() as u64);
    }

    Ok(transaction)
}

/// Obtiene la SEC public key y la DER signature, necesarios para el procedimiento de firma
fn obtain_sec_der(z: [u8; 32], firmante: &Account) -> Result<(Vec<u8>, Vec<u8>), RustifyError> {
    let secp = Secp256k1::new();

    let trx_message = match Message::from_slice(&z) {
        Ok(m) => m,
        Err(_) => return Err(RustifyError::ErrorParseoTxn),
    };

    let private_key = match SecretKey::from_str(&firmante.obtain_hex_privatekey()) {
        Ok(k) => k,
        Err(_) => return Err(RustifyError::ErrorConversionSecretKey),
    };

    let signature = secp.sign_ecdsa(&trx_message, &private_key).serialize_der();
    let mut der_signature = signature.to_vec();
    der_signature.push(0x01);

    let sec_pubkey = private_key.public_key(&secp).serialize().to_vec(); // SEC Compressed

    Ok((der_signature, sec_pubkey))
}

/// Obtiene el mensaje a utilizar en la firma, conocido como z.
/// Para ello, elimina los script_bytes de los otros inputs que no
/// sean el del parametro i: esto, para realizar la firma con multiples
/// inputs. Tipo de firmado: SIGHASH_ALL
fn obtain_z(mut transaction: Txn, input_firma: usize) -> [u8; 32] {
    for i in 0..transaction.tx_in.len() {
        if i != input_firma {
            transaction.tx_in[i].script_bytes = CompactSize::new(0);
            transaction.tx_in[i].signature_script = vec![];
        }
    }

    let mut modified_trx = transaction.as_bytes();
    const SIGHASH_ALL: u32 = 1;
    modified_trx.append(&mut SIGHASH_ALL.to_le_bytes().to_vec());

    sha256d::Hash::hash(&modified_trx).to_byte_array()
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Sender;

    use crate::{
        account::Account,
        config::Config,
        errors::RustifyError,
        logger::Logger,
        txn::Txn,
        wallet_txn::{generar_txn, obtain_z},
    };

    #[test]
    fn generar_txn_wallet_test_err() {
        let config = crate::config::Config::new("./node.config").unwrap();
        let logger_sender = initialize_logger_test(&config);

        let mut emisor = Account::new_str(
            "mremfsNt32NAqPodczJQcY9sfKbcFk33ge",
            "cRQuMXoGdBQm6iKmJ1fyT6qqCkK9AtAadFeoxqN4QYWsA8wN3eyy",
        );
        let receptor = Account::new_str(
            "mvkRvqush6X2bJLihJyRJCEA3hygBCCXxs",
            "cRCLe18WvER3JYsfpGvNDncbsZhdecFwQmiVGBcRcC5EJLz7jRaG",
        );

        emisor.balance = 0.03544412;
        assert_eq!(
            generar_txn(&logger_sender, &emisor, receptor, 1f64, 0.01),
            Err(RustifyError::WalletSinFondosSuficientes)
        );
    }

    #[test]
    fn test_obtain_z() {
        let raw_txn = "0100000001813f79011acb80925dfe69b3def355fe914bd1d96a3f5f71bf8303c6a989c7d1000000001976a914a802fc56c704ce87c42d7c92eb75e7896bdc41ae88acfeffffff02a135ef01000000001976a914bc3b654dca7e56b04dca18f2566cdaf02e8d9ada88ac99c39800000000001976a9141c4bc762dd5423e332166702cb75f40df79fea1288ac19430600";
        let mod_txn_vec = (0..raw_txn.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw_txn[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .unwrap();
        let mod_txn = Txn::from_bytes(mod_txn_vec, 0).unwrap().0;

        let z = obtain_z(mod_txn, 0);

        let expected_z_str = "27e0c5994dec7824e56dec6b2fcb342eb7cdb0d0957c2fce9882f715e85d81a6";
        let expected_z = (0..expected_z_str.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&expected_z_str[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .unwrap();

        assert_eq!(z.to_vec(), expected_z);
    }

    fn initialize_logger_test(config: &Config) -> Sender<String> {
        let logger = match Logger::new("loggertest.log", config.print_logger) {
            Ok(logger) => logger,
            Err(e) => {
                eprintln!("Error creating logger: {}", e);
                std::process::exit(1);
            }
        };

        let (logger_sender, _handle) = match logger.init_logger() {
            Ok(result) => result,
            Err(_e) => {
                eprintln!("Error initializing logger");
                std::process::exit(1);
            }
        };

        logger_sender
    }
}
