use crate::{
    config::Config,
    errors::RustifyError,
    logger::{log, log_with_parameters, Action, Lvl},
    serialized_block::SerializedBlock,
    txn::Txn,
};
use std::{
    collections::HashMap,
    fs::{self, DirEntry, File, ReadDir},
    io::Read,
    sync::mpsc::Sender,
};

//Tipo de dato de Hashmap de transacción
type TrxKey = (String, u32);
type TrxHashMap<T> = HashMap<TrxKey, T>;

/// Obtiene listado de UTXOs realizando los siguientes pasos:
/// 1) Lee los bloques descargados (se considera precondición que la descarga de bloques fue exitosa)
/// y los parsea en las estructuras propias de su constitución
/// 2) Procesa todos los bloques parseados para obtener un hashmap con la información relevante para
/// matchear (TXID, output_index)
/// 3) Realiza el matcheo en sí
///
/// Extra: Nosotros consideramos esta cuenta como valida
/// OUTPUTS_TOTAL - (INPUTS_TOTAL - INPUTS_SIN_MATCH) = UTXO
pub fn obtain_utxo(
    config: &Config,
    logger_sender: &Sender<String>,
) -> Result<TrxHashMap<Txn>, RustifyError> {
    let now = std::time::Instant::now();
    log(
        Lvl::Info(Action::UTXO),
        "Ha iniciado el proceso de obtención de UTXOs",
        logger_sender,
    );
    //Esto evita que si llega un bloque nuevo justo entre las linea 42 y 43, se calcule mal las UTXOs
    //No afecta en terminos de memoria
    let iter_bloques_input = fs::read_dir(&config.blocks_path)?;
    let iter_bloques_utxos = fs::read_dir(&config.blocks_path)?;

    let inputs = obtain_inputs(iter_bloques_input)?;
    let utxos = obtain_utxos_from(inputs, iter_bloques_utxos, logger_sender)?;

    log_with_parameters(
        Lvl::Info(Action::UTXO),
        format!("Cantidad de UTXOs: {}", utxos.len()),
        logger_sender,
    );
    log_with_parameters(
        Lvl::Info(Action::UTXO),
        format!(
            "Se tardó {} segundos en obtener el set de UTXOs.",
            now.elapsed().as_secs()
        ),
        logger_sender,
    );

    Ok(utxos)
}

/// Realiza el procedimiento de obtencion de outputs y matcheo con los inputs
/// para asi obtener finalmente las UTXO
fn obtain_utxos_from(
    mut inputs: TrxHashMap<()>,
    dir_blocks: ReadDir,
    logger_sender: &Sender<String>,
) -> Result<TrxHashMap<Txn>, RustifyError> {
    let mut buffer: Vec<u8>;
    let mut utxos: TrxHashMap<Txn> = HashMap::new();

    for entry in dir_blocks {
        buffer = obtener_buffer(entry?)?;
        let block = obtener_block_de_buffer(buffer)?;

        for tx_index in 0..block.txns.len() {
            let txid = Txn::obtain_tx_id(block.txns[tx_index].as_bytes());
            for output_index in 0..block.txns[tx_index].tx_out.len() {
                let key = (txid.clone(), output_index as u32);
                match inputs.get(&key) {
                    Some(_input) => {
                        inputs.remove(&key);
                    }
                    None => {
                        utxos.insert(key, block.txns[tx_index].clone());
                    }
                }
            }
        }
    }
    log_with_parameters(Lvl::Warning(Action::UTXO), format!("Hay {} inputs que no se pudieron matchear con ningun output (por el corte de la fecha que realizamos).", inputs.len()), logger_sender);

    Ok(utxos)
}

/// Obtiene los inputs de todos los bloques guardados en disco
fn obtain_inputs(dir_blocks: ReadDir) -> Result<TrxHashMap<()>, RustifyError> {
    let mut buffer: Vec<u8>;
    let mut inputs: TrxHashMap<()> = HashMap::new();

    for entry in dir_blocks {
        buffer = obtener_buffer(entry?)?;
        let block = obtener_block_de_buffer(buffer)?;

        for tx_index in 0..block.txns.len() {
            for input_index in 0..block.txns[tx_index].tx_in.len() {
                let previous_output =
                    block.txns[tx_index].tx_in[input_index].obtain_tx_id_of_previous_output();
                inputs.insert(previous_output, ());
            }
        }
    }

    Ok(inputs)
}

fn obtener_block_de_buffer(buffer: Vec<u8>) -> Result<SerializedBlock, RustifyError> {
    let block = SerializedBlock::from_bytes(&buffer)?;
    Ok(block)
}

/// Abre un archivo en el directorio de bloques y lee todo
/// su contenido
fn obtener_buffer(entry: DirEntry) -> Result<Vec<u8>, RustifyError> {
    let mut archivo_bloque = File::options()
        .read(true)
        .write(false)
        .create(false)
        .open(entry.path())?;
    let mut buffer = Vec::<u8>::new();
    archivo_bloque.read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Actualiza la lista de utxos dado un nuevo bloque recibido
/// durante la ejecución de la wallet
pub fn update_utxo(
    mut utxos: TrxHashMap<Txn>,
    logger_sender: &Sender<String>,
    new_block: &SerializedBlock,
) -> Result<TrxHashMap<Txn>, RustifyError> {
    let now = std::time::Instant::now();
    let mut inputs_s_matchear = 0;
    let mut spent_utxo: TrxHashMap<()> = HashMap::new();
    let mut cant_outputs = 0;

    //agregado de nuevos outputs y creacion de inputs
    let mut inputs = HashMap::new();
    for tx_index in 0..new_block.txns.len() {
        let txid = Txn::obtain_tx_id(new_block.txns[tx_index].as_bytes());

        for input_index in 0..new_block.txns[tx_index].tx_in.len() {
            let previous_output =
                new_block.txns[tx_index].tx_in[input_index].obtain_tx_id_of_previous_output();
            inputs.insert(previous_output, ());
        }

        //Los outputs ahora son nuevos UTXOs
        for output_index in 0..new_block.txns[tx_index].tx_out.len() {
            let key = (txid.clone(), output_index as u32);
            utxos.insert(key, new_block.txns[tx_index].clone());
            cant_outputs += 1;
        }
    }

    //Se matchean las utxo gastadas
    for input in inputs.keys() {
        match utxos.get(input) {
            Some(_k) => {
                //Una UTXO se transforma en SPENT
                spent_utxo.insert(input.clone(), ());
            }
            None => {
                inputs_s_matchear += 1;
            }
        };
    }
    //Se eliminan Spent utxos
    for key in spent_utxo.keys() {
        utxos.remove(key);
    }

    log_with_parameters(
        Lvl::Info(Action::UTXO),
        format!(
            "INFO: {} nuevas UTXO y {} UTXOs se gastaron",
            cant_outputs,
            spent_utxo.len()
        ),
        logger_sender,
    );

    log_with_parameters(
        Lvl::Info(Action::UTXO),
        format!(
            "INFO: Se tardó {} milisegundos en actualizar el set de UTXOs.",
            now.elapsed().as_millis()
        ),
        logger_sender,
    );

    if inputs_s_matchear != 0 {
        log_with_parameters(
            Lvl::Warning(Action::UTXO),
            format!(
                "Hay {} inputs sin matchear en el bloque nuevo, por el corte de fecha realizado",
                inputs_s_matchear
            ),
            logger_sender,
        );
    }

    Ok(utxos)
}
