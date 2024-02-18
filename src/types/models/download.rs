use sqlx::{types::ipnetwork::IpNetwork, PgConnection};

use crate::types::api::ApiError;

pub async fn create_download(
    ip: IpNetwork,
    mod_version_id: i32,
    pool: &mut PgConnection,
) -> Result<(), ApiError> {
    let existing = match sqlx::query!(
        r#"
        SELECT * FROM mod_downloads
        WHERE ip = $1 AND mod_version_id = $2
        "#,
        ip,
        mod_version_id
    )
    .fetch_optional(&mut *pool)
    .await
    {
        Ok(e) => e,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        }
    };

    if let Some(_) = existing {
        return Ok(());
    }

    match sqlx::query!(
        r#"
        INSERT INTO mod_downloads (ip, mod_version_id)
        VALUES ($1, $2)
        "#,
        ip,
        mod_version_id
    )
    .execute(&mut *pool)
    .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            log::error!("{}", e);
            Err(ApiError::InternalError)
        }
    }
}