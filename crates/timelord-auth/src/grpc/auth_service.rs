#![allow(dead_code)]
use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::repo::{org_repo, user_repo};
use crate::services::AppState;
use timelord_proto::timelord::auth::{
    auth_service_server::AuthService, GetUserOrgsRequest, GetUserOrgsResponse, GetUserRequest,
    GetUserResponse, OrgMembership, ValidateTokenRequest, ValidateTokenResponse,
};

pub struct AuthServiceImpl {
    state: Arc<AppState>,
}

impl AuthServiceImpl {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>,
    ) -> Result<Response<ValidateTokenResponse>, Status> {
        let token = &request.into_inner().token;
        match self.state.jwt.decode_access(token) {
            Ok(data) => {
                let claims = data.claims;
                Ok(Response::new(ValidateTokenResponse {
                    valid: true,
                    user_id: claims.sub.to_string(),
                    org_id: claims.org.to_string(),
                    role: claims.role,
                    jti: claims.jti.to_string(),
                }))
            }
            Err(_) => Ok(Response::new(ValidateTokenResponse {
                valid: false,
                ..Default::default()
            })),
        }
    }

    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let user_id: uuid::Uuid = request
            .into_inner()
            .user_id
            .parse()
            .map_err(|_| Status::invalid_argument("Invalid user_id"))?;

        let user = user_repo::find_by_id(&self.state.pool, user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(GetUserResponse {
            id: user.id.to_string(),
            email: user.email,
            display_name: user.display_name,
            avatar_url: user.avatar_url.unwrap_or_default(),
            provider: user.provider,
        }))
    }

    async fn get_user_orgs(
        &self,
        request: Request<GetUserOrgsRequest>,
    ) -> Result<Response<GetUserOrgsResponse>, Status> {
        let user_id: uuid::Uuid = request
            .into_inner()
            .user_id
            .parse()
            .map_err(|_| Status::invalid_argument("Invalid user_id"))?;

        let orgs = org_repo::list_user_orgs(&self.state.pool, user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let memberships = orgs
            .into_iter()
            .map(|(org, role)| OrgMembership {
                org_id: org.id.to_string(),
                org_name: org.name,
                slug: org.slug,
                role: role.to_string(),
            })
            .collect();

        Ok(Response::new(GetUserOrgsResponse { memberships }))
    }
}
