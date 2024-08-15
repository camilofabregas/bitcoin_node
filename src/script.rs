use crate::{account::Account, errors::RustifyError};
use bitcoin_hashes::{hash160, Hash};

#[derive(Debug, Clone)]
pub struct Script {
    bytes: Vec<u8>,
}
impl Script {
    pub fn new(
        mut signature: Vec<u8>,
        mut sec_pubkey: Vec<u8>,
        pubkey_hash: Vec<u8>,
    ) -> Result<Script, RustifyError> {
        Self::check_pubkey_hash(&sec_pubkey, &pubkey_hash)?;

        let mut bytes: Vec<u8> = vec![];
        let byte_verif = signature[signature.len() - 1];
        bytes.push(signature.len() as u8);
        bytes.append(&mut signature);
        bytes.push(sec_pubkey.len() as u8);
        bytes.append(&mut sec_pubkey);

        if byte_verif == 0x01 {
            Ok(Script { bytes })
        } else {
            Err(RustifyError::CheckInvalidoScript)
        }
    }

    /// Verifica si la sec_pubkey con hash160 es igual al pubkey_hash del
    /// p2pkh que se halla en el pk_script
    pub fn check_pubkey_hash(sec_pubkey: &[u8], pubkey_hash: &Vec<u8>) -> Result<(), RustifyError> {
        let hash_to_compare: [u8; 20] = hash160::Hash::hash(sec_pubkey).to_byte_array();
        if &hash_to_compare.to_vec() == pubkey_hash {
            Ok(())
        } else {
            Err(RustifyError::CheckInvalidoScript)
        }
    }

    pub fn as_vec(&mut self) -> Vec<u8> {
        let mut v: Vec<u8> = vec![];
        v.append(&mut self.bytes);
        v
    }

    pub fn obtain_public_adress(raw_script: Vec<u8>) -> Result<String, RustifyError> {
        let mut index: usize = 0;
        if raw_script.len() == index {
            return Err(RustifyError::ErrorConversionBitcoinAddress);
        }
        index += (raw_script[0] + 1) as usize;
        let size_sec_pubkey: usize = raw_script[index] as usize;
        index += 1;
        let mut pk_sigscript = vec![];
        pk_sigscript.append(&mut raw_script[index..index + size_sec_pubkey].to_vec());
        let pubkey_hash: [u8; 20] = hash160::Hash::hash(&pk_sigscript).to_byte_array();
        Ok(Account::encode_bitcoin_adress(pubkey_hash.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{account::Account, script::Script, txn::Txn};

    #[test]
    fn test_check_pubkey_hash() {
        let emisor = Account::new_str(
            "mvkRvqush6X2bJLihJyRJCEA3hygBCCXxs",
            "cRCLe18WvER3JYsfpGvNDncbsZhdecFwQmiVGBcRcC5EJLz7jRaG",
        );

        let sec_pubkey: Vec<u8> = vec![
            2, 89, 57, 143, 224, 171, 34, 253, 196, 207, 98, 210, 238, 4, 103, 193, 109, 184, 50,
            234, 208, 149, 218, 177, 211, 39, 100, 182, 210, 61, 111, 76, 170,
        ];

        assert_eq!(
            Script::check_pubkey_hash(&sec_pubkey, &emisor.decode_bitcoin_adress().unwrap()),
            Ok(())
        );
    }

    #[test]
    fn test_obtain_public_adress() {
        let cuenta = Account::new_str(
            "mkbyF2EZNjAADM7aLCfHAHtxJ9B6cn7FKm",
            "cMtFCjCzR6UYnYxJH6Za9gZt2XCHTDjzfds1g3SMdUoNTHXnS4jd",
        );

        let raw_txn = "0100000001b768014d3909dc4568e4cee1cac20c3c54249a6a8e257c5c34f79e7493523bbe000000006b483045022100853ae1201003ae5c5e45325edd4367806042c5a6c947d15eed46ba9ee94f2bd1022003acc8324b1a87edfb488f17c0b77b40ee0c95a7d5a0032897600485592b84cd0121025cfb3d6d3fc413dd0245ff93e90f0f7d63de3e850c6108d4787edf4624c8af8effffffff0240420f00000000001976a9147a23d7cbca2bb541d28045ca9f7d2a405fa7949e88ac3f420f00000000001976a91437cb7ff61be22be2644473bc3cffad63db74c0aa88acf1109e64";
        let txn_vec = (0..raw_txn.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw_txn[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .unwrap();
        let txn = Txn::from_bytes(txn_vec, 0).unwrap().0;
        assert_eq!(
            Script::obtain_public_adress(txn.tx_in[0].signature_script.clone()).unwrap(),
            cuenta.public_address
        );
    }
}
