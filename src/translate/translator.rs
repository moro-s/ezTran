use super::engine::{EngineConfig, EngineKind};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// 翻译结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResult {
    /// 译文
    pub text: String,
    /// 源语言
    pub from: String,
    /// 目标语言
    pub to: String,
    /// 使用的引擎名
    pub engine: String,
}

/// 翻译器：根据引擎配置执行翻译
pub struct Translator;

impl Translator {
    /// 执行翻译（同步阻塞）
    pub fn translate(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        log::info!(
            "[translate] 开始翻译: engine={} kind={} text_len={} from={} to={}",
            engine.name, engine.kind.label(), text.len(), from, to
        );
        let result = match engine.kind {
            EngineKind::Youdao => Self::translate_youdao(engine, text, from, to),
            EngineKind::Baidu => Self::translate_baidu(engine, text, from, to),
            EngineKind::DeepL => Self::translate_deepl(engine, text, from, to),
            EngineKind::Custom => Self::translate_custom(engine, text, from, to),
        };
        match &result {
            Ok(r) => log::info!("[translate] 翻译成功: engine={} result_len={}", engine.name, r.text.len()),
            Err(e) => log::warn!("[translate] 翻译失败: engine={} error={}", engine.name, e),
        }
        result
    }

    fn translate_youdao(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.api_key.is_empty() || engine.api_secret.is_empty() {
            anyhow::bail!("有道翻译需要配置 API Key 和 API Secret");
        }

        // 有道智云 API: https://openapi.youdao.com/api
        // v3 签名: sign = sha256(appKey + input + salt + curtime + appSecret)
        // input: q 长度 > 20 时取前10 + 长度 + 后10，否则为 q 本身
        let salt = gen_salt();
        let curtime = gen_curtime();
        let input = youdao_sign_input(text);
        let sign_input = format!("{}{}{}{}{}", engine.api_key, input, salt, curtime, engine.api_secret);
        let sign = sha256_hex(&sign_input);
        log::info!(
            "[translate][youdao] 签名: salt={} curtime={} input_len={} sign_prefix={}",
            salt, curtime, input.len(), &sign[..8]
        );

        let url = format!(
            "https://openapi.youdao.com/api?q={}&from={}&to={}&appKey={}&salt={}&signType=v3&curtime={}&sign={}",
            url_encode(text),
            from,
            to,
            engine.api_key,
            salt,
            curtime,
            sign
        );

        let resp = http_get_json(&url, "有道翻译")?;
        log::debug!("[translate][youdao] 收到响应: keys={}", resp_keys(&resp));

        // 有道 API 即使 HTTP 200 也可能返回错误 errorCode
        if let Some(msg) = youdao_check_error(&resp) {
            anyhow::bail!("有道翻译错误: {}", msg);
        }

        let translated = resp
            .get("translation")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }

    fn translate_baidu(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.api_key.is_empty() || engine.api_secret.is_empty() {
            anyhow::bail!("百度翻译需要配置 App ID 和 App Secret");
        }

        // 百度翻译 API: https://fanyi-api.baidu.com/api/trans/vip/translate
        // sign = md5(appid + q + salt + key)
        // api_key = App ID, api_secret = 密钥
        let salt = gen_salt();
        let sign_input = format!("{}{}{}{}", engine.api_key, text, salt, engine.api_secret);
        let sign = md5_hex(&sign_input);
        log::debug!("[translate][baidu] 签名完成: salt={} sign_len={}", salt, sign.len());

        let url = format!(
            "https://fanyi-api.baidu.com/api/trans/vip/translate?q={}&from={}&to={}&appid={}&salt={}&sign={}",
            url_encode(text),
            from,
            to,
            engine.api_key,
            salt,
            sign
        );

        let resp = http_get_json(&url, "百度翻译")?;
        log::debug!("[translate][baidu] 收到响应: keys={}", resp_keys(&resp));

        // 百度 API 即使 HTTP 200 也可能返回 error_code
        if let Some(msg) = baidu_check_error(&resp) {
            anyhow::bail!("百度翻译错误: {}", msg);
        }

        let translated = resp
            .get("trans_result")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("dst"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }

    fn translate_deepl(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.api_key.is_empty() {
            anyhow::bail!("DeepL 需要配置 API Key");
        }

        // DeepL API (free): https://api-free.deepl.com/v2/translate
        // DeepL API (pro): https://api.deepl.com/v2/translate
        let endpoint = if engine.endpoint.is_empty() {
            "https://api-free.deepl.com/v2/translate".to_string()
        } else {
            engine.endpoint.clone()
        };

        let url = format!(
            "{}?text={}&target_lang={}",
            endpoint,
            url_encode(text),
            to.to_uppercase()
        );
        log::debug!("[translate][deepl] endpoint={}", endpoint);

        let resp = http_get_json_with_auth(&url, &engine.api_key, "DeepL-Auth-Key", "DeepL")?;
        log::debug!("[translate][deepl] 收到响应: keys={}", resp_keys(&resp));
        let translated = resp
            .get("translations")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }

    fn translate_custom(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.endpoint.is_empty() {
            anyhow::bail!("自定义引擎需要配置 Endpoint URL");
        }

        let url = format!(
            "{}/translate?q={}&from={}&to={}",
            engine.endpoint.trim_end_matches('/'),
            url_encode(text),
            from,
            to
        );

        let resp = http_get_json_with_auth_opt(&url, &engine.api_key, "自定义引擎")?;
        log::debug!("[translate][custom] 收到响应: keys={}", resp_keys(&resp));
        let translated = resp
            .get("text")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }
}

// ── 签名与工具函数 ──

/// 生成 salt（基于时间戳的简单随机数）
fn gen_salt() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
        .to_string()
}

/// 生成当前时间戳（秒级，用于有道 v1 签名）
fn gen_curtime() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string()
}

/// 有道签名 input 计算：文本长度 > 20 时取前10字符 + 长度 + 后10字符
fn youdao_sign_input(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > 20 {
        let head: String = chars[..10].iter().collect();
        let tail: String = chars[chars.len() - 10..].iter().collect();
        format!("{}{}{}", head, chars.len(), tail)
    } else {
        text.to_string()
    }
}

/// 计算 SHA-256 十六进制摘要
fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 计算 MD5 十六进制摘要
fn md5_hex(input: &str) -> String {
    use md5::Md5;
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 有道翻译 errorCode 映射为可读信息
fn youdao_error_msg(code: &str) -> &'static str {
    match code {
        "1" => "源语言不支持",
        "2" => "引擎处理失败",
        "3" => "用户未登录",
        "4" => "翻译数据超长",
        "5" => "签名异常",
        "10" => "词库不存在",
        "11" => "查询失败",
        "12" => "翻译服务故障",
        "13" => "翻译结果过长",
        "30" => "无法翻译句子",
        "31" => "翻译服务异常",
        "32" => "翻译结果异常",
        "40" => "语言不匹配",
        "41" => "语言不支持",
        "42" => "句子过长",
        "50" => "密钥异常",
        "51" => "词库异常",
        "52" => "签名验证失败",
        "101" => "缺少必填参数",
        "102" => "不支持的语言类型",
        "103" => "翻译文本过长",
        "104" => "不支持的功能",
        "105" => "该功能已下线",
        "106" => "请求失败",
        "107" => "密钥不合法",
        "108" => "appid 不存在",
        "109" => "签名无效",
        "110" => "访问频率受限",
        "111" => "请求过多",
        "112" => "账户欠费",
        "113" => "账号被停用",
        "201" => "密钥不合法",
        "202" => "签名检验失败",
        "203" => "访问IP不在白名单",
        "205" => "请求来源不合法",
        "206" => "签名校验失败",
        "207" => "账户欠费",
        "208" => "账号被停用",
        _ => "未知错误",
    }
}

/// 检查有道 API 返回的 errorCode 是否为错误（兼容字符串和数字类型）
fn youdao_check_error(resp: &serde_json::Value) -> Option<&'static str> {
    let code = resp
        .get("errorCode")
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())));
    match code.as_deref() {
        Some("0") | None => None,
        Some(c) => {
            log::warn!("[translate][youdao] errorCode={} resp={}", c, resp);
            Some(youdao_error_msg(c))
        }
    }
}

/// 检查百度 API 返回的 error_code 是否为错误（兼容字符串和数字类型）
fn baidu_check_error(resp: &serde_json::Value) -> Option<String> {
    let has_error = resp
        .get("error_code")
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())));
    if has_error.is_some() {
        let msg = resp
            .get("error_msg")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        Some(msg.to_string())
    } else {
        None
    }
}

// ── HTTP 工具函数（基于 minreq）──
// 所有错误信息均不含 URL 或密钥等敏感信息

fn http_get_json(url: &str, context: &str) -> anyhow::Result<serde_json::Value> {
    log::debug!("[http] {} 发送 GET 请求", context);
    let response = minreq::get(url).send()?;
    let status = response.status_code;
    let body = response.as_str().unwrap_or("").to_string();
    log::debug!("[http] {} 响应: status={} body_len={}", context, status, body.len());
    if status >= 400 {
        log::warn!("[http] {} 请求失败: HTTP {}", context, status);
        anyhow::bail!("{}请求失败 (HTTP {})", context, status);
    }
    let value: serde_json::Value = serde_json::from_str(&body)?;
    Ok(value)
}

fn http_get_json_with_auth(
    url: &str,
    api_key: &str,
    auth_prefix: &str,
    context: &str,
) -> anyhow::Result<serde_json::Value> {
    log::debug!("[http] {} 发送 GET 请求 (auth={})", context, auth_prefix);
    let response = minreq::get(url)
        .with_header("Authorization", format!("{} {}", auth_prefix, api_key))
        .send()?;
    let status = response.status_code;
    let body = response.as_str().unwrap_or("").to_string();
    log::debug!("[http] {} 响应: status={} body_len={}", context, status, body.len());
    if status >= 400 {
        log::warn!("[http] {} 请求失败: HTTP {}", context, status);
        anyhow::bail!("{}请求失败 (HTTP {})", context, status);
    }
    let value: serde_json::Value = serde_json::from_str(&body)?;
    Ok(value)
}

fn http_get_json_with_auth_opt(
    url: &str,
    api_key: &str,
    context: &str,
) -> anyhow::Result<serde_json::Value> {
    let has_auth = !api_key.is_empty();
    log::debug!("[http] {} 发送 GET 请求 (bearer={})", context, has_auth);
    let mut req = minreq::get(url);
    if has_auth {
        req = req.with_header("Authorization", format!("Bearer {}", api_key));
    }
    let response = req.send()?;
    let status = response.status_code;
    let body = response.as_str().unwrap_or("").to_string();
    log::debug!("[http] {} 响应: status={} body_len={}", context, status, body.len());
    if status >= 400 {
        log::warn!("[http] {} 请求失败: HTTP {}", context, status);
        anyhow::bail!("{}请求失败 (HTTP {})", context, status);
    }
    let value: serde_json::Value = serde_json::from_str(&body)?;
    Ok(value)
}

/// 提取 JSON 响应的顶层 key 列表（用于日志，不含值）
fn resp_keys(resp: &serde_json::Value) -> String {
    if let Some(obj) = resp.as_object() {
        let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        keys.join(",")
    } else {
        "(non-object)".to_string()
    }
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                (b as char).to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── url_encode 测试 ──

    #[test]
    fn test_url_encode_ascii() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a+b=c"), "a%2Bb%3Dc");
    }

    #[test]
    fn test_url_encode_chinese() {
        // "你好" 的 UTF-8 编码: E4 BD A0 E5 A5 BD
        assert_eq!(url_encode("你好"), "%E4%BD%A0%E5%A5%BD");
        assert_eq!(url_encode("翻译"), "%E7%BF%BB%E8%AF%91");
    }

    #[test]
    fn test_url_encode_mixed() {
        // 中英混合
        assert_eq!(url_encode("hello世界"), "hello%E4%B8%96%E7%95%8C");
    }

    #[test]
    fn test_url_encode_safe_chars() {
        assert_eq!(url_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    // ── sha256_hex 测试 ──

    #[test]
    fn test_sha256_hex_known() {
        // sha256("abc") 的已知值
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_sha256_hex_empty() {
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_youdao_sign_format() {
        // 验证签名输入格式: appKey + q + salt + appSecret
        let app_key = "test_key";
        let q = "hello";
        let salt = "123456";
        let app_secret = "test_secret";
        let sign_input = format!("{}{}{}{}", app_key, q, salt, app_secret);
        assert_eq!(sign_input, "test_keyhello123456test_secret");
        // 签名应该是一个 64 字符的十六进制字符串
        let sign = sha256_hex(&sign_input);
        assert_eq!(sign.len(), 64);
        assert!(sign.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_youdao_sign_input_short() {
        // 短文本（<= 20 字符）直接返回原文
        assert_eq!(youdao_sign_input("hello"), "hello");
        assert_eq!(youdao_sign_input("你好世界"), "你好世界");
        assert_eq!(youdao_sign_input("12345678901234567890"), "12345678901234567890");
    }

    #[test]
    fn test_youdao_sign_input_long() {
        // 长文本（> 20 字符）取前10 + 长度 + 后10
        let text = "abcdefghijklmnopqrstuvwxyz"; // 26 字符
        let input = youdao_sign_input(text);
        assert_eq!(input, "abcdefghij26qrstuvwxyz");
    }

    #[test]
    fn test_youdao_sign_input_long_unicode() {
        // 中英文混合长文本
        let text = "你好世界这是一段测试文本用于验证签名逻辑abcdef";
        let chars: Vec<char> = text.chars().collect();
        let input = youdao_sign_input(text);
        // 前10字符 + 长度 + 后10字符
        let head: String = chars[..10].iter().collect();
        let tail: String = chars[chars.len() - 10..].iter().collect();
        assert_eq!(input, format!("{}{}{}", head, chars.len(), tail));
    }

    // ── md5_hex 测试 ──

    #[test]
    fn test_md5_hex_known() {
        // md5("abc") 的已知值
        assert_eq!(
            md5_hex("abc"),
            "900150983cd24fb0d6963f7d28e17f72"
        );
    }

    #[test]
    fn test_md5_hex_empty() {
        assert_eq!(
            md5_hex(""),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
    }

    #[test]
    fn test_baidu_sign_format() {
        // 验证签名输入格式: appid + q + salt + key
        let appid = "appid123";
        let q = "你好";
        let salt = "999";
        let key = "secret456";
        let sign_input = format!("{}{}{}{}", appid, q, salt, key);
        assert_eq!(sign_input, "appid123你好999secret456");
        let sign = md5_hex(&sign_input);
        assert_eq!(sign.len(), 32);
        assert!(sign.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ── 有道 errorCode 检查测试 ──

    #[test]
    fn test_youdao_check_error_success_str() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"errorCode": "0", "translation": ["hello"]}"#).unwrap();
        assert!(youdao_check_error(&resp).is_none());
    }

    #[test]
    fn test_youdao_check_error_success_num() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"errorCode": 0, "translation": ["hello"]}"#).unwrap();
        assert!(youdao_check_error(&resp).is_none());
    }

    #[test]
    fn test_youdao_check_error_error_str() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"errorCode": "50"}"#).unwrap();
        assert_eq!(youdao_check_error(&resp), Some("密钥异常"));
    }

    #[test]
    fn test_youdao_check_error_error_num() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"errorCode": 52}"#).unwrap();
        assert_eq!(youdao_check_error(&resp), Some("签名验证失败"));
    }

    #[test]
    fn test_youdao_check_error_missing() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"translation": ["hello"]}"#).unwrap();
        assert!(youdao_check_error(&resp).is_none());
    }

    // ── 百度 error_code 检查测试 ──

    #[test]
    fn test_baidu_check_error_success() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"trans_result": [{"dst": "hello"}]}"#).unwrap();
        assert!(baidu_check_error(&resp).is_none());
    }

    #[test]
    fn test_baidu_check_error_error_str() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"error_code": "54001", "error_msg": "签名错误"}"#).unwrap();
        assert_eq!(baidu_check_error(&resp), Some("签名错误".to_string()));
    }

    #[test]
    fn test_baidu_check_error_error_num() {
        let resp: serde_json::Value = serde_json::from_str(r#"{"error_code": 54001, "error_msg": "签名错误"}"#).unwrap();
        assert_eq!(baidu_check_error(&resp), Some("签名错误".to_string()));
    }

    // ── 有道翻译响应解析测试 ──

    #[test]
    fn test_youdao_parse_translation() {
        let resp: serde_json::Value = serde_json::from_str(
            r#"{"errorCode": "0", "translation": ["你好世界"]}"#
        ).unwrap();
        assert!(youdao_check_error(&resp).is_none());
        let translated = resp
            .get("translation")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.as_str())
            .unwrap_or("");
        assert_eq!(translated, "你好世界");
    }

    #[test]
    fn test_youdao_parse_error_response() {
        let resp: serde_json::Value = serde_json::from_str(
            r#"{"errorCode": "50", "msg": "Invalid Key"}"#
        ).unwrap();
        assert_eq!(youdao_check_error(&resp), Some("密钥异常"));
    }

    // ── 百度翻译响应解析测试 ──

    #[test]
    fn test_baidu_parse_translation() {
        let resp: serde_json::Value = serde_json::from_str(
            r#"{"from": "en", "to": "zh", "trans_result": [{"src": "hello", "dst": "你好"}]}"#
        ).unwrap();
        assert!(baidu_check_error(&resp).is_none());
        let translated = resp
            .get("trans_result")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("dst"))
            .and_then(|s| s.as_str())
            .unwrap_or("");
        assert_eq!(translated, "你好");
    }

    // ── Translator::translate 缺失密钥测试 ──

    #[test]
    fn test_translate_youdao_missing_key() {
        let engine = EngineConfig {
            kind: EngineKind::Youdao,
            name: "test".into(),
            enabled: true,
            api_key: String::new(),
            api_secret: String::new(),
            endpoint: String::new(),
        };
        let result = Translator::translate(&engine, "hello", "en", "zh");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API Key"));
    }

    #[test]
    fn test_translate_deepl_missing_key() {
        let engine = EngineConfig {
            kind: EngineKind::DeepL,
            name: "test".into(),
            enabled: true,
            api_key: String::new(),
            api_secret: String::new(),
            endpoint: String::new(),
        };
        let result = Translator::translate(&engine, "hello", "en", "zh");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API Key"));
    }

    #[test]
    fn test_translate_custom_missing_endpoint() {
        let engine = EngineConfig {
            kind: EngineKind::Custom,
            name: "test".into(),
            enabled: true,
            api_key: String::new(),
            api_secret: String::new(),
            endpoint: String::new(),
        };
        let result = Translator::translate(&engine, "hello", "en", "zh");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Endpoint"));
    }

    // ── gen_salt 测试 ──

    #[test]
    fn test_gen_salt_is_string() {
        let salt = gen_salt();
        assert!(!salt.is_empty());
        assert!(salt.chars().all(|c| c.is_ascii_digit()));
    }

    // ── youdao_error_msg 测试 ──

    #[test]
    fn test_youdao_error_msg_known() {
        assert_eq!(youdao_error_msg("50"), "密钥异常");
        assert_eq!(youdao_error_msg("52"), "签名验证失败");
        assert_eq!(youdao_error_msg("5"), "签名异常");
    }

    #[test]
    fn test_youdao_error_msg_unknown() {
        assert_eq!(youdao_error_msg("999"), "未知错误");
        assert_eq!(youdao_error_msg("abc"), "未知错误");
    }

    // ── v3 签名逻辑测试 ──

    #[test]
    fn test_youdao_v3_sign_format() {
        // v3 签名: sha256(appKey + input + salt + curtime + appSecret)
        let app_key = "test_key";
        let q = "hello";
        let salt = "123456";
        let curtime = "1700000000";
        let app_secret = "test_secret";
        let sign_input = format!("{}{}{}{}{}", app_key, q, salt, curtime, app_secret);
        assert_eq!(sign_input, "test_keyhello1234561700000000test_secret");
        let sign = sha256_hex(&sign_input);
        assert_eq!(sign.len(), 64);
        assert!(sign.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_gen_curtime_is_digits() {
        let curtime = gen_curtime();
        assert!(!curtime.is_empty());
        assert!(curtime.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_youdao_v3_sign_with_truncation() {
        // 长文本 v3 签名: input 使用截断后的值
        let app_key = "mykey";
        let text = "abcdefghijklmnopqrstuvwxyz"; // 26 字符
        let salt = "999";
        let curtime = "1700000000";
        let app_secret = "mysecret";
        let input = youdao_sign_input(text);
        assert_eq!(input, "abcdefghij26qrstuvwxyz");
        let sign_input = format!("{}{}{}{}{}", app_key, input, salt, curtime, app_secret);
        let sign = sha256_hex(&sign_input);
        assert_eq!(sign.len(), 64);
    }

    // ── 有道 API 集成测试（需要网络，默认忽略）──

    #[test]
#[ignore = "需要网络连接和有效的有道 API 密钥，运行: cargo test test_youdao_api_live -- --ignored --nocapture"]
    fn test_youdao_api_live() {
        let engine = EngineConfig {
            kind: EngineKind::Youdao,
            name: "test".into(),
            enabled: true,
            api_key: "4f93c8a4af09ce7f".into(),
            api_secret: "nDdhiptKZzOOeIKRChIV4wVYKC52T8b3".into(),
            endpoint: String::new(),
        };
        let result = Translator::translate_youdao(&engine, "hello", "en", "zh-CHS");
        match &result {
            Ok(r) => println!("[live] 有道翻译成功: text={}", r.text),
            Err(e) => println!("[live] 有道翻译失败: {}", e),
        }
        // 不做严格断言，只观察实际 API 返回
    }

    #[test]
#[ignore = "需要网络连接和有效的百度 API 密钥，运行: cargo test test_baidu_api_live -- --ignored --nocapture"]
    fn test_baidu_api_live() {
        let engine = EngineConfig {
            kind: EngineKind::Baidu,
            name: "test".into(),
            enabled: true,
            api_key: "3f950365af0abfbe".into(),
            api_secret: "nDdhiptKZzOOeIKRChIV4wVYKC52T8b3".into(),
            endpoint: String::new(),
        };
        let result = Translator::translate_baidu(&engine, "hello", "en", "zh");
        match &result {
            Ok(r) => println!("[live] 百度翻译成功: text={}", r.text),
            Err(e) => println!("[live] 百度翻译失败: {}", e),
        }
    }
}
