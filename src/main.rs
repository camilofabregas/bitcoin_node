use gtk::glib;
use rustify_11::block_header::BlockHeader;
use rustify_11::inv::Inv;
use rustify_11::txn::Txn;
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rustify_11::config::Config;
use rustify_11::errors::{catch, RustifyError};
use rustify_11::gui::iniciar_gui;
use rustify_11::gui_events::GuiEvent;
use rustify_11::logger::initialize_logger;
use rustify_11::node::{conectar, handshake, initial_block_download, recibir_nuevos_bloques_txs};
use rustify_11::server::iniciar_server;
use rustify_11::utxo::obtain_utxo;
use rustify_11::wallet_events::{iniciar_wallet, WalletEvent};

//Tipo de dato de Hashmap de transacción
type TrxKey = (String, u32);
type TrxHashMap<T> = HashMap<TrxKey, T>;
type TrxServer = Vec<(String, Txn)>;
type OkInicioNodo = (
    TrxHashMap<Txn>,
    TcpStream,
    Arc<Mutex<Vec<BlockHeader>>>,
    Arc<Mutex<TrxServer>>,
);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config = match Config::load_config(&args) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    let logger_sender = initialize_logger(&config);

    let (sender_wallet, recv_wallet) = std::sync::mpsc::channel();
    let (sender_gui, recv_gui) = glib::MainContext::channel(glib::source::Priority::DEFAULT);

    let (sender_notif, recv_notif) = std::sync::mpsc::channel();

    iniciar_gui(recv_gui, sender_wallet.clone(), &config);

    let (utxos_init, mut socket, headers, txn_memory_client) = match iniciar_nodo(
        &config,
        &logger_sender,
        sender_gui.clone(),
        sender_wallet,
        sender_notif,
    ) {
        Ok((u, s, h, n)) => (u, s, h, n),
        Err(e) => {
            catch(e);
            std::process::exit(1);
        }
    };

    if config.server_mode {
        iniciar_server(
            &config,
            &logger_sender,
            headers,
            txn_memory_client,
            recv_notif,
        );
    }

    iniciar_wallet(
        &mut socket,
        &config,
        &logger_sender,
        utxos_init,
        recv_wallet,
        sender_gui,
    );
}

/// Inicializa un nodo Bitcoin de tipo light.
/// Se conecta a otros nodos, realiza un handshake, y descarga headers y bloques.
/// Queda a la espera de nuevos bloques para validar (y descargar si es valido).
pub fn iniciar_nodo(
    config: &Config,
    logger_sender: &Sender<String>,
    sender_gui: gtk::glib::Sender<GuiEvent>,
    sender_wallet: Sender<WalletEvent>,
    sender_notif: Sender<Inv>,
) -> Result<OkInicioNodo, RustifyError> {
    let mut socket = conectar(config, logger_sender)?;
    handshake(&mut socket, config, logger_sender)?;
    thread::sleep(Duration::from_millis(1000)); // Para que se llegue a ver el "Connecting to peers..." en la GUI.

    let headers = initial_block_download(&mut socket, config, logger_sender, &sender_gui)?;

    sender_gui.send(GuiEvent::CargarBloques(
        headers[config.height_bloque_inicial..].to_owned(),
        config.height_bloque_inicial as u32,
    ))?;

    let headers_ref = Arc::new(Mutex::new(headers)); // Usamos Arc Mutex para compartir el vector de headers entre threads.
    let mut headers_block_broadcasting = headers_ref.clone();

    let txn_memory_server: Arc<Mutex<TrxServer>> = Arc::new(Mutex::new(vec![]));
    let txn_memory_client = txn_memory_server.clone();

    let mut socket_clone = socket.try_clone()?;
    let config_clone = config.clone();
    let logger_sender_clone = logger_sender.clone();
    let sender_gui_clone = sender_gui.clone();
    thread::spawn(move || -> Result<(), RustifyError> {
        recibir_nuevos_bloques_txs(
            &mut socket_clone,
            &mut headers_block_broadcasting,
            txn_memory_server,
            &config_clone,
            (
                &logger_sender_clone,
                &sender_gui_clone,
                &sender_wallet,
                &sender_notif,
            ),
        )?;
        Ok(())
    });

    sender_gui.send(GuiEvent::ActualizarLabelEstado(
        "Obtaining UTXOs...".to_string(),
    ))?;

    let utxos = obtain_utxo(config, logger_sender)?;

    sender_gui.send(GuiEvent::ActualizarLabelEstado("Up to date.".to_string()))?;
    sender_gui.send(GuiEvent::OcultarEstado)?;

    Ok((utxos, socket, headers_ref, txn_memory_client))
}
