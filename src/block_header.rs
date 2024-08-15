use crate::config::Config;
use crate::errors::RustifyError;
use crate::getheaders::{getheaders, getheaders_loop};
use crate::gui_events::GuiEvent;
use crate::logger::{log, log_with_parameters, Action, Lvl};
use bitcoin_hashes::{sha256d, Hash};
use std::fs::{self, File};
use std::io::{prelude::*, BufReader};
use std::net::TcpStream;
use std::sync::mpsc::Sender;

const TESTNET_GENESIS_HASH: [u8; 32] = [
    0x43, 0x49, 0x7f, 0xd7, 0xf8, 0x26, 0x95, 0x71, 0x08, 0xf4, 0xa3, 0x0f, 0xd9, 0xce, 0xc3, 0xae,
    0xba, 0x79, 0x97, 0x20, 0x84, 0xe9, 0x0e, 0xad, 0x01, 0xea, 0x33, 0x09, 0x00, 0x00, 0x00, 0x00,
];
pub const NULL_HASH: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const TESNET_GENESIS_HEADER: [u8; 80] = [
    0x01, 0x00, 0x00, 0x00, // version
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // previous_block_header_hash
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e, 0x67, 0x76, 0x8f, 0x61,
    // merkle_root_hash
    0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa, 0x4b, 0x1e, 0x5e, 0x4a,
    0xda, 0xe5, 0x49, 0x4d, // time
    0xff, 0xff, 0x00, 0x1d, // n_bits
    0x1a, 0xa4, 0xae, 0x18, // nonce
];

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub version: i32,
    pub previous_block_header_hash: [u8; 32],
    pub merkle_root_hash: [u8; 32],
    pub time: u32,
    pub n_bits: u32,
    pub nonce: u32,
}

impl BlockHeader {
    /// Realiza el parseo de una cadena de 80 bytes al tipo de dato BlockHeader
    pub fn from_bytes(bytes: &[u8]) -> Result<BlockHeader, RustifyError> {
        let version = i32::from_le_bytes(bytes[0..4].try_into()?);

        let mut previous_block_header_hash = [0u8; 32];
        previous_block_header_hash.copy_from_slice(&bytes[4..36]);

        let mut merkle_root_hash = [0u8; 32];
        merkle_root_hash.copy_from_slice(&bytes[36..68]);

        let time = u32::from_le_bytes(bytes[68..72].try_into()?);
        let n_bits = u32::from_le_bytes(bytes[72..76].try_into()?);
        let nonce = u32::from_le_bytes(bytes[76..80].try_into()?);

        Ok(BlockHeader {
            version,
            previous_block_header_hash,
            merkle_root_hash,
            time,
            n_bits,
            nonce,
        })
    }

    ///Obtiene una cadena de 80 bytes en base al struct BlockHeader
    pub fn as_bytes(&self) -> [u8; 80] {
        let mut bytes = [0u8; 80];
        bytes[..4].copy_from_slice(&self.version.to_le_bytes());
        bytes[4..36].copy_from_slice(&self.previous_block_header_hash);
        bytes[36..68].copy_from_slice(&self.merkle_root_hash);
        bytes[68..72].copy_from_slice(&self.time.to_le_bytes());
        bytes[72..76].copy_from_slice(&self.n_bits.to_le_bytes());
        bytes[76..80].copy_from_slice(&self.nonce.to_le_bytes());
        bytes
    }

    /// Obtiene el tiempo directamente de la cadena de bytes, sin realizar
    /// el parseo al tipo de dato BlockHeader
    pub fn obtain_time(raw_header: &[u8]) -> Result<u32, RustifyError> {
        let mut start_string = [0; 4];
        start_string.copy_from_slice(&raw_header[68..72]);
        let timestamp = u32::from_le_bytes(start_string);
        Ok(timestamp)
    }

    // Obtengo el hash del bloque anterior, existente en el header
    pub fn obtain_previous_block_hash(&self) -> [u8; 32] {
        self.previous_block_header_hash
    }
}

/// Descarga los headers faltantes para tener toda la blockchain de headers actualizada.
/// Carga los headers actuales desde el archivo a memoria, y descarga los nuevos headers.
/// Devuelve toda la blockchain de headers completa, almacenada en memoria (vector de headers).
pub fn actualizar_header_blockchain(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
    sender_gui: &gtk::glib::Sender<GuiEvent>,
) -> Result<(Vec<BlockHeader>, usize), RustifyError> {
    sender_gui.send(GuiEvent::ActualizarLabelEstado(
        "Loading local headers...".to_string(),
    ))?;
    let mut headers_archivo = File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(&config.headers_path)?;
    let mut headers: Vec<BlockHeader>;
    let pagina_headers: Vec<Vec<u8>>;
    let mut indice_ultimo_header = 0;
    if fs::metadata(&config.headers_path)?.len() == 0 {
        // Si el archivo esta vacio, cargo de cero (desde el genesis).
        log(
            Lvl::Info(Action::INB),
            "Descargando toda la blockchain de headers mediante el mensaje getheaders...",
            logger_sender,
        );
        pagina_headers = getheaders(
            socket,
            vec![TESTNET_GENESIS_HASH.to_vec()],
            NULL_HASH.to_vec(),
            config,
            logger_sender,
        )?;
        headers = vec![BlockHeader::from_bytes(&TESNET_GENESIS_HEADER)?];
    } else {
        // Si el archivo tiene headers, tomo el ultimo.
        log(
            Lvl::Info(Action::INB),
            "Cargando los headers guardados localmente...",
            logger_sender,
        );
        let ultimo_header_archivo: Vec<u8>;
        (headers, ultimo_header_archivo) = cargar_headers_memoria(&headers_archivo)?;
        let ultimo_hash_archivo = sha256d::Hash::hash(&ultimo_header_archivo)
            .to_byte_array()
            .to_vec();
        indice_ultimo_header = headers.len();

        log(
            Lvl::Info(Action::INB),
            "Descargando nuevos headers mediante el mensaje getheaders...",
            logger_sender,
        );
        pagina_headers = getheaders(
            socket,
            vec![ultimo_hash_archivo],
            NULL_HASH.to_vec(),
            config,
            logger_sender,
        )?;
    }

    sender_gui.send(GuiEvent::ActualizarLabelEstado(
        "Downloading headers...".to_string(),
    ))?;
    getheaders_loop(
        socket,
        &mut headers_archivo,
        &mut headers,
        pagina_headers,
        config,
        logger_sender,
    )?;

    log_with_parameters(
        Lvl::Info(Action::INB),
        format!(
            "INFO: Blockchain de headers actualizada. Total: {}",
            headers.len()
        ),
        logger_sender,
    );

    Ok((headers, indice_ultimo_header))
}

/// Carga los headers guardados en disco (archivo) a memoria (Vec<Vec<u8>>).
/// Traduce cada linea del archivo de hexa a vector en bytes decimales.
/// Devuelve un vector con todos los headers que estaban guardados en el archivo.
fn cargar_headers_memoria(archivo: &File) -> Result<(Vec<BlockHeader>, Vec<u8>), RustifyError> {
    let buf_reader = BufReader::new(archivo);
    let mut headers: Vec<BlockHeader> = vec![BlockHeader::from_bytes(&TESNET_GENESIS_HEADER)?];
    let mut ultima_linea = String::new();
    for linea in buf_reader.lines() {
        let linea_clonada = linea?.clone();
        let header = (0..linea_clonada.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&linea_clonada[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()?;
        let header_struct = BlockHeader::from_bytes(&header)?;
        headers.push(header_struct);
        ultima_linea = linea_clonada;
    }

    let ultimo_header = (0..ultima_linea.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&ultima_linea[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()?;

    Ok((headers, ultimo_header))
}

/// Guarda la pagina de headers descargada en disco y en memoria.
/// Recibe el archivo donde se guardan los headers, y la pagina de headers descargada.
pub fn guardar_headers(
    archivo: &mut File,
    headers: &mut Vec<BlockHeader>,
    pagina_headers: &Vec<Vec<u8>>,
) -> Result<(), RustifyError> {
    for header in pagina_headers {
        // Recorro cada header (vector) y lo transformo a String en hexa.
        let header_bytes: String = header.iter().map(|b| format!("{:02x}", b) + "").collect();
        writeln!(archivo, "{}", header_bytes)?;
        let header_struct = BlockHeader::from_bytes(header)?;
        headers.push(header_struct);
    }
    Ok(())
}
