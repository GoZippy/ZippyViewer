use crate::db::store::DbStore;
use crate::db::schema::{UpdateChannel, Release};
use anyhow::Result;
use serde::Serialize;

#[derive(Clone)]
pub struct UpdateService {
    store: DbStore,
}

impl UpdateService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn list_channels(&self) -> Result<Vec<UpdateChannel>> {
        let channels = sqlx::query_as::<_, UpdateChannel>(
            "SELECT * FROM update_channels ORDER BY name ASC"
        )
        .fetch_all(self.store.get_pool())
        .await?;
        Ok(channels)
    }

    pub async fn list_releases(&self, channel_id: Option<&str>) -> Result<Vec<Release>> {
        let query = if let Some(cid) = channel_id {
            sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE channel_id = ? ORDER BY published_at DESC").bind(cid)
        } else {
             sqlx::query_as::<_, Release>("SELECT * FROM releases ORDER BY published_at DESC")
        };
        
        let releases = query.fetch_all(self.store.get_pool()).await?;
        Ok(releases)
    }

    pub async fn publish_release(&self, release: Release) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO releases (version, channel_id, url, checksum, changelog, published_at, is_active)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&release.version)
        .bind(&release.channel_id)
        .bind(&release.url)
        .bind(&release.checksum)
        .bind(&release.changelog)
        .bind(release.published_at)
        .bind(release.is_active)
        .execute(self.store.get_pool())
        .await?;
        
        Ok(())
    }

    pub async fn get_rollout_status(&self) -> Result<Vec<ChannelRolloutStatus>> {
        let channels = self.list_channels().await?;
        let mut status_list = Vec::new();

        for channel in channels {
            // Get latest active release for this channel
            let latest_release = sqlx::query_as::<_, Release>(
                "SELECT * FROM releases WHERE channel_id = ? AND is_active = 1 ORDER BY published_at DESC LIMIT 1"
            )
            .bind(&channel.id)
            .fetch_optional(self.store.get_pool())
            .await?;

            // Count total devices in this channel
            let total_devices: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM devices WHERE channel_id = ?"
            )
            .bind(&channel.id)
            .fetch_one(self.store.get_pool())
            .await?;

            let mut devices_on_latest = 0;
            let mut active_version = None;

            if let Some(release) = latest_release {
                active_version = Some(release.version.clone());
                // Count devices on this version
                devices_on_latest = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM devices WHERE channel_id = ? AND version = ?"
                )
                .bind(&channel.id)
                .bind(&release.version)
                .fetch_one(self.store.get_pool())
                .await?;
            }

            // Count offline devices (rudimentary check, e.g., not seen in 24h or status='offline')
            // Using 'status' field for simplicity as per schema
            let devices_offline: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM devices WHERE channel_id = ? AND status = 'offline'"
            )
            .bind(&channel.id)
            .fetch_one(self.store.get_pool())
            .await?;

            status_list.push(ChannelRolloutStatus {
                channel_id: channel.id,
                channel_name: channel.name,
                active_version,
                total_devices,
                devices_on_latest,
                devices_pending: total_devices - devices_on_latest,
                devices_offline,
            });
        }

        Ok(status_list)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelRolloutStatus {
    pub channel_id: String,
    pub channel_name: String,
    pub active_version: Option<String>,
    pub total_devices: i64,
    pub devices_on_latest: i64,
    pub devices_pending: i64,
    pub devices_offline: i64,
}
