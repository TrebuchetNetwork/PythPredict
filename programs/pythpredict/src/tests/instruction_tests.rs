#[cfg(test)]
mod instruction_tests {
    use super::*;
    use crate::instructions::*;
    use anchor_lang::prelude::*;

    #[test]
    fn test_parse_pyth_price_alternative() {
        // Create mock price account data
        let mut data = vec![0u8; 512];

        // Set magic number
        data[0..4].copy_from_slice(&0xa1b2c3d4u32.to_le_bytes());

        // Set price at offset 208
        let price = 9500000i64; // $95000 with exponent -2
        data[208..216].copy_from_slice(&price.to_le_bytes());

        // Set confidence
        let conf = 1000u64;
        data[216..224].copy_from_slice(&conf.to_le_bytes());

        // Set exponent
        let expo = -2i32;
        data[224..228].copy_from_slice(&expo.to_le_bytes());

        // Create mock AccountInfo
        let key = Pubkey::new_unique();
        let mut lamports = 0;
        let owner = Pubkey::new_unique();
        let account = AccountInfo {
            key: &key,
            lamports: Rc::new(RefCell::new(&mut lamports)),
            data: Rc::new(RefCell::new(&mut data[..])),
            owner: &owner,
            is_signer: false,
            is_writable: false,
            executable: false,
            rent_epoch: 0,
        };

        let result = parse_pyth_price_alternative(&account);
        assert!(result.is_ok());

        let (parsed_price, parsed_expo, parsed_conf) = result.unwrap();
        assert_eq!(parsed_price, 9500000);
        assert_eq!(parsed_expo, -2);
        assert_eq!(parsed_conf, 1000);
    }
}