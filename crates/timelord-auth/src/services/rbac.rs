#![allow(dead_code)]
use crate::models::org_member::OrgRole;
use timelord_common::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    CalendarRead,
    CalendarWrite,
    EventRead,
    EventWrite,
    OrgManage,
    MemberManage,
}

/// Check if the given role has the required permission.
pub fn check(role: &OrgRole, permission: Permission) -> Result<(), AppError> {
    let allowed = match permission {
        Permission::CalendarRead | Permission::EventRead => true, // all roles
        Permission::CalendarWrite | Permission::EventWrite => {
            matches!(role, OrgRole::Owner | OrgRole::Admin | OrgRole::Member)
        }
        Permission::OrgManage | Permission::MemberManage => {
            matches!(role, OrgRole::Owner | OrgRole::Admin)
        }
    };

    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}
