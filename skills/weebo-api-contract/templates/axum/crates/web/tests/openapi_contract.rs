//! source: ProxyAuthK8S@8fe220b (libs/server/api/tests/openapi_contract.rs), trimmed.
//!
//! The contract gate: every 2xx response but 204 declares its body, or is
//! listed in `NO_BODY` with the reason. A response without a body reaches the
//! generated client (HeyApi) as `unknown`. An exemption that no longer matches
//! a bodiless response fails too, so the list cannot rot.

#![expect(
    clippy::expect_used,
    reason = "test helpers: a panic is a failed test, with its message"
)]

/// `(method, path, status, reason)` answered without a declared body.
/// ProxyAuthK8S lists its empty liveness `200` and the proxy relays here.
const NO_BODY: &[(&str, &str, &str, &str)] = &[];

/// The vacuous-pass guard: an empty spec passes the body check. Raise it as
/// the API grows, a little under the real count.
const MIN_ROUTES: usize = 1;

type Exemptions<'a> = &'a [(&'a str, &'a str, &'a str, &'a str)];

/// Every 2xx response but 204, as `(method, path, status, has_body)`.
fn success_responses(document: &serde_json::Value) -> Vec<(String, String, String, bool)> {
    let mut found = Vec::new();
    for (path, item) in document["paths"].as_object().expect("paths") {
        for (method, operation) in item.as_object().expect("path item") {
            let Some(responses) = operation.get("responses").and_then(|r| r.as_object()) else {
                continue;
            };
            for (status, response) in responses {
                if status.starts_with('2') && status != "204" {
                    let has_body = response
                        .get("content")
                        .and_then(|content| content.as_object())
                        .is_some_and(|content| !content.is_empty());
                    found.push((method.to_uppercase(), path.clone(), status.clone(), has_body));
                }
            }
        }
    }
    found
}

/// A response without a body and without an exemption, or a stale exemption.
fn violations(responses: &[(String, String, String, bool)], no_body: Exemptions) -> Vec<String> {
    let exempt = |method: &str, path: &str, status: &str| {
        no_body.iter().any(|(m, p, s, _)| *m == method && *p == path && *s == status)
    };
    let mut problems: Vec<String> = responses
        .iter()
        .filter(|(method, path, status, has_body)| !has_body && !exempt(method, path, status))
        .map(|(method, path, status, _)| {
            format!(
                "openapi_contract: {method} {path} {status} declares no body \
                 (add body = T to its #[utoipa::path], or list it in NO_BODY with a reason)"
            )
        })
        .collect();
    for (method, path, status, _) in no_body {
        let still_bodiless = responses
            .iter()
            .any(|(m, p, s, has_body)| m == method && p == path && s == status && !has_body);
        if !still_bodiless {
            problems.push(format!(
                "openapi_contract: NO_BODY lists {method} {path} {status}, which is gone or now has a body; drop the entry"
            ));
        }
    }
    problems
}

#[test]
fn every_success_response_declares_its_body() {
    let document = serde_json::to_value(my_crate::openapi_spec()).expect("the document serializes");
    let problems = violations(&success_responses(&document), NO_BODY);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_spec_is_not_empty() {
    let paths = my_crate::openapi_spec().paths.paths.len();
    assert!(paths >= MIN_ROUTES, "expected the full route set, got {paths} paths");
}

/// The guard can fail: a bodiless response it was not told about is reported,
/// and so is an exemption left behind.
#[test]
fn the_guard_reports_a_bodiless_response_and_a_stale_exemption() {
    let exemptions: Exemptions = &[("GET", "/health", "200", "empty liveness")];
    let mut responses = vec![
        ("GET".into(), "/health".into(), "200".into(), false),
        ("POST".into(), "/api/v1/new".into(), "201".into(), false),
    ];
    let problems = violations(&responses, exemptions);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("POST /api/v1/new 201 declares no body"));

    responses.retain(|(_, path, _, _)| path != "/health");
    let problems = violations(&responses, exemptions);
    assert!(problems.iter().any(|p| p.contains("NO_BODY lists GET /health")));
}
