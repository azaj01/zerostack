use crate::permission::checker::{CheckResult, PermissionChecker};
use crate::permission::{Action, PermissionConfig, PermissionConfigs, SecurityMode, ToolPerm};

fn make_checker(mode: SecurityMode) -> PermissionChecker {
    PermissionChecker::new(
        &PermissionConfigs::default(),
        mode,
        Some(std::path::PathBuf::from("/home/user/project")),
    )
}

fn configs_from(config: PermissionConfig) -> PermissionConfigs {
    PermissionConfigs::from(config)
}

// --- SecurityMode behavior ---

#[test]
fn yolo_allows_everything() {
    let mut checker = make_checker(SecurityMode::Yolo);
    assert_eq!(checker.check("bash", "rm -rf /"), CheckResult::Allowed);
    assert_eq!(checker.check("write", "/etc/passwd"), CheckResult::Allowed);
}

#[test]
fn restrictive_makes_unconfigured_tool_ask() {
    let mut checker = make_checker(SecurityMode::Restrictive);
    let result = checker.check("some_tool", "any input");
    assert!(matches!(result, CheckResult::Ask));
}

#[test]
fn standard_allows_unknown_tool_with_default() {
    let mut checker = make_checker(SecurityMode::Standard);
    let result = checker.check("some_tool", "any input");
    assert!(matches!(result, CheckResult::Allowed));
}

#[test]
fn accept_auto_allows_inside_working_dir() {
    let config = PermissionConfig {
        write: Some(ToolPerm::Simple(Action::Ask)),
        ..PermissionConfig::default()
    };
    let mut checker = PermissionChecker::new(
        &configs_from(config),
        SecurityMode::Accept,
        Some(std::path::PathBuf::from("/home/user/project")),
    );
    let result = checker.check_path("write", "/home/user/project/src/main.rs");
    assert!(matches!(result, CheckResult::Allowed));
}

#[test]
fn accept_asks_for_external_path() {
    let mut checker = make_checker(SecurityMode::Accept);
    let external_path = if cfg!(windows) {
        "D:\\outside\\file.txt"
    } else {
        "/etc/config.conf"
    };
    let result = checker.check_path("write", external_path);
    assert!(
        matches!(result, CheckResult::Ask),
        "expected Ask, got {:?} for path: {}",
        result,
        external_path,
    );
}

// --- Deny rules ---

#[test]
fn deny_rule_blocks_regardless_of_mode() {
    let mut checker = make_checker(SecurityMode::Standard);
    let result = checker.check("bash", "rm -rf /home/user/project");
    assert!(matches!(result, CheckResult::Denied(_)));
}

#[test]
fn deny_rule_not_blocked_by_yolo() {
    let mut checker = make_checker(SecurityMode::Yolo);
    let result = checker.check("bash", "rm -rf /home/user/project");
    assert!(matches!(result, CheckResult::Allowed));
}

// --- Doom loop detection ---

#[test]
fn doom_loop_triggers_after_three_repeated_calls() {
    let mut checker = make_checker(SecurityMode::Standard);
    checker.check("bash", "ls");
    checker.check("bash", "ls");
    let result = checker.check("bash", "ls");
    assert!(matches!(result, CheckResult::Ask));
}

#[test]
fn doom_loop_does_not_trigger_before_three() {
    let mut checker = make_checker(SecurityMode::Standard);
    checker.check("bash", "ls");
    let result = checker.check("bash", "ls");
    assert!(matches!(result, CheckResult::Allowed));
}

#[test]
fn doom_loop_resets_for_different_inputs() {
    let mut checker = make_checker(SecurityMode::Standard);
    checker.check("bash", "ls");
    checker.check("bash", "ls");
    checker.check("bash", "pwd");
    let result = checker.check("bash", "pwd");
    assert!(matches!(result, CheckResult::Allowed));
}

// --- Session allowlist ---

#[test]
fn session_allowlist_bypasses_rules() {
    let mut checker = make_checker(SecurityMode::Restrictive);
    checker.add_session_allowlist("bash".into(), "cargo test **");
    let result = checker.check("bash", "cargo test --all");
    assert!(matches!(result, CheckResult::Allowed));
}

#[test]
fn session_allowlist_is_tool_specific() {
    let mut checker = make_checker(SecurityMode::Restrictive);
    checker.add_session_allowlist("read".into(), "**");
    assert!(matches!(
        checker.check("read", "/etc/passwd"),
        CheckResult::Allowed
    ));
    assert!(matches!(
        checker.check("write", "some/file.txt"),
        CheckResult::Ask
    ));
}

// --- External path detection ---

#[test]
fn external_absolute_path_outside_cwd_is_detected() {
    let mut checker = make_checker(SecurityMode::Standard);
    let external_path = if cfg!(windows) {
        "D:\\outside\\secret.txt"
    } else {
        "/etc/shadow"
    };
    let result = checker.check_path("write", external_path);
    assert!(
        matches!(result, CheckResult::Ask),
        "expected Ask, got {:?}",
        result,
    );
}

#[test]
fn relative_path_is_not_external() {
    let mut checker = make_checker(SecurityMode::Accept);
    let result = checker.check_path("read", "src/lib.rs");
    assert!(matches!(result, CheckResult::Allowed));
}

// --- Config-driven rules ---

#[test]
fn explicit_granular_rules_take_effect() {
    let config = PermissionConfig {
        read: Some(ToolPerm::Granular(
            [
                ("*.md".to_string(), Action::Allow),
                ("*.rs".to_string(), Action::Ask),
            ]
            .into(),
        )),
        ..PermissionConfig::default()
    };
    let mut checker = PermissionChecker::new(&configs_from(config), SecurityMode::Standard, None);
    assert_eq!(checker.check("read", "README.md"), CheckResult::Allowed);
    assert_eq!(checker.check("read", "main.rs"), CheckResult::Ask);
}

// --- Standard mode: always allow read/write/edit/list_dir within CWD ---

#[test]
fn standard_allows_path_tools_in_cwd_despite_deny_rules() {
    let config = PermissionConfig {
        read: Some(ToolPerm::Simple(Action::Deny)),
        ..PermissionConfig::default()
    };
    let mut checker = PermissionChecker::new(
        &configs_from(config),
        SecurityMode::Standard,
        Some(std::path::PathBuf::from("/home/user/project")),
    );
    // Deny rule is overridden — CWD paths are always allowed in Standard mode
    let result = checker.check_path("read", "/home/user/project/src/main.rs");
    assert!(
        matches!(result, CheckResult::Allowed),
        "expected Allowed for CWD path, got {:?}",
        result,
    );
}

#[test]
fn standard_allows_write_in_cwd_despite_deny_rules() {
    let config = PermissionConfig {
        write: Some(ToolPerm::Simple(Action::Deny)),
        ..PermissionConfig::default()
    };
    let mut checker = PermissionChecker::new(
        &configs_from(config),
        SecurityMode::Standard,
        Some(std::path::PathBuf::from("/home/user/project")),
    );
    let result = checker.check_path("write", "/home/user/project/new_file.rs");
    assert!(
        matches!(result, CheckResult::Allowed),
        "expected Allowed for CWD path, got {:?}",
        result,
    );
}

#[test]
fn standard_asks_external_path_even_for_path_tools() {
    // External paths should still trigger Ask in Standard mode
    let mut checker = make_checker(SecurityMode::Standard);
    let external = if cfg!(windows) {
        "D:\\outside\\file.txt"
    } else {
        "/etc/config.conf"
    };
    let result = checker.check_path("read", external);
    assert!(
        matches!(result, CheckResult::Ask),
        "expected Ask for external path, got {:?}",
        result,
    );
}

#[test]
fn standard_deny_still_works_for_non_path_tools() {
    // Non-path tools (bash, grep, etc.) should still respect deny rules
    let mut checker = make_checker(SecurityMode::Standard);
    let result = checker.check("bash", "rm -rf /home/user/project");
    assert!(
        matches!(result, CheckResult::Denied(_)),
        "expected Denied for bash deny rule, got {:?}",
        result,
    );
}

#[test]
fn standard_list_dir_in_cwd_is_allowed() {
    let mut checker = make_checker(SecurityMode::Standard);
    let result = checker.check_path("list_dir", "/home/user/project/src");
    assert!(
        matches!(result, CheckResult::Allowed),
        "expected Allowed for list_dir in CWD, got {:?}",
        result,
    );
}

// --- Regex permission rules ---

#[test]
fn regex_granular_rules_take_effect() {
    let config = PermissionConfig {
        read: Some(ToolPerm::Granular(
            [
                (r"\.md$".to_string(), Action::Allow),
                (r"\.rs$".to_string(), Action::Ask),
            ]
            .into(),
        )),
        ..PermissionConfig::default()
    };
    let configs = PermissionConfigs {
        regex: config,
        ..PermissionConfigs::default()
    };
    let mut checker = PermissionChecker::new(&configs, SecurityMode::Standard, None);
    assert_eq!(checker.check("read", "README.md"), CheckResult::Allowed);
    assert_eq!(checker.check("read", "main.rs"), CheckResult::Ask);
    assert_eq!(checker.check("read", "main.py"), CheckResult::Allowed);
}

#[test]
fn regex_simple_action() {
    let config = PermissionConfig {
        bash: Some(ToolPerm::Simple(Action::Ask)),
        ..PermissionConfig::default()
    };
    let configs = PermissionConfigs {
        regex: config,
        ..PermissionConfigs::default()
    };
    let mut checker = PermissionChecker::new(&configs, SecurityMode::Standard, None);
    let result = checker.check("bash", "anything");
    assert!(matches!(result, CheckResult::Ask));
}

#[test]
fn regex_and_glob_rules_merge() {
    let glob = PermissionConfig {
        read: Some(ToolPerm::Granular(
            [("*.md".to_string(), Action::Allow)].into(),
        )),
        ..PermissionConfig::default()
    };
    let regex = PermissionConfig {
        read: Some(ToolPerm::Granular(
            [(r"\.rs$".to_string(), Action::Ask)].into(),
        )),
        ..PermissionConfig::default()
    };
    let configs = PermissionConfigs { glob, regex };
    let mut checker = PermissionChecker::new(&configs, SecurityMode::Standard, None);
    assert_eq!(checker.check("read", "README.md"), CheckResult::Allowed);
    assert_eq!(checker.check("read", "main.rs"), CheckResult::Ask);
}

#[test]
fn regex_default_action_used_when_no_glob_default() {
    let glob = PermissionConfig::default();
    let regex = PermissionConfig {
        default: Some(Action::Ask),
        ..PermissionConfig::default()
    };
    let configs = PermissionConfigs { glob, regex };
    let mut checker = PermissionChecker::new(&configs, SecurityMode::Standard, None);
    // Default from regex config should be used when glob has no default
    let result = checker.check("unknown_tool", "anything");
    assert!(matches!(result, CheckResult::Ask));
}

#[test]
fn regex_glob_default_precedence() {
    let glob = PermissionConfig {
        default: Some(Action::Allow),
        ..PermissionConfig::default()
    };
    let regex = PermissionConfig {
        default: Some(Action::Ask),
        ..PermissionConfig::default()
    };
    let configs = PermissionConfigs { glob, regex };
    let mut checker = PermissionChecker::new(&configs, SecurityMode::Standard, None);
    // Glob default should take precedence over regex default
    let result = checker.check("unknown_tool", "anything");
    assert!(matches!(result, CheckResult::Allowed));
}

// --- Path traversal detection (normalize_path) ---

#[test]
fn path_traversal_with_dotdot_is_detected_as_external() {
    let mut checker = make_checker(SecurityMode::Standard);
    let traversal = if cfg!(windows) {
        "C:\\home\\user\\project\\..\\etc\\shadow"
    } else {
        "/home/user/project/../etc/shadow"
    };
    let result = checker.check_path("read", traversal);
    assert!(
        matches!(result, CheckResult::Ask),
        "expected Ask for traversal path, got {:?}",
        result,
    );
}

#[test]
fn dot_components_are_normalized_away() {
    let mut checker = make_checker(SecurityMode::Standard);
    let path = if cfg!(windows) {
        "C:\\home\\user\\project\\.\\src\\main.rs"
    } else {
        "/home/user/project/./src/main.rs"
    };
    let result = checker.check_path("read", path);
    assert!(
        matches!(result, CheckResult::Allowed),
        "expected Allowed for dot-normalized CWD path, got {:?}",
        result,
    );
}

#[test]
fn nested_dotdot_traverses_to_root() {
    let mut checker = make_checker(SecurityMode::Standard);
    let traversal = if cfg!(windows) {
        "C:\\home\\user\\project\\..\\..\\..\\etc\\passwd"
    } else {
        "/home/user/project/../../../etc/passwd"
    };
    let result = checker.check_path("read", traversal);
    assert!(
        matches!(result, CheckResult::Ask),
        "expected Ask for deep traversal path, got {:?}",
        result,
    );
}

// --- Session allowlist with absolute paths on check_path ---

#[test]
fn session_allowlist_matches_absolute_path_when_stored_as_relative() {
    let mut checker = make_checker(SecurityMode::Restrictive);
    checker.add_session_allowlist("read".into(), "src/*");
    let result = checker.check_path("read", "/home/user/project/src/main.rs");
    assert!(
        matches!(result, CheckResult::Allowed),
        "expected Allowed for absolute path matching relative allowlist, got {:?}",
        result,
    );
}

#[test]
fn session_allowlist_matches_relative_path_when_stored_as_absolute() {
    let mut checker = make_checker(SecurityMode::Restrictive);
    checker.add_session_allowlist("read".into(), "/home/user/project/src/*");
    let result = checker.check_path("read", "src/main.rs");
    assert!(
        matches!(result, CheckResult::Allowed),
        "expected Allowed for relative path matching absolute allowlist, got {:?}",
        result,
    );
}

// --- MCP tool config ---

#[test]
fn mcp_tool_simple_rule_is_respected() {
    let config = PermissionConfig {
        mcp_tool: Some(ToolPerm::Simple(Action::Deny)),
        ..PermissionConfig::default()
    };
    let mut checker = PermissionChecker::new(&configs_from(config), SecurityMode::Standard, None);
    let result = checker.check("mcp_tool", "mcp_tool:filesystem:read_file");
    assert!(
        matches!(result, CheckResult::Denied(_)),
        "expected Denied for MCP tool, got {:?}",
        result,
    );
}

#[test]
fn mcp_tool_granular_rules_respected() {
    let config = PermissionConfig {
        mcp_tool: Some(ToolPerm::Granular(
            [
                ("mcp_tool:fs:allow_*".to_string(), Action::Allow),
                ("mcp_tool:fs:deny_*".to_string(), Action::Deny),
            ]
            .into(),
        )),
        ..PermissionConfig::default()
    };
    let mut checker = PermissionChecker::new(&configs_from(config), SecurityMode::Standard, None);
    assert_eq!(
        checker.check("mcp_tool", "mcp_tool:fs:allow_read"),
        CheckResult::Allowed
    );
    assert!(matches!(
        checker.check("mcp_tool", "mcp_tool:fs:deny_write"),
        CheckResult::Denied(_)
    ));
}

#[test]
fn mcp_tool_default_action_when_no_rules() {
    let mut checker = make_checker(SecurityMode::Standard);
    let result = checker.check("mcp_tool", "mcp_tool:some_server:some_tool");
    assert!(
        matches!(result, CheckResult::Allowed),
        "expected Allowed for MCP tool with no rules (default), got {:?}",
        result,
    );
}
