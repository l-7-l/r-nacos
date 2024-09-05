use crate::common::appdata::AppShareData;
use crate::common::model::ApiResult;
use crate::common::string_utils::StringUtils;
use crate::config::core::ConfigActor;
use crate::console::model::NamespaceInfo;
use crate::console::NamespaceUtils;
use actix::Addr;
use actix_web::{web, HttpResponse, Responder};
use std::sync::Arc;
use uuid::Uuid;

pub async fn query_namespace_list(config_addr: web::Data<Addr<ConfigActor>>) -> impl Responder {
    let namespaces = NamespaceUtils::get_namespaces(&config_addr).await;
    HttpResponse::Ok().json(ApiResult::success(Some(namespaces)))
}

pub async fn add_namespace(
    param: web::Json<NamespaceInfo>,
    app_data: web::Data<Arc<AppShareData>>,
) -> impl Responder {
    let mut param = param.0;
    if StringUtils::is_option_empty(&param.namespace_id) {
        param.namespace_id = Some(Uuid::new_v4().to_string());
    }
    match NamespaceUtils::add_namespace(&app_data, param).await {
        Ok(_) => HttpResponse::Ok().json(ApiResult::success(Some(true))),
        Err(e) => HttpResponse::Ok().json(ApiResult::<()>::error(
            "SYSTEM_ERROR".to_string(),
            Some(e.to_string()),
        )),
    }
}

pub async fn update_namespace(
    param: web::Json<NamespaceInfo>,
    app_data: web::Data<Arc<AppShareData>>,
) -> impl Responder {
    match NamespaceUtils::update_namespace(&app_data, param.0).await {
        Ok(_) => HttpResponse::Ok().json(ApiResult::success(Some(true))),
        Err(e) => HttpResponse::Ok().json(ApiResult::<()>::error(
            "SYSTEM_ERROR".to_string(),
            Some(e.to_string()),
        )),
    }
}

pub async fn remove_namespace(
    param: web::Json<NamespaceInfo>,
    app_data: web::Data<Arc<AppShareData>>,
) -> impl Responder {
    match NamespaceUtils::remove_namespace(&app_data, param.0.namespace_id).await {
        Ok(_) => HttpResponse::Ok().json(ApiResult::success(Some(true))),
        Err(e) => HttpResponse::Ok().json(ApiResult::<()>::error(
            "SYSTEM_ERROR".to_string(),
            Some(e.to_string()),
        )),
    }
}
