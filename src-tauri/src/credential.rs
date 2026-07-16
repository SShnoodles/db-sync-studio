use keyring::{Entry, Error};

use crate::db::DbConnection;

const SERVICE_NAME: &str = "cc.ssnoodles.db-sync-studio";

pub fn store_password(connection_id: &str, password: &str) -> Result<(), String> {
    credential_entry(connection_id)?
        .set_password(password)
        .map_err(|error| format!("Unable to store the database password securely: {error}"))
}

pub fn hydrate_password(connection: &mut DbConnection) -> Result<(), String> {
    if connection.db_type == "sqlite"
        || connection
            .password
            .as_deref()
            .is_some_and(|password| !password.is_empty())
    {
        return Ok(());
    }
    connection.password = match credential_entry(&connection.id)?.get_password() {
        Ok(password) => Some(password),
        Err(Error::NoEntry) => None,
        Err(error) => {
            return Err(format!(
                "Unable to read the database password from secure storage: {error}"
            ))
        }
    };
    Ok(())
}

pub fn delete_password(connection_id: &str) -> Result<(), String> {
    match credential_entry(connection_id)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Unable to delete the database password from secure storage: {error}"
        )),
    }
}

fn credential_entry(connection_id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, connection_id)
        .map_err(|error| format!("Unable to access secure credential storage: {error}"))
}
