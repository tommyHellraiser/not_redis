use std::future::{Ready, ready};

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures_util::future::LocalBoxFuture;

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
        
        let Some(token) = get_token(&req) else {
            let response = req.into_response(
                HttpResponse::Unauthorized()
                    .body("No secret token found")
                    .map_into_right_body(),
            );
            return Box::pin(async { Ok(response) });
        };

        if !secret_is_valid(&token) {
            let response = req.into_response(
                HttpResponse::Unauthorized()
                    .body("Invalid secret token")
                    .map_into_right_body(),
            );
            return Box::pin(async { Ok(response) });
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

fn secret_is_valid(token: &str) -> bool {
    if token == "1234" {
        return true;
    }

    false
}
