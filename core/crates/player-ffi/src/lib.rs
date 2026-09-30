//! Stable, versioned C ABI for the native Windows application.
#[unsafe(no_mangle)]
pub extern "C" fn spiewak_abi_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn abi_is_versioned() {
        assert_eq!(super::spiewak_abi_version(), 1);
    }
}
