use std::future::{Ready, ready};

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::StatusCode,
};
use futures_util::future::LocalBoxFuture;

use crate::modules::config::Config;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct Validation;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for Validation
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = Middleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(Middleware { service }))
    }
}

pub struct Middleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for Middleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let Ok(app_config) = Config::get() else {
            let msg = "Failed to get app config";
            println!("{}", msg);
            return Box::pin(async {
                Ok(early_response(req, StatusCode::INTERNAL_SERVER_ERROR, msg))
            });
        };

        let Some(request_token) = get_token(&req) else {
            return Box::pin(async {
                Ok(early_response(
                    req,
                    StatusCode::UNAUTHORIZED,
                    "No secret token found",
                ))
            });
        };

        if !token_is_valid(&request_token, &app_config.api.token) {
            return Box::pin(async {
                Ok(early_response(
                    req,
                    StatusCode::UNAUTHORIZED,
                    "Invalid secret token",
                ))
            });
        }

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}

fn get_token(req: &ServiceRequest) -> Option<String> {
    req.headers()
        .get("Secret")
        .and_then(|value| value.to_str().ok())
        .map(|content| content.to_string())
}

fn token_is_valid(request_token: &str, token: &str) -> bool {
    request_token == token
}

fn early_response<B: 'static>(
    req: ServiceRequest,
    status_code: StatusCode,
    body: &str,
) -> ServiceResponse<EitherBody<B>> {
    req.into_response(
        HttpResponse::build(status_code)
            .json(body)
            .map_into_right_body(),
    )
}
