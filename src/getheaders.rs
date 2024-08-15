use crate::block_header::{guardar_headers, BlockHeader, NULL_HASH};
use crate::compactsize::CompactSize;
use crate::config::Config;
use crate::errors::RustifyError;
use crate::message_handler::handle_specific_message;
use crate::message_header::MessageHeader;
use crate::node::write_to_node;
use bitcoin_hashes::{sha256d, Hash};
use std::fs::File;
use std::net::TcpStream;
use std::sync::mpsc::Sender;

const HASH_LENGTH: usize = 32;

#[derive(Debug)]
pub struct GetHeadersMessage {
    pub version: u32,
    pub hash_count: CompactSize,
    pub starting_hashes: Vec<Vec<u8>>,
    pub stopping_hash: Vec<u8>,
}

impl GetHeadersMessage {
    pub fn new(
        starting_hashes: Vec<Vec<u8>>,
        stopping_hash: Vec<u8>,
        config: &Config,
    ) -> GetHeadersMessage {
        GetHeadersMessage {
            version: config.version as u32,
            hash_count: CompactSize::new(starting_hashes.len() as u64),
            starting_hashes,
            stopping_hash,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes_getheaders: Vec<u8> = vec![];

        bytes_getheaders.append(&mut self.version.to_le_bytes().to_vec());
        bytes_getheaders.append(&mut self.hash_count.as_bytes());
        for starting_hash in &self.starting_hashes {
            bytes_getheaders.append(&mut starting_hash.clone());
        }
        bytes_getheaders.append(&mut self.stopping_hash.clone());

        bytes_getheaders
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<GetHeadersMessage, RustifyError> {
        let version = u32::from_le_bytes(bytes[0..4].try_into()?);
        let (hash_count, index) = CompactSize::parse_from_byte_array(&bytes[4..14]);

        let mut starting_hashes = vec![];
        let mut start_index = 4 + index;
        for _i in 0..hash_count.value() {
            let mut starting_hash = vec![0; HASH_LENGTH];
            starting_hash.copy_from_slice(&bytes[start_index..start_index + HASH_LENGTH]);
            starting_hashes.push(starting_hash);
            start_index += HASH_LENGTH;
        }

        let mut stopping_hash = vec![0; HASH_LENGTH];
        stopping_hash.copy_from_slice(&bytes[start_index..start_index + HASH_LENGTH]);

        Ok(GetHeadersMessage {
            version,
            hash_count,
            starting_hashes,
            stopping_hash,
        })
    }
}

/// Ciclo de mensajes GETHEADERS, manda mensajes hasta tener toda la blockchain de headers descargada.
/// Recibe el socket al nodo conectado, el archivo de headers, el vector de headers, y la pagina actual de headers descargada.
/// Actualiza el vector de headers y el archivo de headers. Los deja con toda la blockchain descargada.
pub fn getheaders_loop(
    socket: &mut TcpStream,
    headers_archivo: &mut File,
    headers: &mut Vec<BlockHeader>,
    mut pagina_headers: Vec<Vec<u8>>,
    config: &Config,
    sender: &Sender<String>,
) -> Result<(), RustifyError> {
    while pagina_headers.len() == 2000 {
        guardar_headers(headers_archivo, headers, &pagina_headers)?;
        let ultimo_hash_pagina = sha256d::Hash::hash(&pagina_headers.pop().unwrap())
            .to_byte_array()
            .to_vec();
        pagina_headers = getheaders(
            socket,
            vec![ultimo_hash_pagina],
            NULL_HASH.to_vec(),
            config,
            sender,
        )?;
    }
    guardar_headers(headers_archivo, headers, &pagina_headers)?;

    Ok(())
}

/// Mensaje GETHEADERS.
/// Devuelve todos los headers posteriores al starting_hash, y previos al stopping_hash.
/// Si stopping_hash es el vector nulo, se devuelven todos los headers posteriores que se encuentren o un máximo de 2000 (lo que ocurra primero).
pub fn getheaders(
    socket: &mut TcpStream,
    starting_hash: Vec<Vec<u8>>,
    stopping_hash: Vec<u8>,
    config: &Config,
    sender: &Sender<String>,
) -> Result<Vec<Vec<u8>>, RustifyError> {
    let getheaders_message = GetHeadersMessage::new(starting_hash, stopping_hash, config);
    let getheaders_message_bytes = getheaders_message.as_bytes();

    let getheaders_message_header =
        MessageHeader::new("getheaders".to_string(), &getheaders_message_bytes);
    let getheaders_message_header_bytes = getheaders_message_header.as_bytes();

    write_to_node(
        socket,
        &getheaders_message_header_bytes,
        &getheaders_message_bytes,
    )?;

    let bytes_getheaders_respuesta =
        handle_specific_message(socket, "headers\0\0\0\0\0".to_string(), sender)?;

    // Proceso el hash_count (tipo compactsize) para luego hacer el slice y removerlo del mensaje.
    let hashcount_compactsize = CompactSize::parse_from_byte_array(&bytes_getheaders_respuesta).1;
    // Corto cada 81 bytes y en el map remuevo el ultimo byte de c/u (remuevo el Transaction count (0x00))
    let headers: Vec<Vec<u8>> = bytes_getheaders_respuesta[hashcount_compactsize..]
        .chunks(81)
        .map(|x| x[0..x.len() - 1].to_vec())
        .collect();

    Ok(headers)
}
