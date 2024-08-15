use crate::block_header::BlockHeader;
use crate::config::Config;
use crate::errors::RustifyError;
use crate::inv::Inv;
use crate::logger::{log, log_with_parameters, Action, Lvl};
use crate::message_handler::handle_specific_message;
use crate::message_header::MessageHeader;
use crate::node::write_to_node;
use crate::serialized_block::SerializedBlock;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::Sender;

const MSG_BLOCK: u32 = 2;

/// Revisa el vector de headers (que debe ser el que cumple la condición
/// temporal de comienzo del tp) y realiza:
/// 1) Envía el mensaje getdata con un pedido de bloque por archivo
/// 2) Se queda esperando a recibir el bloque y lo guarda en un archivo especifico de bloque
pub fn block_download(
    socket: &mut TcpStream,
    header: BlockHeader,
    block_path: String,
    cant_block_for_inv: u32,
    sender: &Sender<String>,
) -> Result<(), RustifyError> {
    getdata(
        socket,
        cant_block_for_inv,
        vec![BlockHeader::as_bytes(&header).to_vec()],
    )?;
    receive_block_data(socket, block_path, sender)?;
    Ok(())
}

/// Guarda un archivo por bloque en el directorio blocks.
/// Si no existe el mismo, lo genera
pub fn guardar_bloque_memoria(
    bytes_block: Vec<u8>,
    blocks_path: &String,
) -> Result<(), RustifyError> {
    let id: String = SerializedBlock::obtain_name_for_blockfile(&bytes_block);

    fs::create_dir_all(blocks_path)?;

    let mut archivo_bloque = File::options()
        .read(false)
        .write(true)
        .create(true)
        .open(format!("{}/{}.txt", blocks_path, id))?;
    archivo_bloque.write_all(&bytes_block)?;
    archivo_bloque.flush()?;
    Ok(())
}

/// Lee todos los archivos de bloques existentes en la carpeta blocks
/// Si no encuentra la carpeta devuelve error
/// Nota: Esta funcion toma como precondicion que todos los bloques
/// ya descargados son los que corresponde procesar. No vuelve a validar
/// contra los headers validos para la fecha establecida
pub fn leer_bloque_memoria(config: &Config) -> Result<Vec<SerializedBlock>, RustifyError> {
    // Obtiene tira de 80 bytes y hashea
    let mut vector_bloques: Vec<SerializedBlock> = vec![];
    let mut block: SerializedBlock;

    for entry in fs::read_dir(&config.blocks_path)? {
        let entry = entry?;
        let mut archivo_bloque = File::options()
            .read(true)
            .write(false)
            .create(false)
            .open(entry.path())?;
        let mut buffer = Vec::<u8>::new();
        archivo_bloque.read_to_end(&mut buffer)?;

        block = SerializedBlock::from_bytes(&buffer)?;

        vector_bloques.push(block);
    }
    Ok(vector_bloques)
}

/// Realiza una espera hasta obtener el mensaje block como respuesta al getdata
/// Cuando ocurre esto, llama a la funcion de guardado de bloque
fn receive_block_data(
    socket: &mut TcpStream,
    block_path: String,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    match &handle_specific_message(socket, "block\0\0\0\0\0\0\0".to_string(), logger_sender) {
        Ok(handled_bytes_headers_respuesta) => {
            guardar_bloque_memoria(handled_bytes_headers_respuesta.to_vec(), &block_path)?;
            log(
                Lvl::Info(Action::INB),
                "Se guardó bloque en disco",
                logger_sender,
            );
        }
        Err(e) => {
            if e == &RustifyError::ElNodoNoEncuentraBloquePedido {
                log(
                    Lvl::Info(Action::INB),
                    "Se prosigue con la descarga de otro bloque",
                    logger_sender,
                );
            } else {
                log_with_parameters(
                    Lvl::Warning(Action::INB),
                    format!("Se obtiene el error {:?} esperando a los bloques", e),
                    logger_sender,
                );
            }
        }
    };
    Ok(())
}

/// Determina la cantidad de bloques a leer desde el header más reciente
/// Reviso todo el vector de headers, para procesar solo aquellos que correspondan segun la fecha
pub fn obtener_headers_validos_fecha(
    config: &Config,
    headers: &[BlockHeader],
    indice_ultimo_header_descargado: usize,
) -> Vec<BlockHeader> {
    let mut indice_primer_header_a_descargar = 0;
    for (i, header) in headers.iter().enumerate() {
        if header.time >= config.timestamp_bloque_inicial {
            indice_primer_header_a_descargar = i;
            if indice_ultimo_header_descargado >= indice_primer_header_a_descargar {
                indice_primer_header_a_descargar = indice_ultimo_header_descargado;
            }
            break;
        }
    }
    headers[indice_primer_header_a_descargar..].to_vec()
}

/// Envía el mensaje getdata, en base a uno o varios headers pasados por parametro
fn getdata(
    socket: &mut TcpStream,
    cant_elem_en_inv: u32,
    headers: Vec<Vec<u8>>,
) -> Result<(), RustifyError> {
    let cantidad_headers_fecha = cant_elem_en_inv as usize;
    let cantidad_total_headers = headers.len();
    let getdata_message = Inv::new(
        cant_elem_en_inv,
        MSG_BLOCK,
        headers[cantidad_total_headers - cantidad_headers_fecha..cantidad_total_headers].to_vec(),
    );

    let getdata_message_bytes = getdata_message.as_bytes();

    let getdata_message_header = MessageHeader::new("getdata".to_string(), &getdata_message_bytes);
    let getdata_message_header_bytes = getdata_message_header.as_bytes();

    write_to_node(
        socket,
        &getdata_message_header_bytes,
        &getdata_message_bytes,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::block_header::{self};
    use crate::errors::RustifyError;
    use bitcoin_hashes::{sha256d, Hash};

    #[test]
    fn test_block_hash_conversion() -> Result<(), RustifyError> {
        let header_hash_bytes: Vec<u8> = [
            0x01, 0x00, 0x00, 0x00, 0xbc, 0x74, 0x9f, 0xb3, 0x77, 0xc9, 0xa9, 0x37, 0x70, 0x2b,
            0xd4, 0xff, 0x35, 0xd3, 0x76, 0xb4, 0x3f, 0x5b, 0xc5, 0x67, 0x26, 0x02, 0x9a, 0x93,
            0xa3, 0x8e, 0x82, 0x32, 0x00, 0x00, 0x00, 0x00, 0x2b, 0xad, 0x84, 0x75, 0xb3, 0x8c,
            0xcd, 0x82, 0xeb, 0x98, 0x37, 0x71, 0x32, 0x45, 0x3c, 0x72, 0x70, 0x6a, 0xe9, 0x62,
            0x6d, 0xe3, 0xf9, 0x13, 0xe8, 0xda, 0x2c, 0xca, 0x67, 0x95, 0x48, 0x11, 0x66, 0x01,
            0x4a, 0x4d, 0xff, 0xff, 0x00, 0x1d, 0x05, 0x94, 0xc6, 0x1b,
        ]
        .to_vec();
        let primer_block_header = sha256d::Hash::hash(&header_hash_bytes)
            .to_byte_array()
            .to_vec();
        let segundo_header_hash = block_header::BlockHeader::from_bytes(
            &[
                0x01, 0x00, 0x00, 0x00, 0xa0, 0x23, 0x49, 0x92, 0x4e, 0xa3, 0x93, 0x90, 0x6e, 0x7f,
                0xe3, 0xff, 0xc2, 0xb9, 0xd1, 0x52, 0xfd, 0xf5, 0x5b, 0xcc, 0x1b, 0xac, 0xe5, 0x25,
                0x15, 0x17, 0x16, 0x81, 0x00, 0x00, 0x00, 0x00, 0x44, 0x8e, 0x52, 0xc8, 0x90, 0x2a,
                0xbd, 0x28, 0xf8, 0x2f, 0x3f, 0xf5, 0xee, 0xc8, 0xc9, 0x7a, 0xb1, 0x6b, 0x71, 0xb6,
                0x16, 0x74, 0x71, 0x3c, 0xbc, 0x4f, 0x9a, 0x89, 0x6f, 0x73, 0x3b, 0x5b, 0xa5, 0x01,
                0x4a, 0x4d, 0xff, 0xff, 0x00, 0x1d, 0x05, 0xc0, 0x9a, 0xf9,
            ]
            .to_vec(),
        )?;
        assert_eq!(
            primer_block_header,
            segundo_header_hash.obtain_previous_block_hash()
        );
        Ok(())
    }
}
