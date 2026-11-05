use serde_json::json;
use tempfile::tempdir;
use voxy_tool_calling::{ToolContext, ToolRegistry};

#[tokio::test]
async fn test_office_tools_end_to_end() {
    let registry = ToolRegistry::with_builtins();
    let ctx = ToolContext::new("test-office-session").with_confirmation(true);
    let dir = tempdir().unwrap();

    // 1. Create Word document
    let docx_path = dir.path().join("TestDoc.docx").to_string_lossy().to_string();
    let docx_call = registry.get("office_docx_create").await.expect("docx tool");
    let docx_res = docx_call.execute(json!({
        "path": docx_path,
        "title": "Quarterly Operations Review",
        "headings": ["1. Executive Summary", "2. Performance Metrics"],
        "paragraphs": ["Q3 exceeded expectations with strong growth across all sectors."],
        "bullets": ["Revenue +24% YoY", "Churn reduced to 1.2%"],
        "table_headers": ["Metric", "Target", "Actual"],
        "table_rows": [["NPS", "70", "74"], ["Uptime", "99.9%", "99.98%"]]
    }), &ctx).await.expect("docx create");
    assert!(docx_res.success);

    // Validate Word document
    let val_tool = registry.get("office_file_validate").await.expect("validate tool");
    let val_docx = val_tool.execute(json!({
        "path": docx_path,
        "kind": "docx"
    }), &ctx).await.expect("validate docx");
    assert!(val_docx.success);
    assert_eq!(val_docx.data["valid"], true);

    // 2. Create Excel spreadsheet
    let xlsx_path = dir.path().join("Budget.xlsx").to_string_lossy().to_string();
    let xlsx_call = registry.get("office_xlsx_create").await.expect("xlsx tool");
    let xlsx_res = xlsx_call.execute(json!({
        "path": xlsx_path,
        "sheet_name": "Projections",
        "headers": ["Department", "Budget", "Spent"],
        "rows": [["Engineering", "50000", "42000"], ["Marketing", "30000", "28500"]]
    }), &ctx).await.expect("xlsx create");
    assert!(xlsx_res.success);

    // Validate Excel spreadsheet
    let val_xlsx = val_tool.execute(json!({
        "path": xlsx_path,
        "kind": "xlsx"
    }), &ctx).await.expect("validate xlsx");
    assert!(val_xlsx.success);
    assert_eq!(val_xlsx.data["valid"], true);

    // 3. Create PowerPoint presentation
    let pptx_path = dir.path().join("Strategy.pptx").to_string_lossy().to_string();
    let pptx_call = registry.get("office_pptx_create").await.expect("pptx tool");
    let pptx_res = pptx_call.execute(json!({
        "path": pptx_path,
        "title": "2027 Vision Deck",
        "slides": [
            {
                "title": "Autonomous AI Companion",
                "bullet_points": ["Native desktop integration", "Multi-modal reasoning", "Zero-fallback model routing"],
                "notes": "Emphasize local execution and privacy guarantees"
            }
        ]
    }), &ctx).await.expect("pptx create");
    assert!(pptx_res.success);

    // Validate PowerPoint presentation
    let val_pptx = val_tool.execute(json!({
        "path": pptx_path,
        "kind": "pptx"
    }), &ctx).await.expect("validate pptx");
    assert!(val_pptx.success);
    assert_eq!(val_pptx.data["valid"], true);
}

#[tokio::test]
async fn test_calendar_tools_flow() {
    let registry = ToolRegistry::with_builtins();
    let ctx = ToolContext::new("test-calendar-session").with_confirmation(true);

    let create_tool = registry.get("calendar_create_event").await.expect("calendar create");
    let list_tool = registry.get("calendar_list_events").await.expect("calendar list");
    let del_tool = registry.get("calendar_delete_event").await.expect("calendar delete");

    // Create event
    let start_time = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let res = create_tool.execute(json!({
        "title": "Architecture Sync",
        "start_time": start_time,
        "location": "Boardroom A",
        "recurrence": "weekly"
    }), &ctx).await.expect("create event");
    assert!(res.success);
    let event_id = res.data["id"].as_str().unwrap().to_string();

    // List events
    let list_res = list_tool.execute(json!({
        "lookahead_days": 14
    }), &ctx).await.expect("list events");
    assert!(list_res.success);
    let count = list_res.data["count"].as_u64().unwrap();
    assert!(count >= 1);

    // Delete event
    let del_res = del_tool.execute(json!({
        "event_id": event_id
    }), &ctx).await.expect("delete event");
    assert!(del_res.success);
    assert_eq!(del_res.data["deleted"], true);
}

#[tokio::test]
async fn test_mail_and_approval_workflow() {
    let registry = ToolRegistry::with_builtins();
    let ctx = ToolContext::new("test-mail-session").with_confirmation(true);

    let search_tool = registry.get("email_search_inbox").await.expect("search inbox");
    let draft_tool = registry.get("email_create_draft").await.expect("create draft");
    let send_tool = registry.get("email_send_approved").await.expect("send email");

    // Search inbox
    let search_res = search_tool.execute(json!({
        "query": "OSMOO"
    }), &ctx).await.expect("search");
    assert!(search_res.success);
    let count = search_res.data["count"].as_u64().unwrap();
    assert!(count >= 1);

    // Create draft
    let draft_res = draft_tool.execute(json!({
        "to": ["partner@enterprise.com"],
        "subject": "Partnership Agreement Follow-Up",
        "body": "Thank you for the productive call today. Attached is the proposed schedule."
    }), &ctx).await.expect("create draft");
    assert!(draft_res.success);
    let draft_id = draft_res.data["draft_id"].as_str().unwrap();

    // Send approved draft
    let send_res = send_tool.execute(json!({
        "draft_id": draft_id,
        "approval_token": "token_approved_by_user"
    }), &ctx).await.expect("send email");
    assert!(send_res.success);
    assert_eq!(send_res.data["status"], "sent");
}

#[tokio::test]
async fn test_travel_shopping_and_sandbox_payments() {
    let registry = ToolRegistry::with_builtins();
    let ctx = ToolContext::new("test-travel-session").with_confirmation(true);

    // Flights
    let flight_tool = registry.get("travel_search_flights").await.expect("flight tool");
    let flight_res = flight_tool.execute(json!({
        "origin": "SFO",
        "destination": "JFK",
        "date": "2026-11-15",
        "passengers": 2
    }), &ctx).await.expect("flights");
    assert!(flight_res.success);
    assert!(flight_res.data["count"].as_u64().unwrap() >= 1);

    // Hotels
    let hotel_tool = registry.get("travel_search_hotels").await.expect("hotel tool");
    let hotel_res = hotel_tool.execute(json!({
        "city": "New York",
        "check_in": "2026-11-15",
        "check_out": "2026-11-18"
    }), &ctx).await.expect("hotels");
    assert!(hotel_res.success);
    assert!(hotel_res.data["count"].as_u64().unwrap() >= 1);

    // Products
    let shop_tool = registry.get("shopping_discover_products").await.expect("shop tool");
    let shop_res = shop_tool.execute(json!({
        "query": "Ergonomic Mechanical Keyboard",
        "max_results": 3
    }), &ctx).await.expect("shop");
    assert!(shop_res.success);
    assert!(shop_res.data["count"].as_u64().unwrap() >= 1);

    // Sandbox payment (only allows tok_sandbox_*)
    let pay_tool = registry.get("payment_execute_sandbox_test").await.expect("payment tool");
    let pay_res = pay_tool.execute(json!({
        "token": "tok_sandbox_mastercard_9999",
        "amount": 149.99
    }), &ctx).await.expect("payment");
    assert!(pay_res.success);
    assert_eq!(pay_res.data["status"], "success");

    // Real card attempt must be strictly rejected
    let real_card_res = pay_tool.execute(json!({
        "token": "4111222233334444",
        "amount": 100.00
    }), &ctx).await;
    assert!(real_card_res.is_err(), "Raw card numbers must be strictly rejected");
}

#[tokio::test]
async fn test_browser_ssrf_redirect_defense_and_metadata_blocking() {
    let registry = ToolRegistry::with_builtins();
    let ctx = ToolContext::new("test-browser-security-session");

    let inspect_tool = registry.get("browser_fetch").await.expect("browser_fetch tool");

    // 1. Direct metadata attack blocked
    let direct_meta = inspect_tool.execute(json!({
        "url": "http://169.254.169.254/latest/meta-data/"
    }), &ctx).await;
    assert!(direct_meta.is_err(), "Direct IMDS access must fail with security violation");

    // 2. Loopback attack blocked
    let loopback = inspect_tool.execute(json!({
        "url": "http://127.0.0.1:8080/admin"
    }), &ctx).await;
    assert!(loopback.is_err(), "Loopback access must fail");

    // 3. IPv4-mapped IPv6 loopback attack blocked
    let mapped_loopback = inspect_tool.execute(json!({
        "url": "http://[::ffff:127.0.0.1]:8080/status"
    }), &ctx).await;
    assert!(mapped_loopback.is_err(), "IPv4-mapped IPv6 loopback must fail");

    // 4. Safe HTTP client builds with DNS resolver and redirect policy
    let safe_client = voxy_tool_calling::builtin::browser_runtime::BrowserRuntime::create_safe_http_client(
        std::time::Duration::from_millis(500),
    );
    assert!(safe_client.is_ok(), "Safe HTTP client should build successfully");
}

#[tokio::test]
async fn test_ssrf_adversarial_unusual_representations_and_namespaces() {
    let registry = ToolRegistry::with_builtins();
    let ctx = ToolContext::new("test-ssrf-adversarial");
    let fetch_tool = registry.get("browser_fetch").await.expect("fetch tool");

    let adversarial_targets = [
        // Unusual address encodings that resolve to 127.0.0.1 (loopback)
        "http://2130706433/",
        "http://0x7f000001/",
        "http://0177.0.0.1/",
        // Link-local & cloud metadata
        "http://169.254.1.1/",
        "http://169.254.169.254/",
        "http://169.254.170.2/v2/metadata",
        "http://100.100.100.200/latest/meta-data",
        "http://[fd00:ec2::254]/",
        // RFC 1918 private subnets
        "http://10.0.0.1:8080/admin",
        "http://172.16.0.1/",
        "http://192.168.1.1:80/",
        // Restricted local / internal namespaces
        "http://localhost:3000/",
        "http://service.local/",
        "http://vault.internal/secrets",
        "http://corp.lan/",
        "http://gateway.home.arpa/",
        "http://control.intranet/",
        "http://metadata.google.internal/",
        // IPv6 loopback
        "http://[::1]:8000/",
        // Unsupported schemes
        "file:///C:/Windows/System32/drivers/etc/hosts",
        "gopher://127.0.0.1:70/",
        "ftp://example.com/test.txt",
    ];

    for target in adversarial_targets {
        let res = fetch_tool.execute(json!({ "url": target }), &ctx).await;
        assert!(
            res.is_err(),
            "Adversarial SSRF target '{target}' must be rejected by security boundary",
        );
    }
}

#[tokio::test]
async fn test_ssrf_dns_resolver_connection_binding() {
    use reqwest::dns::Resolve;
    use std::str::FromStr;

    let resolver = voxy_tool_calling::builtin::browser_runtime::SsrfSafeDnsResolver;

    // Test that the socket resolver blocks restricted destinations at connection time
    let blocked_hosts = [
        "127.0.0.1",
        "169.254.169.254",
        "10.0.0.1",
        "192.168.0.1",
        "localhost",
        "metadata.google.internal",
        "instance-data",
        "db.internal",
        "switch.local",
        "nas.corp",
        "ap.lan",
        "router.home.arpa",
        "::1",
    ];

    for host in blocked_hosts {
        let name = reqwest::dns::Name::from_str(host).expect("valid name");
        let res = resolver.resolve(name).await;
        assert!(
            res.is_err(),
            "SsrfSafeDnsResolver must block restricted host '{host}' at connection time"
        );
    }
}
