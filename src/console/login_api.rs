use std::sync::Arc;

use actix_web::{
    cookie::Cookie,
    web::{self, Data},
    HttpRequest, HttpResponse, Responder,
};
use captcha::filters::{Grid, Noise};
use captcha::Captcha;

use crate::{
    common::{
        appdata::AppShareData,
        crypto_utils,
        model::{ApiResult, UserSession},
    },
    raft::cache::{
        model::{CacheKey, CacheType, CacheValue},
        CacheLimiterReq, CacheManagerReq, CacheManagerResult,
    },
    user::{UserManagerReq, UserManagerResult},
};

use super::model::login_model::LoginParam;

pub async fn login(
    request: HttpRequest,
    app: Data<Arc<AppShareData>>,
    web::Form(param): web::Form<LoginParam>,
) -> actix_web::Result<impl Responder> {
    //校验验证码
    let captcha_token = if let Some(ck) = request.cookie("captcha_token") {
        ck.value().to_owned()
    } else {
        return Ok(HttpResponse::Ok().json(ApiResult::<()>::error(
            "CAPTCHA_CHECK_ERROR".to_owned(),
            Some("captcha token is empty".to_owned()),
        )));
    };
    let captcha_code = param.captcha.to_uppercase();
    let cache_req = CacheManagerReq::Get(CacheKey::new(
        CacheType::String,
        Arc::new(format!("Captcha_{}", &captcha_token)),
    ));
    let captcha_check_result = if let Ok(Ok(CacheManagerResult::Value(CacheValue::String(v)))) =
        app.cache_manager.send(cache_req).await
    {
        &captcha_code == v.as_ref()
    } else {
        false
    };
    if !captcha_check_result {
        return Ok(HttpResponse::Ok()
            .cookie(
                Cookie::build("captcha_token", "")
                    .path("/")
                    .http_only(true)
                    .finish(),
            )
            .json(ApiResult::<()>::error(
                "CAPTCHA_CHECK_ERROR".to_owned(),
                Some("CAPTCHA_CHECK_ERROR".to_owned()),
            )));
    }
    let limit_key = Arc::new(format!("USER_L#{}", &param.username));
    let limit_req = CacheLimiterReq::Hour {
        key: limit_key.clone(),
        limit: app.sys_config.console_login_one_hour_limit as i32,
    };
    //登录前先判断是否登陆准入
    if let Ok(CacheManagerResult::Limiter(acquire_result)) =
        app.raft_cache_route.request_limiter(limit_req).await
    {
        if !acquire_result {
            return Ok(HttpResponse::Ok().json(ApiResult::<()>::error(
                "LOGIN_LIMITE_ERROR".to_owned(),
                Some("Frequent login, please try again later".to_owned()),
            )));
        }
    } else {
        return Ok(HttpResponse::Ok().json(ApiResult::<()>::error("SYSTEM_ERROR".to_owned(), None)));
    }
    let password = match decode_password(&param.password, &captcha_token) {
        Ok(v) => v,
        Err(e) => {
            log::error!("decode_password error:{}", e);
            return Ok(HttpResponse::Ok().json(ApiResult::<()>::error(
                "SYSTEM_ERROR".to_owned(),
                Some("decode_password error".to_owned()),
            )));
        }
    };
    let msg = UserManagerReq::CheckUser {
        name: param.username,
        password,
    };
    if let Ok(Ok(UserManagerResult::CheckUserResult(valid, user))) =
        app.user_manager.send(msg).await
    {
        if valid {
            //增加长度避免遍历
            let token = Arc::new(
                uuid::Uuid::new_v4().to_string().replace('-', "")
                    + &uuid::Uuid::new_v4().to_string().replace('-', ""),
            );
            let session = Arc::new(UserSession {
                username: user.username,
                nickname: user.nickname.unwrap_or_default(),
                roles: user.roles.unwrap_or_default(),
                extend_infos: user.extend_info.unwrap_or_default(),
            });
            let cache_req = CacheManagerReq::Set {
                key: CacheKey::new(CacheType::UserSession, token.clone()),
                value: CacheValue::UserSession(session),
                ttl: app.sys_config.console_login_timeout,
            };
            app.cache_manager.do_send(cache_req);
            //登录成功后清除登陆限流计数
            let clear_limit_req =
                CacheManagerReq::Remove(CacheKey::new(CacheType::String, limit_key));
            app.cache_manager.do_send(clear_limit_req);
            return Ok(HttpResponse::Ok()
                .cookie(
                    Cookie::build("token", token.as_str())
                        .path("/")
                        .http_only(true)
                        .finish(),
                )
                .cookie(
                    Cookie::build("captcha_token", "")
                        .path("/")
                        .http_only(true)
                        .finish(),
                )
                .json(ApiResult::success(Some(valid))));
        } else {
            return Ok(HttpResponse::Ok()
                .json(ApiResult::<()>::error("USER_CHECK_ERROR".to_owned(), None)));
        }
    }
    Ok(HttpResponse::Ok().json(ApiResult::<()>::error("SYSTEM_ERROR".to_owned(), None)))
}

fn decode_password(password: &str, captcha_token: &str) -> anyhow::Result<String> {
    let password_data = crypto_utils::decode_base64(password)?;
    let password = String::from_utf8(crypto_utils::decrypt_aes128(
        &captcha_token[0..16],
        &captcha_token[16..32],
        &password_data,
    )?)?;
    Ok(password)
}

const WIDTH: u32 = 220;
const HEIGHT: u32 = 120;

pub async fn gen_captcha(app: Data<Arc<AppShareData>>) -> actix_web::Result<impl Responder> {
    //let obj = gen(Difficulty::Easy);
    let mut obj = Captcha::new();
    obj.add_chars(4)
        .apply_filter(Noise::new(0.1))
        .apply_filter(Grid::new(8, 8))
        .view(WIDTH, HEIGHT);
    let mut code = "".to_owned();
    for c in obj.chars() {
        code.push(c);
    }
    let code = Arc::new(code.to_ascii_uppercase());

    let img = obj.as_base64().unwrap_or_default();
    let token = uuid::Uuid::new_v4().to_string().replace('-', "");
    //log::info!("gen_captcha code:{}", &code);
    let cache_req = CacheManagerReq::Set {
        key: CacheKey::new(CacheType::String, Arc::new(format!("Captcha_{}", &token))),
        value: CacheValue::String(code),
        ttl: 300,
    };
    app.cache_manager.send(cache_req).await.ok();
    Ok(HttpResponse::Ok()
        .cookie(
            Cookie::build("captcha_token", token.as_str())
                .path("/")
                .http_only(true)
                .finish(),
        )
        .insert_header(("Captcha-Token", token.as_str()))
        .json(ApiResult::success(Some(img))))
}

pub async fn logout(
    request: HttpRequest,
    app: Data<Arc<AppShareData>>,
) -> actix_web::Result<impl Responder> {
    let token = if let Some(ck) = request.cookie("token") {
        ck.value().to_owned()
    } else if let Some(v) = request.headers().get("Token") {
        v.to_str().unwrap_or_default().to_owned()
    } else {
        "".to_owned()
    };
    let token = Arc::new(token);
    let cache_req = CacheManagerReq::Remove(CacheKey::new(CacheType::UserSession, token));
    app.cache_manager.do_send(cache_req);
    return Ok(HttpResponse::Ok()
        .cookie(
            Cookie::build("token", "")
                .path("/")
                .http_only(true)
                .finish(),
        )
        .json(ApiResult::success(Some(true))));
}
