#[derive(PartialEq, Debug, Default, Clone)]
pub struct CompactSize {
    pub number: Vec<u8>,
}
impl CompactSize {
    pub fn new(value: u64) -> CompactSize {
        let mut numero: Vec<u8> = vec![];
        if value <= 252 {
            numero.push(value.try_into().unwrap_or_default());
        } else if (253..=65535).contains(&value) {
            numero.push(0xfd);
            let parsing_value: u16 = value.try_into().unwrap_or_default();
            numero.append(&mut parsing_value.to_le_bytes().to_vec());
        } else if (65536..=4294967295).contains(&value) {
            numero.push(0xfe);
            let parsing_value: u32 = value.try_into().unwrap_or_default();
            numero.append(&mut parsing_value.to_le_bytes().to_vec());
        } else {
            numero.push(0xff);
            let parsing_value: u64 = value;
            numero.append(&mut parsing_value.to_le_bytes().to_vec());
        }
        CompactSize { number: numero }
    }

    ///A partir de un vector de bytes, verifica cual es el valor compactsize recibido
    ///Toma 9 bytes para poder abarcar todas las posibilidades de CompactSize
    pub fn parse_from_byte_array(byte_array: &[u8]) -> (Self, usize) {
        let bytes = match byte_array[0] {
            0xfd => 3,
            0xfe => 5,
            0xff => 9,
            _ => 1,
        };
        (
            CompactSize::new(Self::parse_to_u64(bytes, byte_array)),
            bytes,
        )
    }

    ///Devuelve el valor en u64 contenido en el CompactSize
    pub fn value(&self) -> u64 {
        Self::parse_to_u64(self.number.len(), &self.number.clone())
    }

    ///Realiza un parseo para poder devolver el compactsize correctamente
    fn parse_to_u64(bytes: usize, byte_array_ori: &[u8]) -> u64 {
        let mut new_csize = [0u8; 8];
        if bytes == 1 {
            new_csize[0] = byte_array_ori[0];
        } else {
            new_csize[..(bytes - 1)].copy_from_slice(&byte_array_ori[1..((bytes - 1) + 1)]);
        }
        u64::from_le_bytes(new_csize)
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.number.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::compactsize::CompactSize;

    #[test]
    fn compactsize_test() {
        let compactsize = CompactSize::new(5);
        assert_eq!(compactsize.number, [0x05]);
        let compactsize = CompactSize::new(515);
        assert_eq!(compactsize.number, [0xfd, 0x03, 0x02]);
        let compactsize = CompactSize::new(50000);
        assert_eq!(compactsize.number, [0xfd, 0x50, 0xc3]);
    }
    #[test]
    fn parse_compactsize_test() {
        let mut byte_array: [u8; 9] = [0xfd, 0x50, 0xc3, 0x43, 0xdd, 0x12, 0x99, 0xe5, 0xa3];
        assert_eq!(
            CompactSize::parse_from_byte_array(&byte_array.to_vec()).1,
            3
        );
        assert_eq!(
            CompactSize::parse_from_byte_array(&byte_array.to_vec()).0,
            CompactSize::new(50000)
        );

        byte_array = [0x05, 0x50, 0xcf, 0x43, 0xdd, 0x12, 0x99, 0xe5, 0xa3];
        assert_eq!(
            CompactSize::parse_from_byte_array(&byte_array.to_vec()).1,
            1
        );
        assert_eq!(
            CompactSize::parse_from_byte_array(&byte_array.to_vec()).0,
            CompactSize::new(5)
        );
    }
    #[test]
    fn parse_to_u64_test() {
        let mut byte_array: [u8; 9] = [0xfd, 0x50, 0xc3, 0x43, 0xdd, 0x12, 0x99, 0xe5, 0xa3];
        assert_eq!(
            CompactSize::parse_to_u64(3, &byte_array.to_vec()),
            50000 as u64
        );

        byte_array = [0x05, 0x50, 0xcf, 0x43, 0xdd, 0x12, 0x99, 0xe5, 0xa3];
        assert_eq!(CompactSize::parse_to_u64(1, &byte_array.to_vec()), 5 as u64);
    }

    #[test]
    fn csize_as_bytes_test() {
        assert_eq!(CompactSize::new(66).as_bytes(), [66]);
        assert_eq!(CompactSize::new(50000).as_bytes(), [0xfd, 0x50, 0xc3]);
    }
}
