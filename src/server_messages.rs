use crate::{
    block_header::BlockHeader,
    compactsize::CompactSize,
    config::Config,
    errors::RustifyError,
    getheaders::GetHeadersMessage,
    inv::Inv,
    logger::{log, log_re_err, log_with_parameters, Action, Lvl},
    message_handler::handle_specific_message,
    message_header::MessageHeader,
    node::{read_from_node, send_inv, write_to_node},
    serialized_block::SerializedBlock,
    server_notification::find_txn_in_memory,
    txn::Txn,
    version::VersionMessage,
    wallet_txn::broadcast_txn,
};
use bitcoin_hashes::{sha256d, Hash};
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    net::TcpStream,
    sync::{mpsc::Sender, Arc, Mutex},
};
type TrxServer = Vec<(String, Txn)>;

const MAX_HEADERS_POR_MENSAJE: usize = 2000;
const MSG_TX: usize = 1;
const MSG_BLOCK: usize = 2;
const LIM_MINIMO_INVENTARIO: usize = 5;

/// Recibe los mensajes version y verack, y los contesta.
/// En caso afirmativo, el handshake queda establecido.
pub fn recibir_handshake(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    recibir_version(socket, config, logger_sender)?;
    recibir_verack(socket, logger_sender)?;
    log(
        Lvl::Info(Action::SERVER),
        "Se realizó el handshake con el nodo. Conexión establecida",
        logger_sender,
    );
    Ok(())
}

/// Recibe el mensaje version, y contesta con su propio mensaje version.
fn recibir_version(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    let version_recibido_bytes =
        handle_specific_message(socket, "version\0\0\0\0\0".to_string(), logger_sender)?;
    let _version_recibido = VersionMessage::from_bytes(&version_recibido_bytes)?;

    let version = VersionMessage::new(socket.peer_addr()?, socket.local_addr()?, config);
    let version_bytes = version.as_bytes(&config.user_agent_rustify);

    let version_header = MessageHeader::new("version".to_string(), &version_bytes);
    let version_header_bytes = version_header.as_bytes();

    write_to_node(socket, &version_header_bytes, &version_bytes)?;
    log(
        Lvl::Info(Action::SERVER),
        "Enviado mensaje version",
        logger_sender,
    );

    Ok(())
}

/// Recibe el mensaje verack, y contesta con su propio mensaje verack.
fn recibir_verack(
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    let _verack_header_recibido_bytes =
        handle_specific_message(socket, "verack\0\0\0\0\0\0".to_string(), logger_sender)?;

    let verack_header = MessageHeader::new("verack".to_string(), &[]);
    let verack_header_bytes = verack_header.as_bytes();

    write_to_node(socket, &verack_header_bytes, &[])?;
    log(
        Lvl::Info(Action::SERVER),
        "Enviado mensaje verack.",
        logger_sender,
    );
    Ok(())
}

/// Recibe el mensaje getheaders, y contesta con un mensaje headers.
/// Se envian todos los headers subsiguientes al starting, con un maximo de 2000 headers por mensaje.
/// En caso de que no se haya encontrado ningún starting hash, se envía el mensaje headers vacío (con count 0).
pub fn recibir_getheaders(
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
    message_header: MessageHeader,
    headers: &Arc<Mutex<Vec<BlockHeader>>>,
    headers_hash_height: &Arc<Mutex<HashMap<Vec<u8>, usize>>>,
) -> Result<(), RustifyError> {
    let getheaders_recibido_bytes = read_from_node(socket, message_header.payload_size as usize)?;
    let getheaders_recibido = GetHeadersMessage::from_bytes(&getheaders_recibido_bytes)?;

    actualizar_headers_hash_height(headers_hash_height, headers)?;

    let mut headers_cliente_bytes = vec![];
    let headers_vec = headers.lock()?;
    let headers_hash_height_map = headers_hash_height.lock()?;
    let mut header_count = 0_u64;
    for starting_hash in getheaders_recibido.starting_hashes {
        match headers_hash_height_map.get(&starting_hash) {
            Some(height) => {
                let mut header_index = height + 1; // Para no enviar el starting hash (el cliente ya lo tiene).
                let mut header_hash = starting_hash;
                let len_headers = headers_vec.len();
                let max_headers_index = header_index + MAX_HEADERS_POR_MENSAJE;
                while header_index < len_headers
                    && header_index < max_headers_index
                    && header_hash != getheaders_recibido.stopping_hash
                {
                    let header_bytes = headers_vec[header_index].as_bytes();
                    header_hash = sha256d::Hash::hash(&header_bytes).to_byte_array().to_vec();
                    headers_cliente_bytes.append(&mut header_bytes.to_vec());
                    headers_cliente_bytes.append(&mut vec![0x00]); // Transaction count
                    header_index += 1;
                    header_count += 1;
                }
                break;
            }
            None => continue,
        }
    }

    enviar_headers(socket, logger_sender, headers_cliente_bytes, header_count)?;

    Ok(())
}

/// Envía por el socket los headers pedidos al nodo cliente, incluyendo la cantidad.
fn enviar_headers(
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
    mut headers_cliente_bytes: Vec<u8>,
    header_count: u64,
) -> Result<(), RustifyError> {
    let mut header_count_bytes = CompactSize::new(header_count).as_bytes();
    header_count_bytes.append(&mut headers_cliente_bytes);

    let headers_message_bytes = header_count_bytes;
    let headers_header = MessageHeader::new("headers".to_string(), &headers_message_bytes);
    let headers_header_bytes = headers_header.as_bytes();

    write_to_node(socket, &headers_header_bytes, &headers_message_bytes)?;
    log(
        Lvl::Info(Action::SERVER),
        "Enviado mensaje headers.",
        logger_sender,
    );
    Ok(())
}

/// Actualiza el HashMap de headers, en el caso de que hayan llegado nuevos headers por block broadcasting.
fn actualizar_headers_hash_height(
    headers_hash_height: &Arc<Mutex<HashMap<Vec<u8>, usize>>>,
    headers: &Arc<Mutex<Vec<BlockHeader>>>,
) -> Result<(), RustifyError> {
    let headers_vec = headers.lock()?;
    let mut headers_hash_height_map = headers_hash_height.lock()?;
    let len_hash = headers_hash_height_map.len();

    if headers_vec.len() > len_hash {
        let slice_headers = &headers_vec[len_hash - 1..];
        for (index, header) in slice_headers.iter().enumerate() {
            let header_hash = sha256d::Hash::hash(&header.as_bytes())
                .to_byte_array()
                .to_vec();
            headers_hash_height_map.insert(header_hash, len_hash - 1 + index);
        }
    }

    Ok(())
}

/// Handlea los mensaje getdata recibidos por el cliente y los separa en
/// funcion de si son pedidos de bloques o pedidos de transacciones.
pub fn recibir_getdata(
    txn_memory_client: &Arc<Mutex<TrxServer>>,
    socket: &mut TcpStream,
    message_header: MessageHeader,
    ip_cliente: &String,
    logger_sender: &Sender<String>,
    config: &Config,
) -> Result<(), RustifyError> {
    let getdata_bytes = read_from_node(socket, message_header.payload_size as usize)?;
    let getdata = Inv::from_bytes(&getdata_bytes)?;
    for inventory in getdata.inventories {
        let tipo = inventory[0] as usize;
        match tipo {
            MSG_BLOCK => {
                log_with_parameters(
                    Lvl::Info(Action::SERVER),
                    format!("Recibido pedido de bloque del cliente {}.", ip_cliente),
                    logger_sender,
                );
                match respond_getdata_block(inventory, socket, logger_sender, config) {
                    Ok(_) => log(
                        Lvl::Info(Action::SERVER),
                        "Se respondió exitosamente el pedido de bloque del cliente.",
                        logger_sender,
                    ),
                    Err(e) => log_re_err(Action::SERVER, e, logger_sender),
                };
            }
            MSG_TX => {
                log_with_parameters(
                    Lvl::Info(Action::SERVER),
                    format!(
                        "Recibido pedido de transacciones del cliente {}.",
                        ip_cliente
                    ),
                    logger_sender,
                );
                match respond_getdata_txn(inventory, socket, logger_sender, txn_memory_client) {
                    Ok(_) => log(
                        Lvl::Info(Action::SERVER),
                        "Se respondió exitosamente el pedido de transacciones del cliente.",
                        logger_sender,
                    ),
                    Err(e) => log_re_err(Action::SERVER, e, logger_sender),
                };
            }
            _ => {}
        };
    }

    Ok(())
}

/// Responde al pedido del bloque del cliente. Si el bloque esta en disco
/// se lo enviara al cliente, caso contrario, se enviara un notfound
fn respond_getdata_block(
    inventory: Vec<u8>,
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
    config: &Config,
) -> Result<(), RustifyError> {
    //Esta validacion es para evitar que inventarios fallados afecten al codigo
    if inventory.len() < LIM_MINIMO_INVENTARIO {
        return Err(RustifyError::NoSeEncontroBloquePedidoPorCliente);
    }
    let possible_block = inventory[4..].to_vec();
    let filename = SerializedBlock::obtain_blockname_from_blockhash(possible_block);
    let path = format!("{}/{}.txt", config.blocks_path, filename);

    let mut archivo_bloque = match File::options()
        .read(true)
        .write(false)
        .create(false)
        .open(path)
    {
        Ok(block) => block,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                send_not_found(socket, inventory, logger_sender);
                return Err(RustifyError::NoSeEncontroBloquePedidoPorCliente);
            } else {
                return Ok(());
            }
        }
    };
    let mut buffer = Vec::<u8>::new();
    archivo_bloque.read_to_end(&mut buffer)?;

    send_block(socket, buffer)?;

    Ok(())
}

/// Responde al pedido de transaccion del cliente. Si la transaccion esta guardada en
/// el vector de txn de memoria (txn_memory),
/// se la enviara al cliente, caso contrario, se enviara un notfound
fn respond_getdata_txn(
    inventory: Vec<u8>,
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
    txn_memory_client: &Arc<Mutex<TrxServer>>,
) -> Result<(), RustifyError> {
    //Esta validacion es para evitar que inventarios fallados afecten al codigo
    if inventory.len() < LIM_MINIMO_INVENTARIO {
        return Err(RustifyError::NoSeEncontroTransaccionPedidaPorCliente);
    }
    let possible_txn = inventory[4..].to_vec();
    let possible_txid = Txn::obtain_txid_from_inventory(possible_txn);
    if let Ok(op_txn) = find_txn_in_memory(txn_memory_client, &possible_txid, logger_sender) {
        match op_txn {
            Some((txid, txn)) => {
                broadcast_txn(&txn, socket)?;
                log_with_parameters(
                    Lvl::Info(Action::SERVER),
                    format!("Se envio transaccion {} al cliente", txid),
                    logger_sender,
                );
            }
            None => {
                send_not_found(socket, inventory, logger_sender);
                return Err(RustifyError::NoSeEncontroTransaccionPedidaPorCliente);
            }
        }
    }

    Ok(())
}

/// Envia al nodo cliente el mensaje de tipo Not Found
/// En esta solución se propone el envio de un unico elemento
/// en el inventario de este mensaje
fn send_not_found(socket: &mut TcpStream, inventory: Vec<u8>, logger_sender: &Sender<String>) {
    let inv = Inv::new(1, MSG_BLOCK as u32, vec![inventory]);
    if send_inv("notfound".to_owned(), socket, &inv).is_ok() {
        log(
            Lvl::Info(Action::SERVER),
            "Se envió al cliente el mensaje notfound",
            logger_sender,
        );
    }
}

/// Envia al nodo cliente un bloque previamente solicitado
fn send_block(socket: &mut TcpStream, block_message_bytes: Vec<u8>) -> Result<(), RustifyError> {
    let block_message_header = MessageHeader::new("block".to_owned(), &block_message_bytes);
    let block_message_header_bytes = block_message_header.as_bytes();
    write_to_node(socket, &block_message_header_bytes, &block_message_bytes)?;
    Ok(())
}
