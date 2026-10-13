use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct User {
    pub id: u64,
    pub nickname: String,
    pub color: u8,
    pub character: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    user: User,
    password_hash: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Data {
    next_id: u64,
    records: BTreeMap<u64, Record>,
}
#[derive(Debug)]
pub enum Error {
    Invalid(&'static str),
    Conflict,
    Unauthorized,
    NotFound,
    Storage,
}
impl Error {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Invalid(s) => s,
            Self::Conflict => "Nickname already exists",
            Self::Unauthorized => "Invalid credentials",
            Self::NotFound => "Profile not found",
            Self::Storage => "Storage unavailable",
        }
    }
}

pub struct UserService {
    data: Data,
    path: PathBuf,
}
impl UserService {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let data = match fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b).map_err(|_| Error::Storage)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::default(),
            Err(_) => return Err(Error::Storage),
        };
        Ok(Self { data, path })
    }
    fn commit(&mut self, data: Data) -> Result<(), Error> {
        let save = || -> Result<(), std::io::Error> {
            if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            let temp = self.path.with_extension("tmp");
            fs::write(&temp, serde_json::to_vec_pretty(&data)?)?;
            fs::rename(temp, &self.path)
        };
        save().map_err(|_| {
            tracing::error!("profile storage write failed");
            Error::Storage
        })?;
        tracing::debug!(records = data.records.len(), "profile storage saved");
        self.data = data;
        Ok(())
    }
    pub fn register(
        &mut self,
        nickname: &str,
        password: &str,
        color: u8,
        character: &str,
    ) -> Result<User, Error> {
        validate_profile(nickname, color, character)?;
        if !(8..=128).contains(&password.len()) {
            return Err(Error::Invalid("Password must contain 8 to 128 bytes"));
        }
        if self
            .data
            .records
            .values()
            .any(|r| r.user.nickname.eq_ignore_ascii_case(nickname))
        {
            return Err(Error::Conflict);
        }
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map_err(|_| Error::Storage)?
            .to_string();
        let mut data = self.data.clone();
        data.next_id = data.next_id.checked_add(1).ok_or(Error::Storage)?;
        let user = User {
            id: data.next_id,
            nickname: nickname.to_owned(),
            color,
            character: character.to_owned(),
        };
        data.records.insert(
            user.id,
            Record {
                user: user.clone(),
                password_hash: hash,
            },
        );
        self.commit(data)?;
        tracing::info!(user_id = user.id, "profile created");
        Ok(user)
    }
    pub fn login(&self, nickname: &str, password: &str) -> Result<User, Error> {
        if nickname.chars().count() > 20 || password.len() > 128 {
            return Err(Error::Unauthorized);
        }
        let record = self
            .data
            .records
            .values()
            .find(|r| r.user.nickname.eq_ignore_ascii_case(nickname));
        // Perform password verification for unknown users too, avoiding a cheap username oracle.
        let fallback = "$argon2id$v=19$m=19456,t=2,p=1$YWJjZGVmZ2hpamtsbW5vcA$wIzgtOHjtRfqPxhwSDSuxrycdGY/LhPXZvLaqRKNRjM";
        let hash = PasswordHash::new(record.map_or(fallback, |r| r.password_hash.as_str()))
            .map_err(|_| Error::Storage)?;
        let valid = Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok();
        if valid && let Some(r) = record {
            tracing::info!(user_id = r.user.id, "login accepted");
            return Ok(r.user.clone());
        }
        tracing::warn!("login rejected");
        Err(Error::Unauthorized)
    }
    pub fn read(&self, id: u64) -> Result<User, Error> {
        tracing::trace!(user_id = id, "profile read");
        self.data
            .records
            .get(&id)
            .map(|r| r.user.clone())
            .ok_or(Error::NotFound)
    }
    pub fn update(
        &mut self,
        id: u64,
        nickname: &str,
        color: u8,
        character: &str,
    ) -> Result<User, Error> {
        validate_profile(nickname, color, character)?;
        if self
            .data
            .records
            .values()
            .any(|r| r.user.id != id && r.user.nickname.eq_ignore_ascii_case(nickname))
        {
            return Err(Error::Conflict);
        }
        let mut data = self.data.clone();
        let record = data.records.get_mut(&id).ok_or(Error::NotFound)?;
        record.user.nickname = nickname.into();
        record.user.color = color;
        record.user.character = character.into();
        let result = record.user.clone();
        self.commit(data)?;
        tracing::info!(user_id = id, "profile updated");
        Ok(result)
    }
    pub fn delete(&mut self, id: u64) -> Result<(), Error> {
        let mut data = self.data.clone();
        data.records.remove(&id).ok_or(Error::NotFound)?;
        self.commit(data)?;
        tracing::info!(user_id = id, "profile deleted");
        Ok(())
    }
}
pub fn validate_profile(nickname: &str, color: u8, character: &str) -> Result<(), Error> {
    if nickname.trim().is_empty() {
        return Err(Error::Invalid("Enter a nickname"));
    }
    if nickname.trim() != nickname || nickname.chars().count() > 20 {
        return Err(Error::Invalid(
            "Use at most 20 characters without outer spaces",
        ));
    }
    if !nickname
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-'))
    {
        return Err(Error::Invalid("Use letters, numbers, spaces, _ or -"));
    }
    if color > 7 {
        return Err(Error::Invalid("Color must be between 0 and 7"));
    }
    if !["default", "keqing", "sonic", "cheburashka"].contains(&character) {
        return Err(Error::Invalid("Unknown character"));
    }
    Ok(())
}
