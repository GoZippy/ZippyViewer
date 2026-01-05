use std::collections::HashSet;
use crate::db::schema::UserRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    // Dashboard
    ViewDashboard,
    
    // Users
    CreateUser,
    EditUser,
    DeleteUser,
    ViewUsers,
    
    // Devices
    ViewDevices,
    ManageDevices, // Edit tags, settings
    DeleteDevice,
    
    // Pairings
    ViewPairings,
    RevokePairing,
    
    // Infrastructure
    ViewInfrastructure,
    ManageInfrastructure, // Add/Remove relays, dirnodes
    
    // Audit
    ViewAuditLogs,
    ExportAuditLogs,
    
    // System
    ManageSettings,
}

impl Permission {
    pub fn description(&self) -> &'static str {
        match self {
            Permission::ViewDashboard => "View the main dashboard",
            Permission::CreateUser => "Create new user accounts",
            Permission::EditUser => "Edit existing user accounts",
            Permission::DeleteUser => "Delete user accounts",
            Permission::ViewUsers => "View list of users",
            Permission::ViewDevices => "View list of devices",
            Permission::ManageDevices => "Manage device settings",
            Permission::DeleteDevice => "Remove devices",
            Permission::ViewPairings => "View active pairings",
            Permission::RevokePairing => "Revoke pairings",
            Permission::ViewInfrastructure => "View infrastructure status",
            Permission::ManageInfrastructure => "Manage infrastructure components",
            Permission::ViewAuditLogs => "View audit logs",
            Permission::ExportAuditLogs => "Export audit logs",
            Permission::ManageSettings => "Manage system settings",
        }
    }
}

pub trait RolePermissions {
    fn permissions(&self) -> HashSet<Permission>;
    fn has_permission(&self, permission: Permission) -> bool;
}

impl RolePermissions for UserRole {
    fn permissions(&self) -> HashSet<Permission> {
        let mut perms = HashSet::new();
        
        // Base permissions for everyone (Viewer+)
        perms.insert(Permission::ViewDashboard);
        perms.insert(Permission::ViewDevices);
        perms.insert(Permission::ViewPairings);
        perms.insert(Permission::ViewInfrastructure);
        
        // Operator additions
        if matches!(self, UserRole::Operator | UserRole::Admin | UserRole::SuperAdmin) {
            perms.insert(Permission::RevokePairing);
            perms.insert(Permission::ManageDevices);
        }
        
        // Admin additions
        if matches!(self, UserRole::Admin | UserRole::SuperAdmin) {
            perms.insert(Permission::ViewUsers);
            perms.insert(Permission::ViewAuditLogs);
            perms.insert(Permission::ExportAuditLogs);
            perms.insert(Permission::CreateUser); // Admins can create Operators/Viewers (logic to be enforced in handler)
            perms.insert(Permission::EditUser);
            perms.insert(Permission::DeleteDevice);
        }
        
        // SuperAdmin additions
        if matches!(self, UserRole::SuperAdmin) {
            perms.insert(Permission::DeleteUser);
            perms.insert(Permission::ManageInfrastructure);
            perms.insert(Permission::ManageSettings);
        }
        
        perms
    }

    fn has_permission(&self, permission: Permission) -> bool {
        self.permissions().contains(&permission)
    }
}

use crate::db::schema::User;
use axum::http::StatusCode;
use std::str::FromStr;

pub fn check_permission(user: &User, permission: Permission) -> Result<(), StatusCode> {
    let role = UserRole::from_str(&user.role).map_err(|_| {
        tracing::error!("Invalid role found for user {}: {}", user.id, user.role);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    if role.has_permission(permission) {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}
