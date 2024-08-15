use crate::block_header::BlockHeader;
use crate::serialized_block::SerializedBlock;
use bitcoin_hashes::{sha256d, Hash};
use std::cmp::Ordering;

const LARGO_TARGET: usize = 32;
const BYTES_IN_SIGNIFICAND: u8 = 3;

/// Dado el header de un bloque, se chequea que cumpla la proof of work.
/// Se utiliza su campo n_bits y el hash del header del bloque.
/// Para que cumpla, el hash tiene que ser menor al target.
/// El target se calcula expandiendo el n_bits de 32 bits a un número de 256 bits en un [u8; 32].
pub fn proof_of_work(header_bloque: &BlockHeader) -> bool {
    let n_bits = header_bloque.n_bits;
    let exponente = (n_bits >> 24) as u8;
    let mantisa = n_bits & 0x00ffffff;

    let mut target = [0u8; 32];
    let desplazamiento = (exponente - BYTES_IN_SIGNIFICAND) as usize;
    let inicio_slice = LARGO_TARGET - desplazamiento - BYTES_IN_SIGNIFICAND as usize - 1;
    let fin_slice = LARGO_TARGET - desplazamiento;
    target[inicio_slice..fin_slice].copy_from_slice(&mantisa.to_be_bytes());

    let mut hash = sha256d::Hash::hash(&header_bloque.as_bytes())
        .to_byte_array()
        .to_vec();
    hash.reverse();

    for i in 0..target.len() {
        match hash[i].cmp(&target[i]) {
            Ordering::Greater => return false,
            Ordering::Less => return true,
            Ordering::Equal => continue,
        }
    }
    false
}

/// Verifica la Proof of Inclusion del bloque recibido.
/// Devuelve true si COINCIDE el hash de la raiz del merkle tree GENERADO con el original (guardado en el header del bloque).
/// Devuelve false si no coinciden (el bloque es invalido y no se agrega a la blockchain).
pub fn proof_of_inclusion(bloque: &SerializedBlock) -> bool {
    let merkle_root_hash = bloque.block_header.merkle_root_hash.to_vec();
    let mut txids: Vec<Vec<u8>> = Vec::new();
    // Genero el TXID para cada transaccion (hash de los bytes de cada transaccion).
    for i in 0..bloque.txn_count.value() as usize {
        txids.push(
            sha256d::Hash::hash(&bloque.txns[i].as_bytes())
                .to_byte_array()
                .to_vec(),
        );
    }
    if generar_merkle_tree_root_hash(&mut txids) == merkle_root_hash {
        return true;
    }
    false
}

/// Genera el merkle tree recursivamente hasta obtener el hash de la raiz (merkle root hash).
/// Recibe el vector de TXIDs (hash de cada transaccion).
/// Devuelve el hash de la raiz del merkle tree.
fn generar_merkle_tree_root_hash(transacciones: &mut Vec<Vec<u8>>) -> Vec<u8> {
    // Caso base
    if transacciones.len() == 1 {
        return transacciones[0].to_vec();
    }
    // Si el nro. de transacciones es impar, duplico la ultima.
    if transacciones.len() % 2 != 0 {
        transacciones.push(transacciones.last().unwrap().to_vec());
    }

    let mut transacciones_hasheadas: Vec<Vec<u8>> = Vec::new();
    // Itero de a pares, hasheando la union de ambas transacciones.
    for i in (0..transacciones.len()).step_by(2) {
        let mut txn_1 = transacciones[i].to_vec();
        let mut txn_2 = transacciones[i + 1].to_vec();
        txn_1.append(&mut txn_2);
        let hash_txn = sha256d::Hash::hash(&txn_1).to_byte_array().to_vec();
        transacciones_hasheadas.push(hash_txn);
    }

    generar_merkle_tree_root_hash(&mut transacciones_hasheadas)
}

/// Genera la merkle proof o merkle path a partir de un bloque y una transacción de ese bloque.
/// La merkle proof es un vector de tuplas con el hash y una string "left" o "right"
/// que indica cómo se deben concatenar esos hashes para obtener la merkle root.
pub fn merkle_proof(transaccion: Vec<u8>, bloque: &SerializedBlock) -> Vec<(Vec<u8>, &str)> {
    let mut merkle_proof: Vec<(Vec<u8>, &str)> = vec![];
    let merkle_tree = generar_merkle_tree(bloque);
    let cant_niveles = merkle_tree.len();
    if cant_niveles == 0 {
        return merkle_proof;
    }

    let mut indice_tx;
    let option_indice_tx = merkle_tree[0].iter().position(|tx| tx == &transaccion);
    match option_indice_tx {
        None => return merkle_proof,
        Some(indice) => indice_tx = indice,
    }

    if indice_tx % 2 == 0 {
        merkle_proof.push((transaccion, "left"));
    } else {
        merkle_proof.push((transaccion, "right"));
    }

    let mut es_izquierdo: bool;
    let mut dir_hermano: &str;
    let mut indice_hermano: usize;
    let mut nodo_hermano: (Vec<u8>, &str);
    for (indice_nivel, _nivel) in merkle_tree.iter().enumerate() {
        if indice_nivel == cant_niveles - 1 {
            break;
        }
        es_izquierdo = (indice_tx % 2) == 0;
        if es_izquierdo {
            dir_hermano = "right";
            indice_hermano = indice_tx + 1;
        } else {
            dir_hermano = "left";
            indice_hermano = indice_tx - 1;
        }
        nodo_hermano = (
            merkle_tree[indice_nivel][indice_hermano].clone(),
            dir_hermano,
        );
        merkle_proof.push(nodo_hermano);
        indice_tx /= 2;
    }
    merkle_proof
}

/// Genera el merkle tree a partir de un bloque.
/// El merkle tree es un vector que contiene otros vectores que representan los niveles del árbol,
/// y estos niveles contienen los hashes, que son Vec<u8>.
fn generar_merkle_tree(bloque: &SerializedBlock) -> Vec<Vec<Vec<u8>>> {
    let mut txids: Vec<Vec<u8>> = Vec::new();
    // Genero el TXID para cada transaccion (hash de los bytes de cada transaccion).
    for i in 0..bloque.txn_count.value() as usize {
        txids.push(
            sha256d::Hash::hash(&bloque.txns[i].as_bytes())
                .to_byte_array()
                .to_vec(),
        );
    }
    let mut merkle_tree = vec![txids]; // txids es el primer nivel del árbol.
    let indice = 0;
    merkle_tree = generar_niveles_arbol(merkle_tree, indice);
    merkle_tree
}

/// Genera los niveles del merkle tree recursivamente.
/// Toma un merkle tree parcial y un indice que indica el último nivel generado.
/// Se debe llamar a esta función con la variable merkle_tree que contenga los txids (primer nivel del árbol).
fn generar_niveles_arbol(
    mut merkle_tree: Vec<Vec<Vec<u8>>>,
    mut indice: usize,
) -> Vec<Vec<Vec<u8>>> {
    // Caso base
    if merkle_tree[indice].len() == 1 {
        return merkle_tree;
    }
    // Si el nro. de transacciones es impar, duplico la ultima.
    if merkle_tree[indice].len() % 2 != 0 {
        let tx_duplicada = merkle_tree[indice].last().unwrap().to_vec();
        merkle_tree[indice].push(tx_duplicada);
    }

    let mut transacciones_hasheadas: Vec<Vec<u8>> = Vec::new();
    // Itero de a pares, hasheando la union de ambas transacciones.
    for i in (0..merkle_tree[indice].len()).step_by(2) {
        let mut txn_1 = merkle_tree[indice][i].to_vec();
        let mut txn_2 = merkle_tree[indice][i + 1].to_vec();
        txn_1.append(&mut txn_2);
        let hash_txn = sha256d::Hash::hash(&txn_1).to_byte_array().to_vec();
        transacciones_hasheadas.push(hash_txn);
    }

    indice += 1;
    merkle_tree.push(transacciones_hasheadas);

    generar_niveles_arbol(merkle_tree, indice)
}

/// Genera la merkle root a partir de la merkle proof.
/// Se hace un reduce para hashear todos los valores hasta obtener la root,
/// respetando el orden de concatenación indicado por el "left" o "right" de la merkle proof.
/// Si la merkle proof no era correcta, devuelve un vector vacío.
pub fn generar_merkle_root_con_merkle_proof(merkle_proof: &[(Vec<u8>, &str)]) -> Vec<u8> {
    let merkle_root = merkle_proof.iter().cloned().reduce(|txn_1, txn_2| {
        if txn_2.1 == "right" {
            let concat = [txn_1.0, txn_2.0].concat();
            let hash_txn = sha256d::Hash::hash(&concat).to_byte_array().to_vec();
            (hash_txn, "")
        } else {
            let concat = [txn_2.0, txn_1.0].concat();
            let hash_txn = sha256d::Hash::hash(&concat).to_byte_array().to_vec();
            (hash_txn, "")
        }
    });
    match merkle_root {
        Some(vec) => vec.0,
        None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::RustifyError;

    /// Test para chequear el caso en el que el proof of work debería ser verdadero.
    /// Se utiliza el header del ejemplo del libro Programming Bitcoin p. 172 (n_bits = 0x18013ce9).
    #[test]
    fn test_proof_of_work_caso_true() -> Result<(), RustifyError> {
        let hexa_header = "020000208ec39428b17323fa0ddec8e887b4a7c53b8c0a0a220cfd0000000000000000005b0750fce0a889502d40508d39576821155e9c9e3f5c3157f961db38fd8b25be1e77a759e93c0118a4ffd71d".to_owned();
        let bytes_header = (0..hexa_header.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hexa_header[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()?;
        let header_bloque = BlockHeader::from_bytes(&bytes_header)?;

        assert_eq!(proof_of_work(&header_bloque), true);
        Ok(())
    }

    /// Test para chequear el caso en el que el proof of work debería ser falso
    /// Se utiliza el header del ejemplo del libro Programming Bitcoin p. 172,
    /// pero se cambia el exponente del n_bits para que el proof of work falle (n_bits = 0x17013ce9).
    #[test]
    fn test_proof_of_work_caso_false() -> Result<(), RustifyError> {
        let hexa_header = "020000208ec39428b17323fa0ddec8e887b4a7c53b8c0a0a220cfd0000000000000000005b0750fce0a889502d40508d39576821155e9c9e3f5c3157f961db38fd8b25be1e77a759e93c0117a4ffd71d".to_owned();
        let bytes_header = (0..hexa_header.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hexa_header[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()?;
        let header_bloque = BlockHeader::from_bytes(&bytes_header)?;

        assert_eq!(proof_of_work(&header_bloque), false);
        Ok(())
    }

    /// Prueba que verifica la proof of inclusion del bloque 2.434.337 con 3 transacciones.
    #[test]
    fn test_proof_of_inclusion_datos_reales() {
        // Uso el bloque 2.434.337
        let merkle_root_hash: Vec<u8> = [
            0x08, 0xcb, 0xea, 0xbc, 0x35, 0x30, 0xd4, 0x6f, 0xc2, 0xaa, 0xd5, 0x89, 0x96, 0xf9,
            0x43, 0xce, 0x86, 0x6d, 0xe1, 0xbe, 0x62, 0x7c, 0x9c, 0x78, 0xd9, 0xbf, 0x8a, 0x5b,
            0x20, 0xd8, 0xd6, 0x1e,
        ]
        .to_vec();
        let txn1 = [
            0x54, 0xb2, 0xd6, 0xb6, 0x71, 0xb7, 0xf8, 0x0f, 0xb4, 0xe0, 0x50, 0xc9, 0x93, 0x9f,
            0x6a, 0xde, 0xc3, 0xc7, 0x73, 0x72, 0xf8, 0x59, 0x71, 0x05, 0x24, 0xbb, 0x3a, 0x41,
            0x33, 0x97, 0xc1, 0xc6,
        ]
        .to_vec();
        let txn2 = [
            0x9f, 0xfc, 0xee, 0x1c, 0x31, 0xc3, 0xb2, 0x24, 0x55, 0xfe, 0xa2, 0x10, 0xa2, 0x62,
            0xdf, 0xa4, 0x05, 0x67, 0xd8, 0x56, 0xa8, 0xbd, 0x8f, 0x35, 0x8f, 0xd9, 0x64, 0x5d,
            0x7b, 0x71, 0x5f, 0x43,
        ]
        .to_vec();
        let txn3 = [
            0x75, 0x61, 0x1a, 0x4c, 0x06, 0xcd, 0xc6, 0x7f, 0x68, 0xbc, 0x50, 0x8f, 0x2f, 0x08,
            0x8d, 0x42, 0x59, 0xc4, 0x03, 0x4b, 0xda, 0x07, 0x5d, 0xbc, 0x3a, 0x82, 0x9c, 0x32,
            0x96, 0xd4, 0x49, 0xd0,
        ]
        .to_vec();
        let mut txns = vec![txn1.to_vec(), txn2.to_vec(), txn3.to_vec()];

        assert_eq!(generar_merkle_tree_root_hash(&mut txns), merkle_root_hash);
    }

    /// Prueba que verifica la proof of inclusion simulando un bloque que contiene una sola transacción.
    #[test]
    fn test_proof_of_inclusion_una_transaccion() {
        // Uso el bloque 2.434.432
        let merkle_root_hash: Vec<u8> = [
            0x88, 0xe6, 0x2c, 0x58, 0x0f, 0x2e, 0xca, 0x71, 0xf4, 0xad, 0x4d, 0xfc, 0x0f, 0xe7,
            0x8a, 0x8f, 0x00, 0x69, 0x7b, 0xf1, 0xa3, 0xce, 0xe5, 0x79, 0xfe, 0x7d, 0xfb, 0x2a,
            0xc5, 0x98, 0x9c, 0x43,
        ]
        .to_vec();
        let txn = [
            0x88, 0xe6, 0x2c, 0x58, 0x0f, 0x2e, 0xca, 0x71, 0xf4, 0xad, 0x4d, 0xfc, 0x0f, 0xe7,
            0x8a, 0x8f, 0x00, 0x69, 0x7b, 0xf1, 0xa3, 0xce, 0xe5, 0x79, 0xfe, 0x7d, 0xfb, 0x2a,
            0xc5, 0x98, 0x9c, 0x43,
        ]
        .to_vec();
        let mut txns = vec![txn.to_vec()];

        assert_eq!(generar_merkle_tree_root_hash(&mut txns), merkle_root_hash);
    }

    /// Test que verifica que el merkle tree generado a partir de las transacciones
    /// sea correcto comparando su último elemento con la root.
    #[test]
    fn test_merkle_tree_bien_generado() {
        // Uso el bloque 2.434.337
        let merkle_root_hash: Vec<u8> = [
            0x08, 0xcb, 0xea, 0xbc, 0x35, 0x30, 0xd4, 0x6f, 0xc2, 0xaa, 0xd5, 0x89, 0x96, 0xf9,
            0x43, 0xce, 0x86, 0x6d, 0xe1, 0xbe, 0x62, 0x7c, 0x9c, 0x78, 0xd9, 0xbf, 0x8a, 0x5b,
            0x20, 0xd8, 0xd6, 0x1e,
        ]
        .to_vec();
        let txn1 = [
            0x54, 0xb2, 0xd6, 0xb6, 0x71, 0xb7, 0xf8, 0x0f, 0xb4, 0xe0, 0x50, 0xc9, 0x93, 0x9f,
            0x6a, 0xde, 0xc3, 0xc7, 0x73, 0x72, 0xf8, 0x59, 0x71, 0x05, 0x24, 0xbb, 0x3a, 0x41,
            0x33, 0x97, 0xc1, 0xc6,
        ]
        .to_vec();
        let txn2 = [
            0x9f, 0xfc, 0xee, 0x1c, 0x31, 0xc3, 0xb2, 0x24, 0x55, 0xfe, 0xa2, 0x10, 0xa2, 0x62,
            0xdf, 0xa4, 0x05, 0x67, 0xd8, 0x56, 0xa8, 0xbd, 0x8f, 0x35, 0x8f, 0xd9, 0x64, 0x5d,
            0x7b, 0x71, 0x5f, 0x43,
        ]
        .to_vec();
        let txn3 = [
            0x75, 0x61, 0x1a, 0x4c, 0x06, 0xcd, 0xc6, 0x7f, 0x68, 0xbc, 0x50, 0x8f, 0x2f, 0x08,
            0x8d, 0x42, 0x59, 0xc4, 0x03, 0x4b, 0xda, 0x07, 0x5d, 0xbc, 0x3a, 0x82, 0x9c, 0x32,
            0x96, 0xd4, 0x49, 0xd0,
        ]
        .to_vec();

        let txns = vec![txn1.to_vec(), txn2.to_vec(), txn3.to_vec()];

        let mut merkle_tree = vec![txns];
        let indice = 0;
        merkle_tree = generar_niveles_arbol(merkle_tree, indice);

        assert_eq!(merkle_tree[2][0], merkle_root_hash);
    }

    /// Test que verifica que la merkle proof sea correcta.
    /// Se genera la merkle proof a partir de un bloque y una transacción de ese bloque.
    /// Luego se genera la merkle root a partir de la merkle proof y se comparar con
    /// la merkle root del bloque, para verificar que la proof se generó correctamente.
    #[test]
    fn test_merkle_proof_bien_generada() {
        // Uso el bloque 2.434.337
        let txn2: Vec<u8> = [
            0x9f, 0xfc, 0xee, 0x1c, 0x31, 0xc3, 0xb2, 0x24, 0x55, 0xfe, 0xa2, 0x10, 0xa2, 0x62,
            0xdf, 0xa4, 0x05, 0x67, 0xd8, 0x56, 0xa8, 0xbd, 0x8f, 0x35, 0x8f, 0xd9, 0x64, 0x5d,
            0x7b, 0x71, 0x5f, 0x43,
        ]
        .to_vec();

        let block_bytes: Vec<u8> = vec![
            0x00, 0x00, 0x40, 0x20, 0xc2, 0xd9, 0x74, 0xfe, 0xca, 0x4b, 0x12, 0x20, 0x50, 0x13,
            0x35, 0xbf, 0x5f, 0x27, 0x2c, 0xd0, 0x38, 0xee, 0xa6, 0x57, 0x82, 0x48, 0xbe, 0xca,
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0xcb, 0xea, 0xbc, 0x35, 0x30,
            0xd4, 0x6f, 0xc2, 0xaa, 0xd5, 0x89, 0x96, 0xf9, 0x43, 0xce, 0x86, 0x6d, 0xe1, 0xbe,
            0x62, 0x7c, 0x9c, 0x78, 0xd9, 0xbf, 0x8a, 0x5b, 0x20, 0xd8, 0xd6, 0x1e, 0x3a, 0xb1,
            0x68, 0x64, 0x8c, 0xca, 0x27, 0x19, 0x47, 0x65, 0xae, 0x10, 0x03, 0x01, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x1b, 0x03, 0x21, 0x25,
            0x25, 0x04, 0x3a, 0xb1, 0x68, 0x64, 0x00, 0x30, 0x00, 0x00, 0x0d, 0x0f, 0x11, 0x00,
            0x00, 0x08, 0x4d, 0x61, 0x72, 0x61, 0x63, 0x6f, 0x72, 0x65, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x26, 0x6a, 0x24, 0xaa, 0x21,
            0xa9, 0xed, 0xb4, 0xf4, 0xcd, 0x0d, 0xd1, 0x54, 0x91, 0xd9, 0xfa, 0x8a, 0x29, 0xb5,
            0x8e, 0x77, 0x5e, 0x72, 0xf7, 0xdf, 0xd9, 0x32, 0x7d, 0x1d, 0x34, 0x51, 0xab, 0x37,
            0x72, 0x1c, 0x2a, 0x3d, 0xcf, 0x45, 0x45, 0x42, 0x25, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x19, 0x76, 0xa9, 0x14, 0xe3, 0x59, 0xf6, 0x95, 0xc8, 0x0f, 0xc9, 0xf7, 0x19, 0x24,
            0x46, 0xcd, 0xc9, 0x4a, 0xaf, 0xa0, 0x07, 0xfa, 0xe2, 0xe6, 0x88, 0xac, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x26, 0x63, 0x1a, 0x4d, 0x2c, 0x80, 0x03,
            0x4c, 0x3c, 0xdb, 0x63, 0xdc, 0xae, 0x2b, 0xb0, 0xfc, 0x23, 0xe2, 0x1c, 0x25, 0x2b,
            0x19, 0x13, 0xb8, 0x51, 0x66, 0x5b, 0x78, 0x63, 0x2b, 0x89, 0x25, 0x01, 0x00, 0x00,
            0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x01, 0xee, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x22, 0x51, 0x20, 0x22, 0x05, 0x33, 0xfd, 0x42, 0xbe, 0xf5, 0x69, 0x80, 0x83,
            0x0d, 0xe1, 0x5a, 0xd9, 0x1c, 0xcb, 0xb0, 0x26, 0x0e, 0x16, 0x44, 0x51, 0x4d, 0xe4,
            0xa5, 0x8e, 0x91, 0xde, 0xd8, 0x55, 0xee, 0x66, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x01, 0xab, 0x16, 0x2e, 0x81, 0x1c, 0x15, 0x0b, 0x07, 0x37, 0x2f, 0x63,
            0x30, 0x95, 0xdb, 0x5a, 0x99, 0x01, 0xe1, 0xc2, 0x12, 0xed, 0x6c, 0xd8, 0x87, 0x14,
            0x85, 0xdc, 0xce, 0x42, 0x65, 0xd1, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0xfe, 0xff,
            0xff, 0xff, 0x02, 0x70, 0x17, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x16, 0x00, 0x14,
            0xf3, 0x51, 0xb1, 0xbf, 0x64, 0x4d, 0xf4, 0x6b, 0x2c, 0x9c, 0xe8, 0xa0, 0xa2, 0x6a,
            0xd6, 0x8d, 0x0f, 0xd2, 0xa3, 0x39, 0x92, 0x75, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x16, 0x00, 0x14, 0x11, 0xea, 0x33, 0x07, 0xe1, 0x0b, 0xd9, 0x86, 0xda, 0x24, 0x75,
            0x76, 0x0c, 0x30, 0xf6, 0xab, 0x45, 0x85, 0xe7, 0x41, 0x1f, 0x25, 0x25, 0x00,
        ];

        let block = SerializedBlock::from_bytes(&block_bytes).unwrap();

        let merkle_proof = merkle_proof(txn2, &block);
        let merkle_root = generar_merkle_root_con_merkle_proof(&merkle_proof);

        assert_eq!(merkle_root, block.block_header.merkle_root_hash);
    }
}
