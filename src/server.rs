use bitcoin_hashes::{sha256d, Hash};

use crate::{
    block_header::BlockHeader,
    config::Config,
    errors::RustifyError,
    inv::Inv,
    logger::{log, log_with_parameters, Action, Lvl},
    message_header::{MessageHeader, MESSAGE_HEADER_SIZE},
    node::read_from_node,
    server_messages::{recibir_getdata, recibir_getheaders, recibir_handshake},
    server_notification::envio_notificaciones_cliente,
    txn::Txn,
};
use std::{
    collections::HashMap,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{mpsc::Sender, Arc, Mutex},
    thread,
};
type TrxServer = Vec<(String, Txn)>;

/// Inicia la instancia del servidor donde el nodo recibirá conexiones entrantes de otros nodos.
/// Crea un TcpListener que queda a la espera de nuevas conexiones.
/// Cada nueva conexion se handlea en handlear_cliente().
pub fn iniciar_server(
    config: &Config,
    logger_sender: &Sender<String>,
    headers: Arc<Mutex<Vec<BlockHeader>>>,
    txn_memory_client: Arc<Mutex<TrxServer>>,
    recv_notif: std::sync::mpsc::Receiver<Inv>,
) {
    log(
        Lvl::Info(Action::SERVER),
        "Iniciando servidor",
        logger_sender,
    );
    let config_clone = config.clone();
    let logger_sender_listener = logger_sender.clone();
    let client_conections: Arc<Mutex<HashMap<String, TcpStream>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let client_connections_notif = Arc::clone(&client_conections);

    thread::spawn(move || -> Result<(), RustifyError> {
        let headers_hash_height = Arc::new(Mutex::new(obtener_hash_height_headers(&headers)?));
        let listener = TcpListener::bind(&config_clone.server_address)?;
        log(
            Lvl::Info(Action::SERVER),
            "Servidor iniciado",
            &logger_sender_listener,
        );
        loop {
            match listener.accept() {
                Ok((socket, addr)) => {
                    agregar_cliente_en_vector_conexiones(&client_conections, &socket, &addr)?;
                    conectar_cliente(
                        socket,
                        addr,
                        &config_clone,
                        &logger_sender_listener,
                        headers.clone(),
                        headers_hash_height.clone(),
                        txn_memory_client.clone(),
                    )
                }
                Err(e) => {
                    log(
                        Lvl::Error(Action::SERVER),
                        "No se pudo conectar al cliente.",
                        &logger_sender_listener,
                    );
                    return Err(e.into());
                }
            }
        }
    });

    let logger_sender_notif = Sender::clone(logger_sender);
    thread::spawn(move || -> Result<(), RustifyError> {
        envio_notificaciones_cliente(client_connections_notif, logger_sender_notif, recv_notif)?;
        Ok(())
    });
}

/// Establece la conexión con el cliente realizando un handshake.
/// Se crea un thread por cada nuevo cliente.
fn conectar_cliente(
    socket: TcpStream,
    addr: SocketAddr,
    config: &Config,
    logger_sender: &Sender<String>,
    headers: Arc<Mutex<Vec<BlockHeader>>>,
    headers_hash_height: Arc<Mutex<HashMap<Vec<u8>, usize>>>,
    txn_memory_client: Arc<Mutex<TrxServer>>,
) {
    let config_clone = config.clone();
    let logger_sender_clone = logger_sender.clone();
    thread::spawn(move || -> Result<(), RustifyError> {
        let mut socket_clone = socket.try_clone()?;
        log_with_parameters(
            Lvl::Info(Action::SERVER),
            format!("Nuevo cliente con IP {}.", &addr.to_string()),
            &logger_sender_clone,
        );
        match recibir_handshake(&mut socket_clone, &config_clone, &logger_sender_clone) {
            Ok(()) => {}
            Err(e) => {
                log(
                    Lvl::Error(Action::SERVER),
                    "No se pudo realizar el handshake con el cliente.",
                    &logger_sender_clone,
                );
                return Err(e);
            }
        };

        // Mandar ping y si no lo contesta en X tiempo, dropear la conexión.
        handlear_peticiones_cliente(
            &mut socket_clone,
            &logger_sender_clone,
            headers,
            headers_hash_height,
            &addr.to_string(),
            txn_memory_client,
            &config_clone,
        )?;
        Ok(())
    });
}

/// Recibe las peticiones del cliente y las maneja acordemente.
fn handlear_peticiones_cliente(
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
    headers: Arc<Mutex<Vec<BlockHeader>>>,
    headers_hash_height: Arc<Mutex<HashMap<Vec<u8>, usize>>>,
    ip_cliente: &String,
    txn_memory_client: Arc<Mutex<TrxServer>>,
    config: &Config,
) -> Result<(), RustifyError> {
    while let Ok((comando, message_header)) = leer_peticion_cliente(socket) {
        match &comando as &str {
            "getheaders\0\0" => {
                log_with_parameters(
                    Lvl::Info(Action::SERVER),
                    format!("Recibido mensaje {} de cliente {}.", comando, ip_cliente),
                    logger_sender,
                );
                recibir_getheaders(
                    socket,
                    logger_sender,
                    message_header,
                    &headers,
                    &headers_hash_height,
                )?;
            }
            "getdata\0\0\0\0\0" => {
                recibir_getdata(
                    &txn_memory_client,
                    socket,
                    message_header,
                    ip_cliente,
                    logger_sender,
                    config,
                )?;
            }
            _ => log_with_parameters(
                Lvl::Info(Action::SERVER),
                format!("Mensaje {} ignorado.", comando),
                logger_sender,
            ),
        }
    }

    Ok(())
}

/// Lee la petición del cliente y devuelve el header y nombre del mensaje recibido.
fn leer_peticion_cliente(socket: &mut TcpStream) -> Result<(String, MessageHeader), RustifyError> {
    let bytes_header_respuesta = read_from_node(socket, MESSAGE_HEADER_SIZE)?;
    let message_header_respuesta = MessageHeader::from_bytes(&bytes_header_respuesta)?;
    let comando = String::from_utf8((message_header_respuesta.command_name).to_vec())?;
    Ok((comando, message_header_respuesta))
}

/// Genera un HashMap que tiene como clave al hash del BlockHeader y como valor a la height de ese BlockHeader.
fn obtener_hash_height_headers(
    headers: &Arc<Mutex<Vec<BlockHeader>>>,
) -> Result<HashMap<Vec<u8>, usize>, RustifyError> {
    let mut headers_hash_height: HashMap<Vec<u8>, usize> = HashMap::new();

    let headers_vec = headers.lock()?;
    for (i, header) in headers_vec.iter().enumerate() {
        let header_hash = sha256d::Hash::hash(&header.as_bytes())
            .to_byte_array()
            .to_vec();
        headers_hash_height.insert(header_hash, i);
    }

    Ok(headers_hash_height)
}

/// Agrega en el vector de clientes conectados a uno nuevo, siempre y cuando no se encontrara
/// ya en el vector
fn agregar_cliente_en_vector_conexiones(
    client_connections: &Arc<Mutex<HashMap<String, TcpStream>>>,
    socket: &TcpStream,
    addr: &SocketAddr,
) -> Result<(), RustifyError> {
    let mut vector_clientes = client_connections.lock()?;

    match vector_clientes.get(&addr.to_string()) {
        Some(_) => {}
        None => {
            vector_clientes.insert(addr.to_string(), socket.try_clone()?);
        }
    };
    Ok(())
}
