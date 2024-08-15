use chrono::Utc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct LockTime {
    pub value: u32,
    _is_block_height: bool,
}

impl LockTime {
    /// Devuelve un numero en formato u32 de la hora actual en unixtime
    pub fn create() -> u32 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(time) => time.as_secs() as u32,
            Err(_) => 0xffffffff,
        }
    }

    pub fn from_bytes(raw_locktime_bytes: Vec<u8>) -> LockTime {
        let mut array_locktime: [u8; 4] = [0u8; 4];
        array_locktime.copy_from_slice(&raw_locktime_bytes);
        let value = u32::from_le_bytes(array_locktime);
        LockTime {
            value,
            _is_block_height: LockTime::is_block_height(value),
        }
    }

    /// Determina si un numero potencialmente LockTime
    /// seria de tipo BlockHeight o de tipo Timestamp
    pub fn is_block_height(value: u32) -> bool {
        value < (500000000_u32)
    }

    /// Obtiene el unix time en formato numero
    pub fn current_unixtime() -> u32 {
        let now = Utc::now();
        now.timestamp() as u32
    }
}

#[cfg(test)]
mod tests {
    use crate::locktime::LockTime;

    #[test]
    fn locktime_new_test() {
        let ahora = LockTime::create();
        assert_ne!(ahora, 0xffffffff);
        assert_eq!(
            LockTime::from_bytes(ahora.to_le_bytes().to_vec()).value,
            ahora
        );
        println!("Comparala con la hora actual: {}", ahora);
    }

    #[test]
    fn locktime_unixtime_test() {
        let raw_data: Vec<u8> = [96, 251, 67, 100].to_vec();
        assert_eq!(LockTime::from_bytes(raw_data)._is_block_height, false);
    }

    #[test]
    fn locktime_blockheight_test() {
        let raw_data: Vec<u8> = (2430349 as u32).to_le_bytes().to_vec();
        assert_eq!(LockTime::from_bytes(raw_data)._is_block_height, true);
        let raw_data: Vec<u8> = (1687485110 as u32).to_le_bytes().to_vec();
        assert_eq!(LockTime::from_bytes(raw_data)._is_block_height, false);
    }

    #[test]
    fn current_unixtime_test() {
        assert!(LockTime::current_unixtime() > 1686434999);
        assert!(LockTime::current_unixtime() < 1718068118);
    }
}
