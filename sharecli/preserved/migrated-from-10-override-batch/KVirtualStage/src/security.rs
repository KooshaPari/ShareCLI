// SPDX-License-Identifier: Apache-2.0 OR MIT
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, Result};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub credential_type: String, // "oauth", "password", "token", "certificate"
    pub service: String,
    pub username: Option<String>,
    pub encrypted_data: String,
    pub metadata: HashMap<String, String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialVault {
    pub id: String,
    pub name: String,
    pub encrypted: bool,
    pub credentials: Vec<Credential>,
    pub created_at: String,
    pub modified_at: String,
}

pub struct SecurityManager {
    vault_path: PathBuf,
    master_key: Option<Vec<u8>>,
    credentials: HashMap<String, Credential>,
    encryption_enabled: bool,
}

impl SecurityManager {
    pub async fn new() -> Result<Self> {
        info!("Initializing Security Manager");

        let vault_path = Self::get_vault_path()?;

        let mut manager = Self {
            vault_path,
            master_key: None,
            credentials: HashMap::new(),
            encryption_enabled: true,
        };

        // Load existing credentials
        manager.load_credentials().await?;

        Ok(manager)
    }

    pub async fn unlock_vault(&mut self, password: String) -> Result<()> {
        info!("Unlocking credential vault");

        // Derive key from password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Password hashing failed: {}", e))?
            .to_string();

        // Use the hash as the master key
        self.master_key = Some(password_hash.as_bytes().to_vec());

        Ok(())
    }

    pub async fn store_credential(&mut self, credential: Credential) -> Result<()> {
        info!(
            "Storing credential: {} ({})",
            credential.name, credential.service
        );

        if self.encryption_enabled && self.master_key.is_none() {
            return Err(anyhow!("Vault is locked. Please unlock first."));
        }

        let encrypted_credential = if self.encryption_enabled {
            self.encrypt_credential(credential)?
        } else {
            credential
        };

        self.credentials
            .insert(encrypted_credential.id.clone(), encrypted_credential);

        // Save to disk
        self.save_credentials().await?;

        Ok(())
    }

    pub async fn get_credential(&self, credential_id: String) -> Result<Credential> {
        info!("Retrieving credential: {}", credential_id);

        if let Some(credential) = self.credentials.get(&credential_id) {
            if self.encryption_enabled && self.master_key.is_none() {
                return Err(anyhow!("Vault is locked. Please unlock first."));
            }

            if self.encryption_enabled {
                self.decrypt_credential(credential.clone())
            } else {
                Ok(credential.clone())
            }
        } else {
            Err(anyhow!("Credential not found: {}", credential_id))
        }
    }

    pub async fn list_credentials(&self) -> Result<Vec<Credential>> {
        info!("Listing credentials");

        let mut credentials = Vec::new();

        for credential in self.credentials.values() {
            if self.encryption_enabled && self.master_key.is_none() {
                // Return metadata only
                let metadata_credential = Credential {
                    id: credential.id.clone(),
                    name: credential.name.clone(),
                    credential_type: credential.credential_type.clone(),
                    service: credential.service.clone(),
                    username: credential.username.clone(),
                    encrypted_data: "[ENCRYPTED]".to_string(),
                    metadata: credential.metadata.clone(),
                    created_at: credential.created_at.clone(),
                    expires_at: credential.expires_at.clone(),
                };
                credentials.push(metadata_credential);
            } else if self.encryption_enabled {
                let decrypted = self.decrypt_credential(credential.clone())?;
                credentials.push(decrypted);
            } else {
                credentials.push(credential.clone());
            }
        }

        Ok(credentials)
    }

    pub async fn remove_credential(&mut self, credential_id: String) -> Result<()> {
        info!("Removing credential: {}", credential_id);

        if self.credentials.remove(&credential_id).is_some() {
            self.save_credentials().await?;
            Ok(())
        } else {
            Err(anyhow!("Credential not found: {}", credential_id))
        }
    }

    pub async fn store_oauth_token(
        &mut self,
        service: String,
        token: OAuthToken,
    ) -> Result<String> {
        info!("Storing OAuth token for service: {}", service);

        let credential_id = uuid::Uuid::new_v4().to_string();
        let token_data = serde_json::to_string(&token)?;

        let credential = Credential {
            id: credential_id.clone(),
            name: format!("{} OAuth Token", service),
            credential_type: "oauth".to_string(),
            service: service.clone(),
            username: None,
            encrypted_data: token_data,
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            expires_at: token.expires_in.map(|expires| {
                chrono::Utc::now()
                    .checked_add_signed(chrono::Duration::seconds(expires as i64))
                    .unwrap()
                    .to_rfc3339()
            }),
        };

        self.store_credential(credential).await?;

        Ok(credential_id)
    }

    pub async fn get_oauth_token(&self, service: String) -> Result<OAuthToken> {
        info!("Retrieving OAuth token for service: {}", service);

        for credential in self.credentials.values() {
            if credential.service == service && credential.credential_type == "oauth" {
                let decrypted = if self.encryption_enabled {
                    self.decrypt_credential(credential.clone())?
                } else {
                    credential.clone()
                };

                let token: OAuthToken = serde_json::from_str(&decrypted.encrypted_data)?;
                return Ok(token);
            }
        }

        Err(anyhow!("OAuth token not found for service: {}", service))
    }

    pub async fn store_password(
        &mut self,
        service: String,
        username: String,
        password: String,
    ) -> Result<String> {
        info!("Storing password for {}: {}", service, username);

        let credential_id = uuid::Uuid::new_v4().to_string();

        let credential = Credential {
            id: credential_id.clone(),
            name: format!("{} Password", service),
            credential_type: "password".to_string(),
            service: service.clone(),
            username: Some(username),
            encrypted_data: password,
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
        };

        self.store_credential(credential).await?;

        Ok(credential_id)
    }

    pub async fn get_password(&self, service: String, username: String) -> Result<String> {
        info!("Retrieving password for {}: {}", service, username);

        for credential in self.credentials.values() {
            if credential.service == service
                && credential.credential_type == "password"
                && credential.username.as_ref() == Some(&username)
            {
                let decrypted = if self.encryption_enabled {
                    self.decrypt_credential(credential.clone())?
                } else {
                    credential.clone()
                };

                return Ok(decrypted.encrypted_data);
            }
        }

        Err(anyhow!("Password not found for {}: {}", service, username))
    }

    pub async fn inject_credentials(&self, session_id: String, service: String) -> Result<()> {
        info!(
            "Injecting credentials for session {} service {}",
            session_id, service
        );

        // This is a placeholder for credential injection
        // In a real implementation, this would interact with the browser
        // or application to fill in credentials

        Ok(())
    }

    fn encrypt_credential(&self, credential: Credential) -> Result<Credential> {
        if let Some(master_key) = &self.master_key {
            let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&master_key[..32]);
            let cipher = Aes256Gcm::new(key);

            let nonce = Nonce::from_slice(b"unique nonce"); // In production, use random nonce

            let encrypted_data = cipher
                .encrypt(nonce, credential.encrypted_data.as_bytes())
                .map_err(|e| anyhow!("Encryption failed: {}", e))?;

            let encoded_data = general_purpose::STANDARD.encode(encrypted_data);

            Ok(Credential {
                encrypted_data: encoded_data,
                ..credential
            })
        } else {
            Err(anyhow!("Master key not available for encryption"))
        }
    }

    fn decrypt_credential(&self, credential: Credential) -> Result<Credential> {
        if let Some(master_key) = &self.master_key {
            let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&master_key[..32]);
            let cipher = Aes256Gcm::new(key);

            let nonce = Nonce::from_slice(b"unique nonce"); // In production, use stored nonce

            let encrypted_data = general_purpose::STANDARD
                .decode(&credential.encrypted_data)
                .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;

            let decrypted_data = cipher
                .decrypt(nonce, encrypted_data.as_slice())
                .map_err(|e| anyhow!("Decryption failed: {}", e))?;

            let decrypted_string = String::from_utf8(decrypted_data)
                .map_err(|e| anyhow!("UTF-8 conversion failed: {}", e))?;

            Ok(Credential {
                encrypted_data: decrypted_string,
                ..credential
            })
        } else {
            Err(anyhow!("Master key not available for decryption"))
        }
    }

    async fn load_credentials(&mut self) -> Result<()> {
        if !self.vault_path.exists() {
            info!("Credential vault does not exist, creating new one");
            return Ok(());
        }

        let vault_data = tokio::fs::read_to_string(&self.vault_path).await?;
        let vault: CredentialVault = serde_json::from_str(&vault_data)?;

        for credential in vault.credentials {
            self.credentials.insert(credential.id.clone(), credential);
        }

        info!("Loaded {} credentials", self.credentials.len());
        Ok(())
    }

    async fn save_credentials(&self) -> Result<()> {
        let vault = CredentialVault {
            id: uuid::Uuid::new_v4().to_string(),
            name: "KVirtualStage Vault".to_string(),
            encrypted: self.encryption_enabled,
            credentials: self.credentials.values().cloned().collect(),
            created_at: chrono::Utc::now().to_rfc3339(),
            modified_at: chrono::Utc::now().to_rfc3339(),
        };

        let vault_data = serde_json::to_string_pretty(&vault)?;

        // Ensure directory exists
        if let Some(parent) = self.vault_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.vault_path, vault_data).await?;

        info!("Saved {} credentials to vault", self.credentials.len());
        Ok(())
    }

    fn get_vault_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home.join(".kvirtualstage").join("vault.json"))
    }

    pub async fn export_vault(&self, export_path: String) -> Result<()> {
        info!("Exporting vault to: {}", export_path);

        let vault_data = tokio::fs::read_to_string(&self.vault_path).await?;
        tokio::fs::write(export_path, vault_data).await?;

        Ok(())
    }

    pub async fn import_vault(&mut self, import_path: String) -> Result<()> {
        info!("Importing vault from: {}", import_path);

        let vault_data = tokio::fs::read_to_string(import_path).await?;
        let vault: CredentialVault = serde_json::from_str(&vault_data)?;

        for credential in vault.credentials {
            self.credentials.insert(credential.id.clone(), credential);
        }

        self.save_credentials().await?;

        Ok(())
    }
}
