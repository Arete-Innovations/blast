//! Tiny string templates the scaffolder still emits OUTSIDE the vendored
//! canonical tree. Keep this file small — anything that lives in the
//! framework itself belongs in `templates/canonical/`, not here.

pub fn env_example(database_url: &str) -> String {
    format!("DATABASE_URL={url}\nBLAST_ENV=dev\nRUST_LOG=info\n", url = database_url,)
}

pub fn env_test_example(database_url: &str) -> String {
    format!("DATABASE_URL={url}\nBLAST_ENV=test\nRUST_LOG=warn\n", url = database_url,)
}

/// Zellij dashboard layout. Lives outside the vendored framework because
/// `blast` (the CLI) owns dashboard ergonomics, not the runtime catalyst.
pub fn dashboard_kdl() -> &'static str {
    r#"// Catablast dashboard zellij layout. Customize freely.
// Each pane is a normal `blast` subprocess.
layout {
    default_tab_template {
        pane size=1 borderless=true {
            plugin location="tab-bar"
        }
        children
    }

    tab name="Dashboard" focus=true {
        pane split_direction="vertical" {
            pane size="30%" name="Menu" command="blast" {
                args "cli"
                focus true
            }
            pane size="70%" split_direction="vertical" {
                pane split_direction="horizontal" {
                    pane name="Server" command="bash" {
                        args "-c" "tail -n200 -f storage/logs/server.log 2>/dev/null || (mkdir -p storage/logs && touch storage/logs/server.log && tail -f storage/logs/server.log)"
                    }
                    pane name="Errors" command="bash" {
                        args "-c" "tail -n200 -f storage/logs/error.log 2>/dev/null || (mkdir -p storage/logs && touch storage/logs/error.log && tail -f storage/logs/error.log)"
                    }
                }
                pane split_direction="horizontal" {
                    pane name="FE HMR" command="bash" {
                        args "-c" "tail -n200 -f storage/logs/fe.log 2>/dev/null || (mkdir -p storage/logs && touch storage/logs/fe.log && tail -f storage/logs/fe.log)"
                    }
                    pane name="Routes" command="bash" {
                        args "-c" "tail -n200 -f storage/logs/routes.log 2>/dev/null || (mkdir -p storage/logs && touch storage/logs/routes.log && tail -f storage/logs/routes.log)"
                    }
                }
            }
        }
    }

    tab name="Fuses" {
        pane name="Fuses" command="blast" {
            args "fuses" "live-table"
        }
    }

    tab name="Logs" {
        pane split_direction="horizontal" {
            pane name="Info" command="bash" {
                args "-c" "tail -n100 -f storage/logs/info.log 2>/dev/null || (touch storage/logs/info.log && tail -f storage/logs/info.log)"
            }
            pane name="Debug" command="bash" {
                args "-c" "tail -n100 -f storage/logs/debug.log 2>/dev/null || (touch storage/logs/debug.log && tail -f storage/logs/debug.log)"
            }
        }
    }
}
"#
}
