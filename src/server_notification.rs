use std::{
    collections::HashMap,
    net::TcpStream,
    sync::{mpsc::Sender, Arc, Mutex},
};

use crate::{
    config::Config,
    errors::RustifyError,
    inv::Inv,
    logger::{log, log_with_parameters, Action, Lvl},
    node::send_inv,
    txn::Txn,
};

type TrxServer = Vec<(String, Txn)>;

/// Se genera un nuevo proceso (uno para todos los clientes) para transmitir
/// Invs que se encuentren en el channel de notificaciones, recibiendo desde el listener.
/// De ocurrir algun error (entre los que se incluye tambien que se haya caido la conexion del cliente)
/// el mismo sera retirado del vector de conexiones, y ya no se le notificara mas nada
pub fn envio_notificaciones_cliente(
    client_connections: Arc<Mutex<HashMap<String, TcpStream>>>,
    logger_sender: Sender<String>,
    recv_notif: std::sync::mpsc::Receiver<Inv>,
) -> Result<(), RustifyError> {
    for inv in recv_notif {
        let mut conexiones_cliente = client_connections.lock()?;

        let mut clientes_caidos = vec![];

        for (addr, socket) in conexiones_cliente.iter_mut() {
            match send_inv("inv".to_owned(), socket, &inv) {
                Ok(_) => {
                    log(
                        Lvl::Info(Action::SERVER),
                        "Se envía inv al cliente",
                        &logger_sender,
                    );
                }
                Err(_) => {
                    log_with_parameters(
                        Lvl::Warning(Action::SERVER),
                        format!("Se desconectó al cliente de IP {}", addr),
                        &logger_sender,
                    );
                    clientes_caidos.push(addr.clone());
                    continue;
                }
            }
        }

        for addr in clientes_caidos {
            conexiones_cliente.remove(&addr);
        }
    }

    Ok(())
}

/// Añade una transaccion en memoria, para que luego el servidor pueda
/// enviarla, en caso de ser solicitada
pub fn add_txn_in_memory(
    txn_memory_server: &mut Arc<Mutex<TrxServer>>,
    transaccion: &Txn,
    txid_str: &String,
    config: &Config,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    let mut txn_memory = txn_memory_server.lock()?;

    if txn_memory.len() == config.cant_max_txn_memoria {
        txn_memory.remove(0);
    }

    txn_memory.push((txid_str.to_string(), transaccion.clone()));
    log(
        Lvl::Info(Action::SERVER),
        "Se guarda transaccion en memoria",
        logger_sender,
    );

    Ok(())
}

/// Busca en el vector de ultimas transacciones guardadas en memoria para ver si
/// coincide con la solicitada por el cliente
pub fn find_txn_in_memory(
    txn_memory_server: &Arc<Mutex<TrxServer>>,
    txid_str: &String,
    logger_sender: &Sender<String>,
) -> Result<Option<(String, Txn)>, RustifyError> {
    let txn_memory = txn_memory_server.lock()?;
    for i in 0..txn_memory.len() {
        if &txn_memory[i].0 == txid_str {
            log(
                Lvl::Info(Action::SERVER),
                "Se envia transaccion al cliente",
                logger_sender,
            );
            return Ok(Some((txn_memory[i].0.clone(), txn_memory[i].1.clone())));
        }
    }
    Ok(None)
}
