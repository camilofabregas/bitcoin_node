use crate::block::{guardar_bloque_memoria, obtener_headers_validos_fecha};
use crate::block_header::{actualizar_header_blockchain, BlockHeader};
use crate::block_validation::{proof_of_inclusion, proof_of_work};
use crate::config::Config;
use crate::errors::RustifyError;
use crate::gui_events::GuiEvent;
use crate::inv::Inv;
use crate::logger::{log, log_err, log_re_err, log_with_parameters, Action, Lvl};
use crate::message_handler::handle_specific_message;
use crate::message_header::MessageHeader;
use crate::serialized_block::SerializedBlock;
use crate::server_notification::add_txn_in_memory;
use crate::threadpool::ThreadPool;
use crate::txn::Txn;
use crate::version::{verack, version};
use crate::wallet_events::WalletEvent;
use rand::prelude::*;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MSG_TX: usize = 1;
const MSG_BLOCK: usize = 2;
type TrxServer = Vec<(String, Txn)>;

/// Conecta el nodo a otro nodo del DNS de Bitcoin Testnet.
/// Devuelve el TcpStream con la conexión establecida.
pub fn conectar(
    config: &Config,
    logger_sender: &Sender<String>,
) -> Result<TcpStream, RustifyError> {
    let server: Vec<_> = config.address.to_socket_addrs()?.collect();

    for _ in 0..config.cant_retries {
        let mut random_ip = server
            .choose(&mut rand::thread_rng())
            .expect("ERROR: Error al obtener una direccion IP.");
        while !random_ip.is_ipv4() {
            random_ip = server
                .choose(&mut rand::thread_rng())
                .expect("ERROR: Error al obtener una direccion IP.");
        }
        match TcpStream::connect_timeout(random_ip, Duration::new(config.timeout_secs, 0)) {
            Ok(conexion) => {
                log_with_parameters(
                    Lvl::Info(Action::CONNECT),
                    format!("Se realizo la conexion con la IP: {}", random_ip),
                    logger_sender,
                );
                return Ok(conexion);
            }
            Err(e) => {
                log_with_parameters(
                    Lvl::Error(Action::CONNECT),
                    format!("La ip {} da el error {}", random_ip, e),
                    logger_sender,
                );
                log_err(Action::CONNECT, e, logger_sender)
            }
        };
    }
    Err(RustifyError::NoHayConexionesDisponibles)
}

/// Hace el handshake con el nodo conectado, para terminar de establecer la conexión.
/// Envía y recibe los mensajes version y verack.
pub fn handshake(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    version(socket, config, logger_sender)?;
    verack(socket, logger_sender)?;
    log(
        Lvl::Info(Action::CONNECT),
        "Se realizó el handshake con el nodo. Conexión establecida",
        logger_sender,
    );

    Ok(())
}

/// Initial Block Download, con método Headers First.
/// 1) Chequear si tengo la cadena de BLOQUES completa.
/// 2) Si 1) es NO, chequear si tengo la cadena de HEADERS completa.
/// 3) Si 2) es NO, se descargan los headers restantes con el mensaje getheaders.
pub fn initial_block_download(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
    sender_gui: &gtk::glib::Sender<GuiEvent>,
) -> Result<Vec<BlockHeader>, RustifyError> {
    let (headers, indice_ultimo_header) =
        actualizar_header_blockchain(socket, config, logger_sender, sender_gui)?; // Vector con todos los headers en memoria.

    let headers_validos_fecha =
        obtener_headers_validos_fecha(config, &headers, indice_ultimo_header);
    let cant_bloques_a_descargar = headers_validos_fecha.len();

    log_with_parameters(
        Lvl::Info(Action::INB),
        format!(
            "Cantidad de bloques a descargar: {:?}",
            cant_bloques_a_descargar
        ),
        logger_sender,
    );

    sender_gui.send(GuiEvent::ActualizarLabelEstado(
        "Downloading blocks...".to_string(),
    ))?;

    if cant_bloques_a_descargar == 0 {
        // Si no hay bloques a descargar, no hace falta inicializar la threadpool.
        return Ok(headers);
    }

    log(
        Lvl::Info(Action::INB),
        "Creación de nuevos threads y handshake con nuevos nodos.",
        logger_sender,
    );

    let threads = ThreadPool::build(config, logger_sender)?;

    log(
        Lvl::Info(Action::INB),
        "Descargando nuevos bloques...",
        logger_sender,
    );

    threads.download_blocks(headers_validos_fecha, logger_sender)?;

    log(
        Lvl::Info(Action::INB),
        "INFO: Descarga de bloques finalizada.",
        logger_sender,
    );

    Ok(headers)
}

/// El nodo queda a la espera de nuevos bloques y transacciones enviados por el nodo remoto para su validación.
/// Se reciben mensajes inv y se filtran aquellos que son de tipo bloque o de tipo transacción.
pub fn recibir_nuevos_bloques_txs(
    socket: &mut TcpStream,
    headers: &mut Arc<Mutex<Vec<BlockHeader>>>,
    mut txn_memory_server: Arc<Mutex<Vec<(String, Txn)>>>,
    config: &Config,
    tupla_senders: (
        &Sender<String>,
        &gtk::glib::Sender<GuiEvent>,
        &Sender<WalletEvent>,
        &Sender<Inv>,
    ),
) -> Result<(), RustifyError> {
    let (logger_sender, sender_gui, sender_wallet, sender_notif) = tupla_senders;
    let mut bytes_respuesta: Vec<u8>;
    let mut headers_archivo = OpenOptions::new()
        .read(true)
        .write(true)
        .append(true)
        .open(config.headers_path.clone())?;
    let blocks_path = config.blocks_path.clone();
    log(
        Lvl::Info(Action::LISTENER),
        "Ha iniciado el proceso que recibe notificaciones de bloques y transacciones",
        logger_sender,
    );
    loop {
        // Filtro si el mensaje recibido es "inv".
        bytes_respuesta = match handle_specific_message(
            socket,
            "inv\0\0\0\0\0\0\0\0\0".to_string(),
            logger_sender,
        ) {
            Ok(b) => b,
            Err(e) => {
                if e == RustifyError::ElNodoNoEncuentraBloquePedido
                    || e == RustifyError::ElNodoNoEncuentraTransaccionPedida
                {
                    continue;
                } else {
                    log_re_err(Action::LISTENER, e.clone(), logger_sender);
                    return Err(e);
                }
            }
        };

        let inv_recibido = Inv::from_bytes(&bytes_respuesta)?;
        let tipo_inv = inv_recibido.inventories[0][0] as usize;
        let tupla_senders = (sender_gui, sender_wallet);
        // Filtro los inv recibidos.
        if tipo_inv == MSG_BLOCK {
            recibir_bloque(
                socket,
                headers,
                logger_sender,
                &mut headers_archivo,
                &blocks_path,
                bytes_respuesta,
                tupla_senders,
            )?;
        } else if tipo_inv == MSG_TX {
            recibir_transaccion(
                socket,
                config,
                logger_sender,
                &inv_recibido,
                sender_wallet,
                &mut txn_memory_server,
            )?;
        } else {
            log(
                Lvl::Info(Action::NETWORK),
                "Inv de otro tipo. Mensaje ignorado.",
                logger_sender,
            );
            continue;
        }
        if config.server_mode {
            match sender_notif.send(inv_recibido) {
                Ok(_) => log(
                    Lvl::Info(Action::LISTENER),
                    "Se envía inv por el channel del servidor",
                    logger_sender,
                ),
                Err(e) => log_re_err(Action::LISTENER, e.into(), logger_sender),
            };
        }
    }
}

/// Envia la transacción recibida como mensaje Inv, a la wallet, parseandola a txid
fn recibir_transaccion(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
    inv_txn: &Inv,
    sender_wallet: &Sender<WalletEvent>,
    txn_memory_server: &mut Arc<Mutex<TrxServer>>,
) -> Result<(), RustifyError> {
    log(
        Lvl::Info(Action::NETWORK),
        "Inv de tipo transaccion.",
        logger_sender,
    );
    let cant_inv = send_inv("getdata".to_owned(), socket, inv_txn)? as usize;
    for _ in 0..cant_inv {
        let bytes_respuesta = match handle_specific_message(
            socket,
            "tx\0\0\0\0\0\0\0\0\0\0".to_string(),
            logger_sender,
        ) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let txid_str = Txn::obtain_tx_id(bytes_respuesta.clone());
        let (transaccion, _) = Txn::from_bytes(bytes_respuesta.to_vec(), 0)?;
        if config.server_mode {
            add_txn_in_memory(
                txn_memory_server,
                &transaccion,
                &txid_str,
                config,
                logger_sender,
            )?;
        }
        sender_wallet.send(WalletEvent::RecibirTxn(transaccion, txid_str))?;
    }

    Ok(())
}

/// Se recibe el bloque enviando un mensaje getdata y recibiendo un mensaje block, a partir del inv.
/// Si el bloque recibido es válido, se agrega a la blockchain local.
fn recibir_bloque(
    socket: &mut TcpStream,
    headers: &mut Arc<Mutex<Vec<BlockHeader>>>,
    logger_sender: &Sender<String>,
    headers_archivo: &mut File,
    blocks_path: &String,
    mut bytes_respuesta: Vec<u8>,
    tupla_senders: (&gtk::glib::Sender<GuiEvent>, &Sender<WalletEvent>),
) -> Result<(), RustifyError> {
    let (sender_gui, sender_wallet) = tupla_senders;

    log(
        Lvl::Info(Action::WALLET),
        "Inv de tipo bloque.",
        logger_sender,
    );
    // Reenvio el inv recibido con un mensaje "getdata", ya que quiero recibir el bloque completo.
    let response_message_header = MessageHeader::new("getdata".to_string(), &bytes_respuesta);
    let response_message_header_bytes = response_message_header.as_bytes();
    write_to_node(socket, &response_message_header_bytes, &bytes_respuesta)?;
    log(
        Lvl::Info(Action::WALLET),
        "Enviado mensaje getdata.",
        logger_sender,
    );

    // Recibo el bloque completo mediante un mensaje "block", durante initial block download
    // En este handleo es posible perder mensajes inv de transacciones, que se descartan.
    bytes_respuesta =
        handle_specific_message(socket, "block\0\0\0\0\0\0\0".to_string(), logger_sender)?;

    log(
        Lvl::Info(Action::WALLET),
        "Recibido mensaje block.",
        logger_sender,
    );
    validar_bloque(
        headers,
        logger_sender,
        headers_archivo,
        blocks_path,
        bytes_respuesta.clone(),
        sender_gui,
        sender_wallet,
    )?;
    Ok(())
}

/// Valida el bloque recibido.
/// Si el bloque cumple la POW y la POI, se agrega a la blockchain local.
/// El bloque se descarga a disco (carpeta blocks), y el header a memoria y disco.
fn validar_bloque(
    headers: &mut Arc<Mutex<Vec<BlockHeader>>>,
    logger_sender: &Sender<String>,
    headers_archivo: &mut File,
    blocks_path: &String,
    bytes_respuesta: Vec<u8>,
    sender_gui: &gtk::glib::Sender<GuiEvent>,
    sender_wallet: &Sender<WalletEvent>,
) -> Result<(), RustifyError> {
    let bloque = SerializedBlock::from_bytes(&bytes_respuesta)?;
    let header_bloque = &bloque.block_header;
    if proof_of_work(header_bloque) && proof_of_inclusion(&bloque) {
        log(
            Lvl::Info(Action::POWPOI),
            "El bloque fue aceptado y guardado localmente",
            logger_sender,
        );
        let mut headers_vec = headers.lock()?;
        sender_gui.send(GuiEvent::CargarBloques(
            vec![header_bloque.clone()],
            headers_vec.len() as u32,
        ))?;

        let header_bytes: String = header_bloque
            .as_bytes()
            .to_vec()
            .iter()
            .map(|b| format!("{:02x}", b) + "")
            .collect();
        writeln!(headers_archivo, "{}", header_bytes)?;
        headers_vec.push(header_bloque.clone());

        guardar_bloque_memoria(bytes_respuesta, blocks_path)?;
        sender_wallet.send(WalletEvent::RecibirBloque(bloque))?;
    } else {
        log(
            Lvl::Warning(Action::POWPOI),
            "El bloque no fue aceptado",
            logger_sender,
        );
    }
    Ok(())
}

// NODE UTILS //

///Respondo al PING con el mensaje PONG al instante.
/// Recibe lo necesario del PING para poder crear el cuerpo del mensaje PONG.
pub fn pong(
    bytes_pong_respuesta: &[u8],
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    let pong_message_header = MessageHeader::new("pong".to_string(), bytes_pong_respuesta);
    let pong_message_header_bytes = pong_message_header.as_bytes();

    write_to_node(socket, &pong_message_header_bytes, bytes_pong_respuesta)?;
    log(
        Lvl::Info(Action::NETWORK),
        "Enviado mensaje pong.",
        logger_sender,
    );
    Ok(())
}

/// Escribe un mensaje al nodo.
/// Recibe el socket conectado al nodo, el header del mensaje, y el payload del mensaje.
pub fn write_to_node(
    socket: &mut TcpStream,
    header: &[u8],
    payload: &[u8],
) -> Result<(), RustifyError> {
    let buffer = [header, payload].concat();
    socket.write_all(&buffer)?;
    socket.flush()?;
    Ok(())
}

/// Lee el mensaje recibido desde el nodo.
/// Recibe el socket conectado al nodo, y el largo del mensaje a leer.
/// Devuelve el mensaje recibido como vector.
pub fn read_from_node(
    socket: &mut TcpStream,
    largo_mensaje: usize,
) -> Result<Vec<u8>, RustifyError> {
    let mut buffer = vec![0u8; largo_mensaje];
    socket.read_exact(&mut buffer)?;
    Ok(buffer)
}

/// Envía el mensaje de tipo inv, en base al mensaje de tipo inv pasado por parametro
/// y el nombre del mensaje especificado
pub fn send_inv(command: String, socket: &mut TcpStream, inv: &Inv) -> Result<u64, RustifyError> {
    let cant_inv = &inv.count;
    let getdata_message_bytes = inv.as_bytes();

    let getdata_message_header = MessageHeader::new(command, &getdata_message_bytes);
    let getdata_message_header_bytes = getdata_message_header.as_bytes();

    write_to_node(
        socket,
        &getdata_message_header_bytes,
        &getdata_message_bytes,
    )?;
    Ok(cant_inv.value())
}
