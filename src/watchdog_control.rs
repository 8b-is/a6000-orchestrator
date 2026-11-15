//! Explicit authorization for HTTP operations that probe or restart fleet services.

pub fn is_control_path(path: &str) -> bool {
    matches!(
        path,
        "/api/watchdog/check"
            | "/api/watchdog/heal"
            | "/a6000/api/watchdog/check"
            | "/a6000/api/watchdog/heal"
    )
}

pub fn authorize(
    method: &str,
    request: &str,
    configured: Option<&str>,
) -> Result<(), &'static str> {
    if method != "POST" {
        return Err("method_not_allowed");
    }
    let Some(expected) =
        configured.filter(|token| token.len() >= 32 && token.bytes().all(|b| b.is_ascii_graphic()))
    else {
        return Err("control_disabled");
    };
    let mut headers = request
        .lines()
        .skip(1)
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("authorization"));
    let Some((_, value)) = headers.next() else {
        return Err("unauthorized");
    };
    if headers.next().is_some() {
        return Err("unauthorized");
    }
    let Some((scheme, token)) = value.trim().split_once(' ') else {
        return Err("unauthorized");
    };
    if !scheme.eq_ignore_ascii_case("Bearer") || token.len() != expected.len() {
        return Err("unauthorized");
    }
    let difference = token
        .bytes()
        .zip(expected.bytes())
        .fold(0_u8, |diff, (a, b)| diff | (a ^ b));
    if difference == 0 {
        Ok(())
    } else {
        Err("unauthorized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TOKEN: &str = "fixture-token-with-at-least-32-bytes";

    #[test]
    fn requires_post_and_an_explicit_token() {
        let valid =
            format!("POST /api/watchdog/heal HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
        assert!(authorize("POST", &valid, Some(TOKEN)).is_ok());
        for method in ["GET", "HEAD", "PUT"] {
            assert_eq!(
                authorize(method, &valid, Some(TOKEN)),
                Err("method_not_allowed")
            );
        }
        assert_eq!(authorize("POST", &valid, None), Err("control_disabled"));
        assert_eq!(
            authorize("POST", &valid, Some("short")),
            Err("control_disabled")
        );
        assert!(authorize(
            "POST",
            &valid,
            Some("different-token-with-at-least-32-bytes")
        )
        .is_err());
    }

    #[test]
    fn rejects_duplicate_headers_and_body_impersonation() {
        let duplicate = format!("POST / HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
        assert!(authorize("POST", &duplicate, Some(TOKEN)).is_err());
        let body = format!("POST / HTTP/1.1\r\n\r\nAuthorization: Bearer {TOKEN}");
        assert!(authorize("POST", &body, Some(TOKEN)).is_err());
        assert!(is_control_path("/api/watchdog/heal"));
        assert!(is_control_path("/a6000/api/watchdog/check"));
        assert!(!is_control_path("/api/watchdog/status"));
    }
}
