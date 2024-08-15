use std::{
    collections::HashMap,
    net::TcpStream,
    str::FromStr,
    sync::mpsc::{Receiver, Sender},
};

use bitcoin_hashes::{sha256d, Hash};
use secp256k1::SecretKey;

use crate::{
    account::{amount_of_satoshis, obtain_pubkey_hash, Account},
    config::Config,
    errors::{obtener_mensaje_personalizado, RustifyError},
    gui_events::GuiEvent,
    logger::{log, log_re_err, log_with_parameters, Action, Lvl},
    script::Script,
    serialized_block::SerializedBlock,
    txn::Txn,
    txn_info::{TxnInfo, TxnType},
    utxo::update_utxo,
    wallet::Wallet,
};

/// Estructura para matchear los distintos eventos que vienen de la
/// interfaz gráfica y hilo que recibe bloques y
/// transacciones, para utilizar la Wallet.
pub enum WalletEvent {
    AgregarWallet(String, String, String),
    CargarWallet(String),
    RealizarTransferencia(String, f64, String, String, f64),
    RecibirBloque(SerializedBlock),
    RecibirTxn(Txn, String),
    Cerrar,
}

type TrxKey = (String, u32);
type TrxHashMap<T> = HashMap<TrxKey, T>;

/// Inicia la wallet y espera que le lleguen acciones por el receiver desde la interfaz gráfica.
pub fn iniciar_wallet(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
    mut utxos: TrxHashMap<Txn>,
    recv_node: Receiver<WalletEvent>,
    sender_gui: gtk::glib::Sender<GuiEvent>,
) {
    log(
        Lvl::Info(Action::WALLET),
        "Iniciando lógica interna de Wallet...",
        logger_sender,
    );
    let mut wallets = Wallet::new(config.wallets_path.clone());

    (utxos, wallets) =
        cargar_wallets_inicio(wallets, &logger_sender.clone(), utxos, sender_gui.clone());

    loop {
        let evento = recv_node.recv();

        match evento {
            Ok(WalletEvent::AgregarWallet(private_key, public_key, alias)) => {
                wallets = evento_agregar_wallet(
                    wallets,
                    logger_sender,
                    &utxos,
                    private_key,
                    public_key,
                    alias,
                );
            }
            Ok(WalletEvent::CargarWallet(alias)) => {
                wallets = evento_cargar_wallet(logger_sender, wallets, alias, &sender_gui, &utxos);
            }
            Ok(WalletEvent::RealizarTransferencia(alias, amount, label, address, fee)) => {
                let tupla_txn_data = (amount, label, address, fee);
                wallets = evento_realizar_trx(
                    logger_sender,
                    &sender_gui,
                    wallets,
                    socket,
                    alias,
                    tupla_txn_data,
                );
            }
            Ok(WalletEvent::RecibirTxn(txn, txid)) => {
                wallets = match evento_recibir_txn(
                    wallets.clone(),
                    &logger_sender.clone(),
                    txn,
                    txid,
                    &sender_gui,
                ) {
                    Ok(w) => w,
                    Err(_) => {
                        log(
                            Lvl::Warning(Action::WALLET),
                            "Se obtuvo un error al recibir transaccion nueva.",
                            logger_sender,
                        );
                        continue;
                    }
                };
            }
            Ok(WalletEvent::RecibirBloque(bloque)) => {
                log(
                    Lvl::Info(Action::WALLET),
                    "Se recibió un bloque nuevo.",
                    logger_sender,
                );

                utxos = update_utxo(utxos, logger_sender, &bloque).unwrap_or_default();

                wallets = match evento_recibir_bloque(
                    wallets.clone(),
                    logger_sender,
                    &sender_gui,
                    &utxos,
                    bloque,
                ) {
                    Ok(w) => w,
                    Err(_) => {
                        log(
                            Lvl::Warning(Action::WALLET),
                            "Se obtuvo un error al recibir un bloque nuevo.",
                            logger_sender,
                        );
                        continue;
                    }
                };
            }
            Ok(WalletEvent::Cerrar) | Err(_) => {
                break;
            }
        }
    }
}

/// Carga en memoria las wallets existentes en los archivos de datos locales
pub fn cargar_wallets_inicio(
    mut wallets: Wallet,
    logger_sender: &Sender<String>,
    utxos: TrxHashMap<Txn>,
    sender_gui: gtk::glib::Sender<GuiEvent>,
) -> (TrxHashMap<Txn>, Wallet) {
    match wallets.load(logger_sender, &utxos) {
        Ok(_) => {
            let mut aliases = vec![];
            for alias in wallets.accounts.keys() {
                aliases.push(alias.to_string());
            }
            sender_gui
                .send(GuiEvent::IniciarWallets(aliases))
                .unwrap_or(());
        }
        Err(e) => {
            log(
                Lvl::Info(Action::WALLET),
                &obtener_mensaje_personalizado(e),
                logger_sender,
            );
        }
    };
    (utxos, wallets)
}

/// Agrega al HashMap de Wallets, la nueva wallet recibida a traves del evento.
/// Siempre y cuando sean validad la private y public key, y no exista una wallet con el mismo alias
pub fn evento_agregar_wallet(
    mut wallets: Wallet,
    logger_sender: &Sender<String>,
    utxos: &TrxHashMap<Txn>,
    private_key: String,
    public_key: String,
    alias: String,
) -> Wallet {
    if claves_validas(&private_key, &public_key) && wallets.accounts.get(&alias).is_none() {
        log_with_parameters(
            Lvl::Info(Action::WALLET),
            format!(
                "INFO: Creada wallet con alias {}, private key {} y public key {}.",
                alias, private_key, public_key
            ),
            logger_sender,
        );
        wallets.agregar_account(alias, public_key, private_key, utxos);
        wallets.save(logger_sender).unwrap_or(());
    } else {
        log(
            Lvl::Warning(Action::WALLET),
            "Las claves ingresadas son inválidas.",
            logger_sender,
        );
    }
    wallets
}

/// Se carga la wallet recibida desde el evento, siempre y cuando
/// se selected una wallet valida
pub fn evento_cargar_wallet(
    logger_sender: &Sender<String>,
    mut wallets: Wallet,
    alias: String,
    sender_gui: &gtk::glib::Sender<GuiEvent>,
    utxos: &TrxHashMap<Txn>,
) -> Wallet {
    match wallets.accounts.get(&alias) {
        Some(_) => {
            log_with_parameters(
                Lvl::Info(Action::WALLET),
                format!("INFO: Cargada wallet con alias {}.", alias),
                logger_sender,
            );

            if let Some(val) = wallets.accounts.get_mut(&alias) {
                val.obtain_account_balance(utxos);
                val.update_pending_balance();
            }

            sender_gui
                .send(GuiEvent::ActualizarWallet(wallets.accounts[&alias].clone()))
                .unwrap_or(());
        }
        None => {
            log_with_parameters(
                Lvl::Warning(Action::WALLET),
                format!("Se intentó cargar la wallet {}, que no existe.", alias),
                logger_sender,
            );
        }
    }
    wallets
}

/// Se genera y broadcastea la transaccion pedida desde evento
pub fn evento_realizar_trx(
    logger_sender: &Sender<String>,
    sender_gui: &gtk::glib::Sender<GuiEvent>,
    mut wallets: Wallet,
    socket: &mut TcpStream,
    alias: String,
    tupla_txn_data: (f64, String, String, f64),
) -> Wallet {
    if wallets.accounts.get(&alias).is_some() {
        let (amount, label, address, fee) = tupla_txn_data;
        let emisor_adress = wallets.accounts[&alias].public_address.clone();
        log_with_parameters(
            Lvl::Info(Action::WALLET),
            format!(
                "Enviando {} bitcoins a adress {}, con detalle {}. Costo de la transaccion: {}.",
                amount, address, label, fee
            ),
            logger_sender,
        );
        match wallets.send_txn(socket, logger_sender, &alias, address.clone(), amount, fee) {
            Ok(transaction) => {
                if let Some(val) = wallets.accounts.get_mut(&alias) {
                    val.sending_txn.push(TxnInfo::new(
                        transaction.clone(),
                        TxnType::Sending,
                        label.replace(' ', "_"),
                        amount + fee,
                        address.clone(),
                        '-'.to_string(),
                    ));
                    val.update_pending_balance()
                }
                for (aux_alias, wallet) in wallets.accounts.clone() {
                    if address == wallet.public_address {
                        if let Some(val) = wallets.accounts.get_mut(&aux_alias) {
                            val.receiving_txn.push(TxnInfo::new(
                                transaction,
                                TxnType::Receiving,
                                label.replace(' ', "_"),
                                amount,
                                emisor_adress,
                                '-'.to_string(),
                            ));
                        }
                        break;
                    }
                }
                wallets.save(logger_sender).unwrap_or(());

                sender_gui
                    .send(GuiEvent::ActualizarWallet(wallets.accounts[&alias].clone()))
                    .unwrap_or(());
            }
            Err(e) => log_re_err(Action::WALLET, e, logger_sender),
        };
    }
    wallets
}

/// Se genera un log cuando se recibe una transaccion nueva por el evento
pub fn evento_recibir_txn(
    mut wallets: Wallet,
    logger_sender: &Sender<String>,
    txn: Txn,
    txid: String,
    sender_gui: &gtk::glib::Sender<GuiEvent>,
) -> Result<Wallet, RustifyError> {
    log_with_parameters(
        Lvl::Info(Action::WALLET),
        format!("Se recibió la transacción {}.", txid),
        logger_sender,
    );
    let sigscript = txn.tx_in[0].signature_script.clone();

    let mut needs_save = false;

    for output_index in 0..txn.tx_out.len() {
        for (_, wallet) in wallets.accounts.iter_mut() {
            let pubkey_hash = wallet.decode_bitcoin_adress()?;

            if obtain_pubkey_hash(&txn.tx_out[output_index]) == pubkey_hash {
                // Se obtuvo una Txn perteneciente a una wallet
                needs_save = true;
                let txn_clone = txn.clone();

                let address = match Script::obtain_public_adress(sigscript.clone()) {
                    Ok(s) => s,
                    Err(_) => "-".to_owned(),
                };
                let mut label = "-".to_owned();
                if address == wallet.public_address {
                    label = "Change".to_owned();
                }

                wallet.receiving_txn.push(TxnInfo::new(
                    txn_clone,
                    TxnType::Receiving,
                    label,
                    amount_of_satoshis(&txn.tx_out[output_index]),
                    address,
                    '-'.to_string(),
                ));

                sender_gui
                    .send(GuiEvent::ActualizarWallet(wallet.clone()))
                    .unwrap_or(());
            }
        }
    }
    if needs_save {
        wallets.save(logger_sender)?;
    }

    Ok(wallets)
}

/// Se actualizan el listado de las UTXO, y si alguna de las transaccion
/// del bloque nuevo pertenece a una de las wallets cargadas, se actualizaran
/// sus parametros de txns
pub fn evento_recibir_bloque(
    mut wallets: Wallet,
    logger_sender: &Sender<String>,
    sender_gui: &gtk::glib::Sender<GuiEvent>,
    utxos: &TrxHashMap<Txn>,
    bloque: SerializedBlock,
) -> Result<Wallet, RustifyError> {
    //Actualiza balances de todas, y recibe el dinero como UTXO
    for (_, wallet) in wallets.accounts.iter_mut() {
        wallet.obtain_account_balance(utxos);
    }

    let mut needs_save = false;

    for tx_index in 0..bloque.txns.len() {
        for output_index in 0..bloque.txns[tx_index].tx_out.len() {
            for (_, wallet) in wallets.accounts.iter_mut() {
                let pubkey_hash = wallet.decode_bitcoin_adress()?;

                if obtain_pubkey_hash(&bloque.txns[tx_index].tx_out[output_index]) == pubkey_hash {
                    // Se obtuvo una Txn en el bloque, perteneciente a una wallet
                    log(
                        Lvl::Info(Action::WALLET),
                        "Una transaccion en el bloque pertenece a una de las wallets",
                        logger_sender,
                    );

                    let txid = Txn::obtain_tx_id(bloque.txns[tx_index].as_bytes());
                    wallet.update_sending_txn(txid.clone(), &bloque);
                    wallet.update_receiving_txn(txid, &bloque.txns[tx_index]);
                    wallet.update_pending_balance();

                    needs_save = true;

                    sender_gui
                        .send(GuiEvent::ActualizarWallet(wallet.clone()))
                        .unwrap_or(());
                }
            }
        }
    }

    if needs_save {
        wallets.save(logger_sender)?;
    }

    Ok(wallets)
}

/// Verifica si las claves publicas y privadas ingresadas son validas
pub fn claves_validas(private_key: &str, public_key: &str) -> bool {
    match decode_public_key(public_key.to_owned()) {
        (true, _) => {}
        (false, _) => return false,
    };
    SecretKey::from_str(&Account::new_str(public_key, private_key).obtain_hex_privatekey()).is_ok()
}

/// Verifica si la Bitcoin Adress ingresada es una pubkey valida
fn decode_public_key(public_key: String) -> (bool, Vec<u8>) {
    let b58 = match bs58::decode(public_key).into_vec() {
        Ok(b) => b,
        Err(_) => return (false, vec![]),
    };
    if b58.len() > 5 {
        let b58_checksum = &b58[b58.len() - 4..b58.len()];
        let b58_hashversion = &b58[0..b58.len() - 4];
        if b58_checksum != &sha256d::Hash::hash(b58_hashversion)[0..4] {
            (false, vec![])
        } else {
            (true, b58_hashversion[1..].to_vec())
        }
    } else {
        (false, vec![])
    }
}

#[cfg(test)]
mod tests {
    use crate::{account::Account, wallet_events::claves_validas};

    #[test]
    fn claves_validas_test() {
        let cuenta = Account::new_str(
            "mremfsNt32NAqPodczJQcY9sfKbcFk33ge",
            "cRQuMXoGdBQm6iKmJ1fyT6qqCkK9AtAadFeoxqN4QYWsA8wN3eyy",
        );
        assert_eq!(
            claves_validas(&cuenta.private_address, &cuenta.public_address),
            true
        );
        let cuenta = Account::new_str("a", "a");
        assert_eq!(
            claves_validas(&cuenta.private_address, &cuenta.public_address),
            false
        );
        let cuenta = Account::new_str("aasgsafdgfdsagasdgf", "aasdfsadfasdfasdfsadfas");
        assert_eq!(
            claves_validas(&cuenta.private_address, &cuenta.public_address),
            false
        );
    }
}
