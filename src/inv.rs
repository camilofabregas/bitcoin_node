use crate::compactsize::CompactSize;
use crate::errors::RustifyError;
use bitcoin_hashes::{sha256d, Hash};

#[derive(Debug, Clone)]
pub struct Inv {
    pub count: CompactSize,
    pub inventories: Vec<Vec<u8>>,
}

impl Inv {
    pub fn new(cant_elem_inventario: u32, inv_type_identifier: u32, hashes: Vec<Vec<u8>>) -> Self {
        Inv {
            count: CompactSize::new(cant_elem_inventario.into()),
            inventories: Inv::generar_inventario(hashes, inv_type_identifier),
        }
    }

    /// Genera la estructura de inventarios en base a un vector de vectores u8 (raw headers)
    /// Los inventarios se guardan como un vector de vectores, donde cada elemento del vector
    /// es un inventario correspondiente a un bloque.
    fn generar_inventario(headers: Vec<Vec<u8>>, tipo_hash: u32) -> Vec<Vec<u8>> {
        let mut vector_inventarios = vec![];

        for header in headers {
            let mut inventario: Vec<u8> = tipo_hash.to_le_bytes().to_vec();
            let mut header_hash = sha256d::Hash::hash(&header).to_byte_array().to_vec();
            inventario.append(&mut header_hash);
            vector_inventarios.push(inventario);
        }
        vector_inventarios
    }

    /// Convierte el mensaje en una cadena de bytes
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut vec_inv: Vec<u8> = vec![];
        vec_inv.append(&mut self.count.clone().number);

        for inventory in &self.inventories {
            vec_inv.append(&mut inventory.clone());
        }
        vec_inv
    }

    /// Convierte la cadena de bytes recibida en un struct Inv
    pub fn from_bytes(bytes: &[u8]) -> Result<Inv, RustifyError> {
        let (count, count_bytes) = CompactSize::parse_from_byte_array(bytes);
        let inventories: Vec<Vec<u8>> = bytes[count_bytes..]
            .chunks(bytes[count_bytes..].len() / count.value() as usize)
            .map(|s| s.into())
            .collect();
        Ok(Inv { count, inventories })
    }
}
