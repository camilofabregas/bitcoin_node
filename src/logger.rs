use std::{
    fs::{File, OpenOptions},
    io::prelude::*,
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

use crate::{
    config::Config,
    errors::{catch, obtener_mensaje_personalizado, RustifyError},
};
use std::sync::{Arc, Mutex};

/// Niveles de registro del logger.
pub enum Lvl {
    Info(Action),
    Error(Action),
    Warning(Action),
}

//Accion o procedimiento que esta siendo ejecutado
pub enum Action {
    UTXO,
    THREADPOOL,
    INB,
    CONNECT,
    WALLET,
    POWPOI,
    SERVER,
    NETWORK,
    LISTENER,
}

/// Logger que registra mensajes en un archivo o los imprime por pantalla, dependiendo de la configuración.
#[derive(Debug, Clone)]
pub struct Logger {
    file: Arc<Mutex<File>>,
    print_logger: bool,
}

impl Logger {
    /// Crea una nueva instancia del logger.
    ///
    /// * `log_file_path` - Ruta del archivo de logs.
    /// * `init_logger` - Indica si se debe inicializar el logger para escribir en el archivo.
    pub fn new(log_file_path: &str, print_logger: bool) -> std::io::Result<Self> {
        let file_path = Path::new(log_file_path);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        Ok(Logger {
            file: Arc::new(Mutex::new(file)),
            print_logger,
        })
    }

    /// Inicializa el logger y devuelve los canales de comunicación para enviar mensajes al logger.
    pub fn init_logger(&self) -> Result<(Sender<String>, JoinHandle<()>), RustifyError> {
        let (sender, receiver): (Sender<String>, Receiver<String>) = mpsc::channel();
        let file = self.file.clone();
        let init_logger = self.print_logger;

        let handle = thread::spawn(move || {
            for content in receiver.iter() {
                if let Err(err) = {
                    let mut file = file
                        .lock()
                        .expect("FATAL ERROR: No se pudo hacer lock en el Logger.");
                    writeln!(file, "{}", content)
                } {
                    // Informo el error por consola, pero no corto la ejecución del programa sólo por no
                    // poder escribir en el archivo de logs
                    catch(err.into());
                }
                if !init_logger {
                    println!("{}", content);
                }
            }
        });

        Ok((sender, handle))
    }
}

/// Inicializa el logger con la configuración especificada y devuelve el logger_sender.
pub fn initialize_logger(config: &Config) -> Sender<String> {
    let logger = match Logger::new("logger.log", config.print_logger) {
        Ok(logger) => logger,
        Err(e) => {
            eprintln!("Error creating logger: {:?}", e);
            std::process::exit(1);
        }
    };

    let (logger_sender, _handle) = match logger.init_logger() {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error creating logger: {:?}", e);
            std::process::exit(1);
        }
    };

    logger_sender
}

/// Registra un mensaje en el logger, permitiendo que menssage tenga parametros
pub fn log_with_parameters(logdata: Lvl, message: String, logger_sender: &Sender<String>) {
    log(logdata, &message, logger_sender);
}

/// Envia al logger un mensaje a escribir, colocando tag timestamp, tag de loglevel,
/// tag de proceso en el que ocurre, y mensaje personalizado.
pub fn log(logdata: Lvl, message: &str, logger_sender: &Sender<String>) {
    let now = chrono::Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let (lvl, action) = log_tags(logdata);
    let mensaje = format!("[{}] [{}] [{}] {}", timestamp, action, lvl, message);
    logger_sender.send(mensaje).unwrap_or(());
}

/// Registra un log para errores de tipo std::io.
pub fn log_err(action: Action, e: std::io::Error, logger_sender: &Sender<String>) {
    log_re_err(action, e.into(), logger_sender);
}

/// Registra un log para errores de tipo RustifyError
pub fn log_re_err(action: Action, e: RustifyError, logger_sender: &Sender<String>) {
    log(
        Lvl::Error(action),
        &obtener_mensaje_personalizado(e),
        logger_sender,
    );
}

/// Define las tags a colocar en cada linea de log
fn log_tags(logdata: Lvl) -> (String, String) {
    match logdata {
        Lvl::Info(ac) => ("INFO".to_owned(), log_action(ac)),
        Lvl::Error(ac) => ("ERROR".to_owned(), log_action(ac)),
        Lvl::Warning(ac) => ("WARN".to_owned(), log_action(ac)),
    }
}

/// Define la tag del proceso en el que ocurre el log.
fn log_action(action: Action) -> String {
    match action {
        Action::UTXO => "UTXO",
        Action::THREADPOOL => "THREADPOOL",
        Action::INB => "INITIAL BLOCK DOWNLOAD",
        Action::CONNECT => "CONEXION",
        Action::WALLET => "WALLET",
        Action::POWPOI => "POW&POI",
        Action::SERVER => "SERVER",
        Action::NETWORK => "NETWORK",
        Action::LISTENER => "LISTENER",
    }
    .to_string()
}
