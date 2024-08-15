use std::{net::TcpStream, sync::mpsc::Sender};

use crate::{
    errors::RustifyError,
    logger::{log_with_parameters, Action, Lvl},
    message_header::MessageHeader,
    node::{pong, read_from_node},
};

const MESSAGE_HEADER_SIZE: usize = 24;

/// Recibe un comando especifico a buscar, ejemplo: busca los blocks e itera los demas mensajes que llegan
/// hasta encontrarlo, utilizando la funcion handle_message.
/// Devuelve el mensaje que se estaba buscando.
pub fn handle_specific_message(
    socket: &mut TcpStream,
    comando_esperado: String,
    logger_sender: &Sender<String>,
) -> Result<Vec<u8>, RustifyError> {
    let mut bytes_header_respuesta = read_from_node(socket, MESSAGE_HEADER_SIZE)?;
    let mut message_header_respuesta = MessageHeader::from_bytes(&bytes_header_respuesta)?;
    let mut bytes_message_respuesta =
        read_from_node(socket, message_header_respuesta.payload_size as usize)?;
    let mut comando = String::from_utf8((message_header_respuesta.command_name).to_vec())?;
    log_with_parameters(
        Lvl::Info(Action::NETWORK),
        format!("Recibido mensaje {}.", comando),
        logger_sender,
    );

    while comando != comando_esperado {
        handle_message(
            &comando,
            &bytes_message_respuesta,
            socket,
            logger_sender,
            &comando_esperado,
        )?;
        bytes_header_respuesta = read_from_node(socket, MESSAGE_HEADER_SIZE)?;
        message_header_respuesta = MessageHeader::from_bytes(&bytes_header_respuesta)?;
        bytes_message_respuesta =
            read_from_node(socket, message_header_respuesta.payload_size as usize)?;
        comando = String::from_utf8(message_header_respuesta.command_name.to_vec())?;
        log_with_parameters(
            Lvl::Info(Action::NETWORK),
            format!("Recibido mensaje {}.", comando),
            logger_sender,
        );
    }

    Ok(bytes_message_respuesta)
}

/// Handleo de mensajes, en esta primer version solo responde el PING con el PONG.
/// No devuelve nada, solo responde y trabaja lo necesario.
pub fn handle_message(
    comando: &str,
    bytes_mensaje_respuesta: &[u8],
    socket: &mut TcpStream,
    logger_sender: &Sender<String>,
    comando_esperado: &str,
) -> Result<(), RustifyError> {
    match comando {
        "ping\0\0\0\0\0\0\0\0" => {
            pong(bytes_mensaje_respuesta, socket, logger_sender)?;
        }
        "tx\0\0\0\0\0\0\0\0\0\0" => {}
        "block\0\0\0\0\0\0\0" => {}
        "notfound\0\0\0\0" => {
            log_notfound_result(socket, comando_esperado, logger_sender);
            match comando_esperado {
                "block\0\0\0\0\0\0\0" => return Err(RustifyError::ElNodoNoEncuentraBloquePedido),
                "tx\0\0\0\0\0\0\0\0\0\0" => {
                    return Err(RustifyError::ElNodoNoEncuentraTransaccionPedida)
                }
                _ => {}
            };
        }
        _ => {}
    }

    Ok(())
}

/// Genera logs en base al tipo de resultado que se estaba esperando
/// (si era un bloque o una transaccion)
fn log_notfound_result(
    socket: &mut TcpStream,
    comando_esperado: &str,
    logger_sender: &Sender<String>,
) {
    let peer_ip = socket.peer_addr();
    let notfound_str = match comando_esperado {
        "block\0\0\0\0\0\0\0" => "el bloque",
        "tx\0\0\0\0\0\0\0\0\0\0" => "la transaccion",
        _ => "lo",
    };
    if let Ok(ip) = peer_ip {
        log_with_parameters(
            Lvl::Warning(Action::NETWORK),
            format!(
                "El nodo con IP {:?} no pudo obtener {} que se solicitó!",
                ip, notfound_str
            ),
            logger_sender,
        );
    } else {
        log_with_parameters(
            Lvl::Warning(Action::NETWORK),
            format!("El nodo no pudo obtener {} que se solicitó!", notfound_str),
            logger_sender,
        );
    }
}
