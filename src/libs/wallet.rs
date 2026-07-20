use crate::error::AppError;

pub fn normalize_wallet_address(wallet_address: &str) -> Result<String, AppError> {
    let decoded = decode_hex(wallet_address)
        .map_err(|_| AppError::BadRequest("invalid wallet address".to_owned()))?;

    if decoded.len() != 20 {
        return Err(AppError::BadRequest("invalid wallet address".to_owned()));
    }

    Ok(format!("0x{}", encode_hex(&decoded)))
}

pub fn same_wallet(left: &str, right: &str) -> bool {
    match (
        normalize_wallet_address(left),
        normalize_wallet_address(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn decode_hex(input: &str) -> Result<Vec<u8>, ()> {
    let normalized = input.strip_prefix("0x").unwrap_or(input);

    if normalized.len() % 2 != 0 {
        return Err(());
    }

    let mut bytes = Vec::with_capacity(normalized.len() / 2);

    for pair in normalized.as_bytes().chunks_exact(2) {
        let high = decode_hex_nibble(pair[0])?;
        let low = decode_hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }

    Ok(bytes)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}
