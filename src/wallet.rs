use crate::{
    account::Account,
    errors::RustifyError,
    logger::{log, log_err, log_with_parameters, Action, Lvl},
    txn::Txn,
    txn_info::{TxnInfo, TxnType},
    wallet_txn::{broadcast_txn, generar_txn},
};

use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    net::TcpStream,
    path::Path,
    sync::mpsc::Sender,
};

type TrxKey = (String, u32);
type TrxHashMap<T> = HashMap<TrxKey, T>;

#[derive(Default, Debug, Clone)]
pub struct Wallet {
    pub accounts: HashMap<String, Account>,
    pub path: String,
}

impl Wallet {
    pub fn new(path: String) -> Wallet {
        let accounts = HashMap::new();
        Wallet { accounts, path }
    }

    ///Funcion para poder agregar un account
    pub fn agregar_account(
        &mut self,
        alias: String,
        public_key: String,
        private_key: String,
        utxos: &TrxHashMap<Txn>,
    ) {
        let mut account_to_add = Account::new(public_key, private_key);
        account_to_add.obtain_account_balance(utxos);
        self.accounts.insert(alias, account_to_add);
    }

    /// Genera la transacción y la broadcastea
    pub fn send_txn(
        &self,
        socket: &mut TcpStream,
        logger_sender: &Sender<String>,
        emisor: &String,
        receptor: String,
        importe: f64,
        fee: f64,
    ) -> Result<Txn, RustifyError> {
        let receptor_account = Account::new(receptor, "".to_owned());

        let transaction = generar_txn(
            logger_sender,
            &self.accounts[emisor],
            receptor_account,
            importe,
            fee,
        )?;
        let txid = Txn::obtain_tx_id(transaction.as_bytes());

        broadcast_txn(&transaction, socket)?;

        log_with_parameters(
            Lvl::Info(Action::WALLET),
            format!(
                "Se ha broadcasteado exitosamente la transacción: {:?}",
                txid
            ),
            logger_sender,
        );
        Ok(transaction)
    }

    /// Si existe un archivo guardado, carga las wallets
    /// En caso de no existir, no se modifica nada
    pub fn load(
        &mut self,
        logger_sender: &Sender<String>,
        utxos: &TrxHashMap<Txn>,
    ) -> Result<(), RustifyError> {
        let path = Path::new(&self.path);
        if !path.exists() {
            return Err(RustifyError::NoHayWalletsGuardadas);
        }
        let archivo = OpenOptions::new().read(true).open(path)?;

        let mut txn_type = TxnType::Undefined;
        let mut alias: String = "".to_string();

        for line in BufReader::new(archivo).lines().flatten() {
            (txn_type, alias) = self.analizar_linea(line, alias, txn_type, utxos)?;
        }
        log(
            Lvl::Info(Action::WALLET),
            "Se cargaron exitosamente los datos de las wallets guardadas",
            logger_sender,
        );
        Ok(())
    }

    /// Guarda en disco la informacion de las wallets cargadas
    pub fn save(&self, logger_sender: &Sender<String>) -> Result<(), RustifyError> {
        let path = Path::new(&self.path);
        fs::create_dir_all(path.parent().unwrap_or(Path::new("")))?;
        _ = fs::remove_file(path);
        let mut archivo = match OpenOptions::new().write(true).create(true).open(path) {
            Ok(archivo) => archivo,
            Err(e) => {
                log_err(Action::WALLET, e, logger_sender);
                return Ok(());
            }
        };

        for (k, v) in &self.accounts {
            let linea = format!("WALLET {} {} {}\n", k, v.public_address, v.private_address);
            archivo.write_all(linea.as_bytes())?;

            Self::write_txn_info(&mut archivo, "SENDING", v.sending_txn.clone())?;

            Self::write_txn_info(&mut archivo, "SENT", v.sent_txn.clone())?;

            Self::write_txn_info(&mut archivo, "RECEIVING", v.receiving_txn.clone())?;

            Self::write_txn_info(&mut archivo, "RECEIVED", v.saved_received_txn.clone())?;
        }

        Ok(())
    }

    fn write_txn_info<W: Write>(
        writer: &mut W,
        section_header: &str,
        txn_infos: Vec<TxnInfo>,
    ) -> io::Result<()> {
        let linea = format!("{}\n", section_header);
        writer.write_all(linea.as_bytes())?;

        for txn_info in txn_infos {
            writer.write_all(Self::obtain_hexdump_from_txn_info(&txn_info).as_bytes())?;
        }

        Ok(())
    }

    fn obtain_hexdump_from_txn_info(txn_info: &TxnInfo) -> String {
        let mut hexdump: String = txn_info
            .txn
            .as_bytes()
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect();
        hexdump += " ";
        hexdump += &txn_info.label;
        hexdump += " ";
        hexdump += &txn_info.amount.to_string();
        hexdump += " ";
        hexdump += &txn_info.address.to_string();
        hexdump += " ";
        hexdump += &txn_info.bloque;
        hexdump += "\n";
        hexdump
    }

    /// Procesa una linea del archivo de /wallet_data/wallet.txt
    fn analizar_linea(
        &mut self,
        linea: String,
        mut alias: String,
        mut txn_type: TxnType,
        utxos: &TrxHashMap<Txn>,
    ) -> Result<(TxnType, String), RustifyError> {
        if linea.contains("WALLET") {
            let parts: Vec<&str> = linea.split_whitespace().collect();
            alias = parts[1].to_string();
            self.agregar_account(
                alias.to_owned(),
                parts[2].to_owned(),
                parts[3].to_owned(),
                utxos,
            );
        } else if linea.contains("SENDING") {
            txn_type = TxnType::Sending;
        } else if linea.contains("SENT") {
            txn_type = TxnType::Sent;
        } else if linea.contains("RECEIVING") {
            txn_type = TxnType::Receiving;
        } else if linea.contains("RECEIVED") {
            txn_type = TxnType::Received;
        } else {
            let palabras: Vec<&str> = linea.split_whitespace().collect();
            let vec_from_hex = (0..palabras[0].len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&linea[i..i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()?;

            let txn = Txn::from_bytes(vec_from_hex, 0)?.0;
            let label = palabras[1].to_owned();
            let amount = palabras[2].parse::<f64>().unwrap_or_default();
            let address = palabras[3].to_owned();
            let bloque = palabras[4].to_owned();

            match txn_type {
                TxnType::Sending => {
                    if let Some(val) = self.accounts.get_mut(&alias) {
                        val.sending_txn.push(TxnInfo::new(
                            txn,
                            TxnType::Sending,
                            label,
                            amount,
                            address,
                            bloque,
                        ))
                    }
                }
                TxnType::Sent => {
                    if let Some(val) = self.accounts.get_mut(&alias) {
                        val.sent_txn.push(TxnInfo::new(
                            txn,
                            TxnType::Sent,
                            label,
                            amount,
                            address,
                            bloque,
                        ))
                    }
                }
                TxnType::Receiving => {
                    if let Some(val) = self.accounts.get_mut(&alias) {
                        val.receiving_txn.push(TxnInfo::new(
                            txn,
                            TxnType::Receiving,
                            label,
                            amount,
                            address,
                            bloque,
                        ))
                    }
                }
                TxnType::Received => {
                    if let Some(val) = self.accounts.get_mut(&alias) {
                        val.saved_received_txn.push(TxnInfo::new(
                            txn,
                            TxnType::Received,
                            label,
                            amount,
                            address,
                            bloque,
                        ))
                    }
                }
                _ => {}
            };
        }
        Ok((txn_type, alias))
    }
}
