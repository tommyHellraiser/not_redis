use std::future::{Ready, ready};

use actix_web::{
    Error,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::StatusCode,
};
use futures_util::future::LocalBoxFuture;
use the_logger::{TheLogger, log_info};

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct RequestLogger;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for RequestLogger
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
        //  Gather info for request logging
        let method = req.method().to_string();
        let path = req.path().to_string();
        let ip = req
            .connection_info()
            .peer_addr()
            .map(|info| info.to_string());

        //  Record start time before the request is executed in the endpoint
        let start_time = std::time::Instant::now();

        let fut = self.service.call(req);

        Box::pin(async move {
            build_and_log_request_info(method, path, ip).await;

            let res = fut.await?;
            let response_code = res.status();

            let process_duration = (std::time::Instant::now() - start_time).as_micros();

            build_and_log_response_info(response_code, process_duration).await;

            Ok(res.map_into_left_body())
        })
    }
}

async fn build_and_log_request_info(method: String, path: String, ip: Option<String>) {
    let logger = TheLogger::instance();
    let mut msg = format!("{} \t{}", method, path);
    if let Some(ip) = ip {
        let ip_str = format!(" -> from: {}", ip);
        msg.push_str(&ip_str);
    }
    log_info!(logger, "{}", msg);
}

async fn build_and_log_response_info(response_code: StatusCode, process_duration: u128) {
    let logger = TheLogger::instance();
    log_info!(
        logger,
        "Response code: {}, Elapsed: {}us",
        response_code,
        process_duration
    );
}
