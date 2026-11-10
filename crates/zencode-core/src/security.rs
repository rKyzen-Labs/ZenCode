use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Windows Credential Manager error: {0}")]
    WindowsCredentialError(String),
    #[error("Credential not found for target: {0}")]
    NotFound(String),
}

pub const ZENCODE_GEMINI_KEY_TARGET: &str = "zencode:gemini_api_key";
pub const ZENCODE_GROQ_KEY_TARGET: &str = "zencode:groq_api_key";

pub struct CredentialService;

impl CredentialService {
    /// Reads generic credential from Windows Credential Manager.
    #[cfg(windows)]
    pub fn get_gemini_api_key() -> Result<String, SecurityError> {
        Self::get_generic_credential(ZENCODE_GEMINI_KEY_TARGET)
    }

    #[cfg(not(windows))]
    pub fn get_gemini_api_key() -> Result<String, SecurityError> {
        Err(SecurityError::WindowsCredentialError(
            "Windows Credential Manager is only supported on Windows".into(),
        ))
    }

    #[cfg(windows)]
    pub fn set_gemini_api_key(key: &str) -> Result<(), SecurityError> {
        Self::set_generic_credential(ZENCODE_GEMINI_KEY_TARGET, "zencode", key)
    }

    #[cfg(windows)]
    pub fn get_groq_api_key() -> Result<String, SecurityError> {
        Self::get_generic_credential(ZENCODE_GROQ_KEY_TARGET)
    }

    #[cfg(not(windows))]
    pub fn get_groq_api_key() -> Result<String, SecurityError> {
        Err(SecurityError::WindowsCredentialError(
            "Windows Credential Manager is only supported on Windows".into(),
        ))
    }

    #[cfg(windows)]
    pub fn set_groq_api_key(key: &str) -> Result<(), SecurityError> {
        Self::set_generic_credential(ZENCODE_GROQ_KEY_TARGET, "zencode", key)
    }

    #[cfg(not(windows))]
    pub fn set_groq_api_key(_key: &str) -> Result<(), SecurityError> {
        Err(SecurityError::WindowsCredentialError(
            "Windows Credential Manager is only supported on Windows".into(),
        ))
    }

    #[cfg(windows)]
    pub fn delete_groq_api_key() -> Result<(), SecurityError> {
        Self::delete_generic_credential(ZENCODE_GROQ_KEY_TARGET)
    }

    #[cfg(not(windows))]
    pub fn delete_groq_api_key() -> Result<(), SecurityError> {
        Err(SecurityError::WindowsCredentialError(
            "Windows Credential Manager is only supported on Windows".into(),
        ))
    }

    #[cfg(windows)]
    pub fn get_generic_credential(target: &str) -> Result<String, SecurityError> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::GetLastError;
        use windows_sys::Win32::Security::Credentials::{
            CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
        };

        let mut target_u16: Vec<u16> = OsStr::new(target).encode_wide().collect();
        target_u16.push(0);

        let mut credential_ptr: *mut CREDENTIALW = std::ptr::null_mut();

        unsafe {
            let ok = CredReadW(
                target_u16.as_ptr(),
                CRED_TYPE_GENERIC,
                0,
                &mut credential_ptr,
            );

            if ok == 0 {
                let err = GetLastError();
                if err == 1168 {
                    // ERROR_NOT_FOUND
                    return Err(SecurityError::NotFound(target.to_string()));
                }
                return Err(SecurityError::WindowsCredentialError(format!(
                    "CredReadW failed with error code: {}",
                    err
                )));
            }

            let cred = &*credential_ptr;
            if cred.CredentialBlob.is_null() || cred.CredentialBlobSize == 0 {
                CredFree(credential_ptr as *const _);
                return Err(SecurityError::NotFound(target.to_string()));
            }

            // CredentialBlob stored by cmdkey is UTF-16LE
            let blob_slice =
                std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize);

            let string_val = if blob_slice.len() >= 2 && blob_slice[1] == 0 {
                // UTF-16LE
                let u16_slice: &[u16] = std::slice::from_raw_parts(
                    cred.CredentialBlob as *const u16,
                    (cred.CredentialBlobSize / 2) as usize,
                );
                String::from_utf16_lossy(u16_slice)
            } else {
                // UTF-8
                String::from_utf8_lossy(blob_slice).to_string()
            };

            CredFree(credential_ptr as *const _);
            Ok(string_val.trim_matches('\0').trim().to_string())
        }
    }

    #[cfg(windows)]
    pub fn set_generic_credential(
        target: &str,
        username: &str,
        secret: &str,
    ) -> Result<(), SecurityError> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::GetLastError;
        use windows_sys::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };

        let mut target_u16: Vec<u16> = OsStr::new(target).encode_wide().collect();
        target_u16.push(0);

        let mut user_u16: Vec<u16> = OsStr::new(username).encode_wide().collect();
        user_u16.push(0);

        // UTF-16LE representation of the secret
        let secret_u16: Vec<u16> = OsStr::new(secret).encode_wide().collect();
        let blob_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                secret_u16.as_ptr() as *const u8,
                secret_u16.len() * std::mem::size_of::<u16>(),
            )
        };

        let cred = CREDENTIALW {
            Flags: 0,
            Type: CRED_TYPE_GENERIC,
            TargetName: target_u16.as_mut_ptr(),
            Comment: std::ptr::null_mut(),
            LastWritten: windows_sys::Win32::Foundation::FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            },
            CredentialBlobSize: blob_bytes.len() as u32,
            CredentialBlob: blob_bytes.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: user_u16.as_mut_ptr(),
        };

        unsafe {
            let ok = CredWriteW(&cred, 0);
            if ok == 0 {
                let err = GetLastError();
                return Err(SecurityError::WindowsCredentialError(format!(
                    "CredWriteW failed with error code: {}",
                    err
                )));
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    pub fn delete_generic_credential(target: &str) -> Result<(), SecurityError> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::GetLastError;
        use windows_sys::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};

        let mut target_u16: Vec<u16> = OsStr::new(target).encode_wide().collect();
        target_u16.push(0);

        unsafe {
            let ok = CredDeleteW(target_u16.as_ptr(), CRED_TYPE_GENERIC, 0);
            if ok == 0 {
                let err = GetLastError();
                if err == 1168 {
                    // ERROR_NOT_FOUND - already not present
                    return Ok(());
                }
                return Err(SecurityError::WindowsCredentialError(format!(
                    "CredDeleteW failed with error code: {}",
                    err
                )));
            }
        }
        Ok(())
    }
}

/// Utility for redacting sensitive secrets from any logs or diagnostic traces.
pub fn redact_secrets(text: &str, secret: Option<&str>) -> String {
    let mut out = text.to_string();
    if let Some(s) = secret {
        if !s.is_empty() {
            out = out.replace(s, "[REDACTED_API_KEY]");
        }
    }
    // General pattern redaction for AI / bearer keys
    if let Some(pos) = out.find("AIza") {
        if pos + 39 <= out.len() {
            out.replace_range(pos..pos + 39, "[REDACTED_API_KEY]");
        }
    }
    if let Some(pos) = out.find("gsk_") {
        let end = out[pos..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == '}')
            .map(|i| pos + i)
            .unwrap_or(out.len());
        out.replace_range(pos..end, "[REDACTED_API_KEY]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_credential_retrieval_from_windows_manager() {
        let key = CredentialService::get_gemini_api_key();
        assert!(key.is_ok(), "Expected to read stored test credential: {:?}", key.err());
        let val = key.unwrap();
        assert!(!val.is_empty());
        assert!(val.starts_with("AQ."));
    }

    #[test]
    fn test_redaction() {
        let sample = "Calling url https://generativelanguage.googleapis.com?key=AQ.SECRET_123456789";
        let redacted = redact_secrets(sample, Some("AQ.SECRET_123456789"));
        assert!(!redacted.contains("SECRET"));
        assert!(redacted.contains("[REDACTED_API_KEY]"));
    }

    #[tokio::test]
    #[cfg(windows)]
    async fn test_invalid_gemini_key_halts_cleanly_and_restores() {
        use crate::planner::{GeminiPlannerClient, PlanningContext};

        // 1. Read real key
        let real_key = CredentialService::get_gemini_api_key().expect("Failed to read initial key");
        assert!(!real_key.is_empty());

        // 2. Set invalid key
        CredentialService::set_gemini_api_key("AQ.INVALID_KEY_9999999999").expect("Failed to write invalid key");

        // 3. Confirm invalid key is now in Credential Manager
        let current_key = CredentialService::get_gemini_api_key().expect("Failed to read invalid key");
        assert_eq!(current_key, "AQ.INVALID_KEY_9999999999");

        // 4. Run planner with invalid key and confirm clean halt
        let planner = GeminiPlannerClient::new();
        let context = PlanningContext {
            task_description: "Test task".into(),
            project_type: "Rust".into(),
            shallow_tree: vec![],
            manifest_summaries: vec![],
            git_branch_and_status: "main clean".into(),
            approved_source_excerpts: vec![],
            constraints: vec![],
            conversation_history: vec![],
        };
        let plan_result = planner.plan(&current_key, &context).await;
        assert!(plan_result.is_err(), "Expected planning to fail with invalid API key");
        let err_msg = plan_result.err().unwrap().to_string();
        println!("  -> Confirmed clean halt with error: {}", redact_secrets(&err_msg, Some(&current_key)));

        // 5. Restore real key
        CredentialService::set_gemini_api_key(&real_key).expect("Failed to restore real key");

        // 6. Verify real key restored
        let restored_key = CredentialService::get_gemini_api_key().expect("Failed to read restored key");
        assert_eq!(restored_key, real_key);
        println!("  -> Confirmed real key safely restored to Windows Credential Manager.");
    }
}
