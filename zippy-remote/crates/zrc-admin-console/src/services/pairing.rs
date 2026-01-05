use crate::db::store::DbStore;
use crate::db::schema::Pairing;
use anyhow::Result;

#[derive(Clone)]
pub struct PairingService {
    store: DbStore,
}

impl PairingService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn list_pairings(&self) -> Result<Vec<Pairing>> {
        let pairings = sqlx::query_as::<_, Pairing>(
            "SELECT * FROM pairings ORDER BY created_at DESC"
        )
        .fetch_all(self.store.get_pool())
        .await?;
        Ok(pairings)
    }
    
    pub async fn get_pairing(&self, id: &str) -> Result<Option<Pairing>> {
         let pairing = sqlx::query_as::<_, Pairing>(
            "SELECT * FROM pairings WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(self.store.get_pool())
        .await?;
        Ok(pairing)       
    }

    pub async fn revoke_pairing(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE pairings SET status = 'revoked' WHERE id = ?"
        )
        .bind(id)
        .execute(self.store.get_pool())
        .await?;
        Ok(())
    }
}
