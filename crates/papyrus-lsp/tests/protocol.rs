//! The server, end to end: a minimal JSON-RPC client on a temporary
//! project.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream};

struct Client {
    reader: BufReader<DuplexStream>,
    writer: DuplexStream,
    next_id: i64,
}

impl Client {
    async fn send(&mut self, message: Value) {
        let body = message.to_string();
        let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        self.writer.write_all(frame.as_bytes()).await.unwrap();
    }

    async fn receive(&mut self) -> Value {
        let mut length = 0;
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line).await.unwrap();
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                length = value.parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        self.reader.read_exact(&mut body).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// The next message satisfying `wanted`; the others are ignored.
    async fn wait(&mut self, wanted: impl Fn(&Value) -> bool) -> Value {
        let search = async {
            loop {
                let message = self.receive().await;
                if wanted(&message) {
                    return message;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(20), search)
            .await
            .expect("expected a response")
    }

    async fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }))
            .await;
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await;
        let response = self.wait(|m| m["id"] == id).await;
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
}

fn uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn project() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("papyrus.toml"), "game = \"skyrim\"\n").unwrap();
    std::fs::write(
        dir.join("MyBase.psc"),
        "ScriptName MyBase extends ObjectReference\nEvent OnRitualDone()\nEndEvent\n",
    )
    .unwrap();
    dir
}

/// A server running in the background, and a client talking to it.
fn connect() -> Client {
    // Two pipes, one per direction.
    let (to_server, server_in) = tokio::io::duplex(1 << 16);
    let (server_out, from_server) = tokio::io::duplex(1 << 16);
    tokio::spawn(papyrus_lsp::serve(server_in, server_out));
    Client {
        reader: BufReader::new(from_server),
        writer: to_server,
        next_id: 0,
    }
}

#[tokio::test]
async fn a_full_session() {
    let dir = project();
    let mut client = connect();

    let result = client
        .request(
            "initialize",
            json!({
                "capabilities": {
                    "general": { "positionEncodings": ["utf-8", "utf-16"] },
                    "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } },
                },
                "workspaceFolders": [{ "uri": uri(&dir), "name": "project" }],
            }),
        )
        .await;
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-8");
    client.notify("initialized", json!({})).await;
    // The server asks to watch the configuration and the scripts.
    let registration = client
        .wait(|m| m["method"] == "client/registerCapability")
        .await;
    let watchers = &registration["params"]["registrations"][0]["registerOptions"]["watchers"];
    assert_eq!(watchers[0]["globPattern"], "**/papyrus.toml");
    client
        .send(json!({ "jsonrpc": "2.0", "id": registration["id"], "result": null }))
        .await;
    let log = client.wait(|m| m["method"] == "window/logMessage").await;
    let log = log["params"]["message"].as_str().unwrap().to_owned();
    assert!(log.contains("1 scripts indexed"), "{log}");

    // A misspelled event, inherited from a project script, and a badly named
    // local.
    let child = dir.join("Child.psc");
    let child_uri = uri(&child);
    let text = "ScriptName Child extends MyBase\nEvent OnRitualDome()\nEndEvent\nFunction F()\n    int HP = 1\n    Debug.Trace(HP)\nEndFunction\n";
    client
        .notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": child_uri, "languageId": "papyrus", "version": 1, "text": text,
            } }),
        )
        .await;
    let published = client
        .wait(|m| m["method"] == "textDocument/publishDiagnostics")
        .await;
    let codes: Vec<&str> = published["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["PAP004", "PAP101"]);
    let event = &published["params"]["diagnostics"][0];
    assert_eq!(
        event["range"]["start"],
        json!({ "line": 1, "character": 6 })
    );

    let document = json!({ "uri": child_uri });
    let actions = client
        .request(
            "textDocument/codeAction",
            json!({
                "textDocument": document,
                "range": event["range"],
                "context": { "diagnostics": [] },
            }),
        )
        .await;
    let titles: Vec<&str> = actions
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rename to `OnRitualDone`"), "{titles:?}");
    assert!(
        titles.contains(&"Allow `events/unknown` on this line"),
        "{titles:?}"
    );

    let edits = client
        .request(
            "textDocument/formatting",
            json!({ "textDocument": document, "options": { "tabSize": 4, "insertSpaces": true } }),
        )
        .await;
    let formatted = edits[0]["newText"].as_str().unwrap();
    // The papyrus.toml indent, or two spaces: not the editor's.
    assert!(formatted.contains("\n  int HP = 1\n"), "{formatted}");
    assert!(
        formatted.starts_with("scriptname Child extends MyBase\n"),
        "{formatted}"
    );

    let symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": document }),
        )
        .await;
    assert_eq!(symbols[0]["name"], "Child");
    assert_eq!(symbols[0]["children"].as_array().unwrap().len(), 2);

    let tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": document }),
        )
        .await;
    assert!(!tokens["data"].as_array().unwrap().is_empty());

    let folds = client
        .request(
            "textDocument/foldingRange",
            json!({ "textDocument": document }),
        )
        .await;
    assert_eq!(folds, json!([{ "startLine": 3, "endLine": 5 }]));

    // Fixed in the editor: the diagnostics follow.
    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": child_uri, "version": 2 },
                "contentChanges": [{ "text": text.replace("Dome", "Done").replace("HP", "hp") }],
            }),
        )
        .await;
    let published = client
        .wait(|m| m["method"] == "textDocument/publishDiagnostics")
        .await;
    assert_eq!(published["params"]["diagnostics"], json!([]));
    assert_eq!(published["params"]["version"], 2);

    // The configuration changes on disk: the levels follow.
    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": child_uri, "version": 3 },
                "contentChanges": [{ "text": text.replace("Dome", "Done") }],
            }),
        )
        .await;
    let published = client
        .wait(|m| m["method"] == "textDocument/publishDiagnostics")
        .await;
    assert_eq!(published["params"]["diagnostics"][0]["code"], "PAP101");
    std::fs::write(
        dir.join("papyrus.toml"),
        "game = \"skyrim\"\n[lints]\n\"naming/locals\" = \"allow\"\n",
    )
    .unwrap();
    client
        .notify(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": [{ "uri": uri(&dir.join("papyrus.toml")), "type": 2 }] }),
        )
        .await;
    let published = client
        .wait(|m| m["method"] == "textDocument/publishDiagnostics")
        .await;
    assert_eq!(published["params"]["diagnostics"], json!([]));

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn indexing_shows_its_progress() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-progress-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("A.psc"), "ScriptName A extends Quest\n").unwrap();
    let mut client = connect();
    client
        .request(
            "initialize",
            json!({
                "capabilities": { "window": { "workDoneProgress": true } },
                "workspaceFolders": [{ "uri": uri(&dir), "name": "project" }],
            }),
        )
        .await;
    client.notify("initialized", json!({})).await;
    let create = client
        .wait(|m| m["method"] == "window/workDoneProgress/create")
        .await;
    let token = create["params"]["token"].clone();
    client
        .send(json!({ "jsonrpc": "2.0", "id": create["id"], "result": null }))
        .await;
    let begin = client.wait(|m| m["method"] == "$/progress").await;
    assert_eq!(begin["params"]["token"], token);
    assert_eq!(begin["params"]["value"]["kind"], "begin");
    let end = client
        .wait(|m| m["method"] == "$/progress" && m["params"]["value"]["kind"] != "report")
        .await;
    assert_eq!(end["params"]["value"]["kind"], "end");
    let log = client.wait(|m| m["method"] == "window/logMessage").await;
    assert!(
        log["params"]["message"]
            .as_str()
            .unwrap()
            .contains("1 scripts indexed")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn each_project_of_a_window_stays_apart() {
    // One window, two projects declaring a `MyBase` script with different
    // events: each child is checked against its own.
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-projects-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in [
        ("skyrim/papyrus.toml", "game = \"skyrim\"\n"),
        (
            "skyrim/MyBase.psc",
            "ScriptName MyBase extends ObjectReference\nEvent OnRitualDone()\nEndEvent\n",
        ),
        ("fallout/papyrus.toml", "game = \"fallout4\"\n"),
        (
            "fallout/MyBase.psc",
            "ScriptName MyBase extends ObjectReference\nEvent OnRitualDune()\nEndEvent\n",
        ),
    ] {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    let mut client = connect();
    client
        .request(
            "initialize",
            json!({
                "capabilities": {},
                "workspaceFolders": [{ "uri": uri(&dir), "name": "mods" }],
            }),
        )
        .await;
    client.notify("initialized", json!({})).await;
    // One line per project once indexed.
    for _ in 0..2 {
        client.wait(|m| m["method"] == "window/logMessage").await;
    }

    let text = "ScriptName Child extends MyBase\nEvent OnRitualDone()\nEndEvent\n";
    let mut found = Vec::new();
    for project in ["skyrim", "fallout"] {
        let child = uri(&dir.join(project).join("Child.psc"));
        client
            .notify(
                "textDocument/didOpen",
                json!({ "textDocument": {
                    "uri": child, "languageId": "papyrus", "version": 1, "text": text,
                } }),
            )
            .await;
        let published = client
            .wait(|m| {
                m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == child
            })
            .await;
        let messages: Vec<String> = published["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["message"].as_str().unwrap().to_owned())
            .collect();
        found.push((project, messages));
    }
    assert_eq!(found[0], ("skyrim", Vec::new()));
    assert_eq!(found[1].1.len(), 1, "{found:?}");
    assert!(found[1].1[0].contains("`OnRitualDune`"), "{found:?}");

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// A URI the server built, with the drive colon decoded as `uri` writes it.
fn decoded(uri: &Value) -> String {
    uri.as_str().unwrap().replace("%3A", ":")
}

/// The position of the first `needle` in `text`, plus `shift` characters.
fn at(text: &str, needle: &str, shift: usize) -> Value {
    let offset = text.find(needle).unwrap() + shift;
    let line = text[..offset].matches('\n').count();
    let character = offset - text[..offset].rfind('\n').map_or(0, |i| i + 1);
    json!({ "line": line, "character": character })
}

#[tokio::test]
async fn navigation_across_files() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-nav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in [
        (
            "mod/papyrus.toml",
            "game = \"skyrim\"\nimports = [\"../lib\"]\n",
        ),
        (
            "mod/MyBase.psc",
            "ScriptName MyBase extends Lib\nint Property Count Auto\n{ How many }\nFunction Greet()\nEndFunction\n",
        ),
        (
            "lib/Lib.psc",
            "ScriptName Lib extends ObjectReference\nFunction Shared()\nEndFunction\n",
        ),
    ] {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let root = dir.join("mod");

    let mut client = connect();
    let result = client
        .request(
            "initialize",
            json!({
                "capabilities": {},
                "workspaceFolders": [{ "uri": uri(&root), "name": "mod" }],
            }),
        )
        .await;
    let capabilities = &result["capabilities"];
    assert_eq!(capabilities["definitionProvider"], true);
    assert_eq!(capabilities["renameProvider"]["prepareProvider"], true);
    client.notify("initialized", json!({})).await;
    let log = client.wait(|m| m["method"] == "window/logMessage").await;
    let log = log["params"]["message"].as_str().unwrap().to_owned();
    assert!(log.contains("1 scripts indexed, 1 imported"), "{log}");

    let child = uri(&root.join("Child.psc"));
    let text = "ScriptName Child extends MyBase\nEvent OnInit()\n\tGreet()\n\tShared()\n\tCount += 1\n\tself.\nEndEvent\n";
    client
        .notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": child, "languageId": "papyrus", "version": 1, "text": text,
            } }),
        )
        .await;
    let document = json!({ "uri": child });
    let position = |needle: &str, shift: usize| json!({ "textDocument": document, "position": at(text, needle, shift) });

    // Into the project, and into an imported script.
    let found = client
        .request("textDocument/definition", position("Greet", 1))
        .await;
    assert_eq!(decoded(&found["uri"]), uri(&root.join("MyBase.psc")));
    assert_eq!(
        found["range"]["start"],
        json!({ "line": 3, "character": 9 })
    );
    let found = client
        .request("textDocument/definition", position("Shared", 1))
        .await;
    assert_eq!(
        decoded(&found["uri"]),
        uri(&dir.join("lib").join("Lib.psc"))
    );

    let hover = client
        .request("textDocument/hover", position("Count", 1))
        .await;
    let value = hover["contents"]["value"].as_str().unwrap();
    assert!(value.contains("int Property Count Auto"), "{value}");
    assert!(value.contains("How many"), "{value}");

    let completion = client
        .request("textDocument/completion", position("self.", 5))
        .await;
    let labels: Vec<&str> = completion
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["label"].as_str().unwrap())
        .collect();
    for expected in ["Count", "Greet", "Shared", "MoveTo"] {
        assert!(labels.contains(&expected), "{expected} in {labels:?}");
    }

    let references = client
        .request(
            "textDocument/references",
            json!({
                "textDocument": document,
                "position": at(text, "Count", 1),
                "context": { "includeDeclaration": true },
            }),
        )
        .await;
    assert_eq!(references.as_array().unwrap().len(), 2, "{references}");

    // `Count += 1` writes.
    let highlights = client
        .request("textDocument/documentHighlight", position("Count", 1))
        .await;
    assert_eq!(highlights.as_array().unwrap().len(), 1, "{highlights}");
    assert_eq!(highlights[0]["kind"], 3);

    // Into a file that is not open: who calls `Greet`, and what it calls.
    let items = client
        .request("textDocument/prepareCallHierarchy", position("Greet", 1))
        .await;
    assert_eq!(items[0]["name"], "Greet");
    assert_eq!(decoded(&items[0]["uri"]), uri(&root.join("MyBase.psc")));
    let incoming = client
        .request("callHierarchy/incomingCalls", json!({ "item": items[0] }))
        .await;
    assert_eq!(incoming[0]["from"]["name"], "OnInit", "{incoming}");
    let items = client
        .request("textDocument/prepareCallHierarchy", position("OnInit", 1))
        .await;
    let outgoing = client
        .request("callHierarchy/outgoingCalls", json!({ "item": items[0] }))
        .await;
    let names: Vec<&str> = outgoing
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["to"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Greet", "Shared"]);

    // A lens per member of `MyBase`, counted once resolved.
    let base = uri(&root.join("MyBase.psc"));
    let lenses = client
        .request(
            "textDocument/codeLens",
            json!({ "textDocument": { "uri": base } }),
        )
        .await;
    assert_eq!(lenses.as_array().unwrap().len(), 2, "{lenses}");
    let resolved = client.request("codeLens/resolve", lenses[1].clone()).await;
    assert_eq!(resolved["command"]["title"], "1 reference");
    assert_eq!(resolved["command"]["command"], "papyrus.showReferences");

    let symbols = client
        .request("workspace/symbol", json!({ "query": "greet" }))
        .await;
    assert_eq!(symbols[0]["name"], "Greet");

    // A property can be renamed, with a warning.
    let prepared = client
        .request("textDocument/prepareRename", position("Count", 1))
        .await;
    assert_eq!(prepared["placeholder"], "Count");
    // The edit, and a warning, in any order.
    client.next_id += 1;
    let id = client.next_id;
    client
        .send(json!({
            "jsonrpc": "2.0", "id": id, "method": "textDocument/rename",
            "params": {
                "textDocument": document,
                "position": at(text, "Count", 1),
                "newName": "Total",
            },
        }))
        .await;
    let (mut edit, mut warning) = (Value::Null, Value::Null);
    while edit.is_null() || warning.is_null() {
        let message = client
            .wait(|m| m["id"] == id || m["method"] == "window/showMessage")
            .await;
        if message["id"] == id {
            edit = message;
        } else {
            warning = message;
        }
    }
    let warning = warning["params"]["message"].as_str().unwrap();
    assert!(warning.contains("Creation Kit"), "{warning}");
    let changes = edit["result"]["changes"].as_object().unwrap();
    assert_eq!(changes.len(), 2, "{edit}");

    // An imported function cannot.
    client.next_id += 1;
    let id = client.next_id;
    client
        .send(json!({
            "jsonrpc": "2.0", "id": id, "method": "textDocument/prepareRename",
            "params": position("Shared", 1),
        }))
        .await;
    let refused = client.wait(|m| m["id"] == id).await;
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(message.contains("imported"), "{message}");

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// The codes of a `publishDiagnostics` notification.
fn codes(message: &Value) -> Vec<String> {
    message["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn calls_are_checked_helped_and_hinted() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-calls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("papyrus.toml"), "game = \"skyrim\"\n").unwrap();
    let lib_text = "ScriptName Lib1 extends Quest\nFunction Start(int stage, bool abForce = false)\nEndFunction\n";
    std::fs::write(dir.join("Lib1.psc"), lib_text).unwrap();

    let mut client = connect();
    let result = client
        .request(
            "initialize",
            json!({
                "capabilities": {},
                "workspaceFolders": [{ "uri": uri(&dir), "name": "calls" }],
                "initializationOptions": { "inlayHints": { "parameterNames": "all" } },
            }),
        )
        .await;
    let capabilities = &result["capabilities"];
    assert_eq!(capabilities["inlayHintProvider"], true);
    assert_eq!(
        capabilities["signatureHelpProvider"]["triggerCharacters"],
        json!(["(", ","])
    );
    client.notify("initialized", json!({})).await;
    client.wait(|m| m["method"] == "window/logMessage").await;

    // A call missing its argument, in another file than the callee.
    let child = uri(&dir.join("Child.psc"));
    let text = "ScriptName Child extends Quest\nLib1 Property other Auto\nEvent OnInit()\n\tother.Start()\n\tother.Start(1, true)\nEndEvent\n";
    client
        .notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": child, "languageId": "papyrus", "version": 1, "text": text,
            } }),
        )
        .await;
    let published = client
        .wait(|m| {
            m["method"] == "textDocument/publishDiagnostics"
                && decoded(&m["params"]["uri"]) == child
                && codes(m).contains(&"PAP005".to_owned())
        })
        .await;
    assert_eq!(codes(&published), ["PAP005"]);

    let document = json!({ "uri": child });
    let help = client
        .request(
            "textDocument/signatureHelp",
            json!({ "textDocument": document, "position": at(text, "Start(1", 6) }),
        )
        .await;
    assert_eq!(
        help["signatures"][0]["label"],
        "Start(int stage, bool abForce = false)"
    );
    assert_eq!(help["activeParameter"], 0);
    assert_eq!(
        help["signatures"][0]["parameters"][1]["label"],
        json!([17, 37])
    );

    let whole =
        json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 9, "character": 0 } });
    let hints = client
        .request(
            "textDocument/inlayHint",
            json!({ "textDocument": document, "range": whole }),
        )
        .await;
    let labels: Vec<&str> = hints
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["stage:", "abForce:"]);

    // Turned off: the server asks the editor to refresh.
    client
        .notify(
            "workspace/didChangeConfiguration",
            json!({ "settings": { "papyrus": { "inlayHints": { "parameterNames": "none" } } } }),
        )
        .await;
    let refresh = client
        .wait(|m| m["method"] == "workspace/inlayHint/refresh")
        .await;
    client
        .send(json!({ "jsonrpc": "2.0", "id": refresh["id"], "result": null }))
        .await;
    let hints = client
        .request(
            "textDocument/inlayHint",
            json!({ "textDocument": document, "range": whole }),
        )
        .await;
    assert_eq!(hints, json!([]));

    // A default value in the callee clears the caller.
    let lib = uri(&dir.join("Lib1.psc"));
    client
        .notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": lib, "languageId": "papyrus", "version": 1, "text": lib_text,
            } }),
        )
        .await;
    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": lib, "version": 2 },
                "contentChanges": [{ "text": lib_text.replace("int stage", "int stage = 0") }],
            }),
        )
        .await;
    client
        .wait(|m| {
            m["method"] == "textDocument/publishDiagnostics"
                && decoded(&m["params"]["uri"]) == child
                && codes(m).is_empty()
        })
        .await;

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// A session on `dir`, whose client creates files in its workspace edits.
#[tokio::test]
async fn scripts_are_renamed_with_their_files_and_form_a_hierarchy() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-types-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("papyrus.toml"), "game = \"skyrim\"\n").unwrap();
    let base = dir.join("Base.psc");
    let child = dir.join("Child.psc");
    std::fs::write(&base, "ScriptName Base extends Quest\n").unwrap();
    std::fs::write(&child, "ScriptName Child extends Base\n").unwrap();
    let mut client = connect();
    client
        .request(
            "initialize",
            json!({
                "capabilities": { "workspace": { "workspaceEdit": {
                    "documentChanges": true, "resourceOperations": ["rename"],
                } } },
                "workspaceFolders": [{ "uri": uri(&dir), "name": "project" }],
            }),
        )
        .await;
    client.notify("initialized", json!({})).await;
    client
        .wait(|m| {
            m["method"] == "window/logMessage"
                && m["params"]["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("indexed"))
        })
        .await;

    // The hierarchy, from the parent down and back up.
    let prepared = client
        .request(
            "textDocument/prepareTypeHierarchy",
            json!({
                "textDocument": { "uri": uri(&base) },
                "position": { "line": 0, "character": 12 },
            }),
        )
        .await;
    let item = prepared[0].clone();
    assert_eq!(item["name"], "Base");
    let subtypes = client
        .request("typeHierarchy/subtypes", json!({ "item": item }))
        .await;
    assert_eq!(subtypes[0]["name"], "Child");
    let supertypes = client
        .request("typeHierarchy/supertypes", json!({ "item": subtypes[0] }))
        .await;
    assert_eq!(supertypes[0]["name"], "Base");

    // Renaming the script moves the file, after the edits.
    let edit = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri(&base) },
                "position": { "line": 0, "character": 12 },
                "newName": "Root",
            }),
        )
        .await;
    let changes = edit["documentChanges"].as_array().unwrap();
    assert_eq!(changes.len(), 3, "{edit}");
    let rename = &changes[2];
    assert_eq!(rename["kind"], "rename");
    assert!(rename["newUri"].as_str().unwrap().ends_with("/Root.psc"));

    // Renaming the file updates the script.
    let edit = client
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": uri(&base),
                "newUri": uri(&dir.join("Root.psc")),
            }] }),
        )
        .await;
    let changes = edit["changes"].as_object().unwrap();
    assert_eq!(changes.len(), 2, "{edit}");
    for edits in changes.values() {
        assert_eq!(edits[0]["newText"], "Root");
    }

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn range_formatting_stays_on_the_selected_lines() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-range-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("papyrus.toml"), "game = \"skyrim\"\n").unwrap();
    let script = dir.join("MyQuest.psc");
    let text = "scriptname MyQuest\n\nfunction F()\nint a = 1\nint b = 2\nendfunction\n";
    std::fs::write(&script, text).unwrap();
    let mut client = editing_client(&dir).await;
    client
        .notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": uri(&script), "languageId": "papyrus", "version": 1, "text": text,
            } }),
        )
        .await;
    let edits = client
        .request(
            "textDocument/rangeFormatting",
            json!({
                "textDocument": { "uri": uri(&script) },
                "range": {
                    "start": { "line": 4, "character": 0 },
                    "end": { "line": 4, "character": 3 },
                },
                "options": { "tabSize": 4, "insertSpaces": true },
            }),
        )
        .await;
    let edits = edits.as_array().unwrap();
    assert_eq!(edits.len(), 1, "{edits:?}");
    assert_eq!(edits[0]["newText"], "  int b = 2\n");
    assert_eq!(edits[0]["range"]["start"]["line"], 4);
    assert_eq!(edits[0]["range"]["end"]["line"], 5);

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

async fn editing_client(dir: &Path) -> Client {
    let mut client = connect();
    client
        .request(
            "initialize",
            json!({
                "capabilities": { "workspace": { "workspaceEdit": {
                    "documentChanges": true, "resourceOperations": ["create"],
                } } },
                "workspaceFolders": [{ "uri": uri(dir), "name": "project" }],
            }),
        )
        .await;
    client.notify("initialized", json!({})).await;
    client
        .wait(|m| {
            m["method"] == "window/logMessage"
                && m["params"]["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("indexed"))
        })
        .await;
    client
}

/// The code actions on the first diagnostic of `code` in `text`, by title.
async fn actions_on(client: &mut Client, path: &Path, text: &str, code: &str) -> Vec<Value> {
    let document = uri(path);
    client
        .notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": document, "languageId": "papyrus", "version": 1, "text": text,
            } }),
        )
        .await;
    let published = client
        .wait(|m| m["method"] == "textDocument/publishDiagnostics")
        .await;
    let diagnostic = published["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == code)
        .unwrap_or_else(|| panic!("no {code} in {published}"))
        .clone();
    let actions = client
        .request(
            "textDocument/codeAction",
            json!({
                "textDocument": { "uri": document },
                "range": diagnostic["range"],
                "context": { "diagnostics": [] },
            }),
        )
        .await;
    actions.as_array().unwrap().clone()
}

fn titled<'a>(actions: &'a [Value], title: &str) -> &'a Value {
    let titles: Vec<&str> = actions
        .iter()
        .map(|a| a["title"].as_str().unwrap())
        .collect();
    actions
        .iter()
        .find(|a| a["title"] == title)
        .unwrap_or_else(|| panic!("no {title:?} in {titles:?}"))
}

const SKSE_CALL: &str = "ScriptName MyQuest extends Quest\nFunction F()\n    Game.GetPlayer().GetWornForm(4)\nEndFunction\n";

#[tokio::test]
async fn papyrus_toml_is_edited_from_a_diagnostic() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-config-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("papyrus.toml");
    std::fs::write(
        &config,
        "game = \"skyrim\" # SE\n\n[lints]\nPAP001 = \"allow\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("MyQuest.psc"), SKSE_CALL).unwrap();
    let mut client = editing_client(&dir).await;

    let actions = actions_on(&mut client, &dir.join("MyQuest.psc"), SKSE_CALL, "PAP017").await;
    let edit = |title| {
        let changes = titled(&actions, title)["edit"]["changes"]
            .as_object()
            .unwrap();
        let (file, edits) = changes.iter().next().unwrap();
        assert_eq!(decoded(&json!(file)), uri(&config));
        edits[0].clone()
    };
    assert_eq!(
        edit("Enable SKSE in papyrus.toml"),
        json!({
            "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 0 } },
            "newText": "skse = true\n",
        })
    );
    assert_eq!(
        edit("Allow `members/skse` in papyrus.toml")["newText"],
        "\"members/skse\" = \"allow\"\n"
    );
    titled(&actions, "Make `members/skse` an error in papyrus.toml");

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn without_papyrus_toml_one_is_created() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-noconfig-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("papyrus.toml");
    let _ = std::fs::remove_file(&config);
    // A quick fix creates the file, then fills it.
    let text = "ScriptName MyQuest extends Quest\nFunction F()\n    int Unused = 1\nEndFunction\n";
    std::fs::write(dir.join("MyQuest.psc"), text).unwrap();
    let mut client = editing_client(&dir).await;
    let actions = actions_on(&mut client, &dir.join("MyQuest.psc"), text, "PAP015").await;
    let changes =
        &titled(&actions, "Allow `locals/unused` in papyrus.toml")["edit"]["documentChanges"];
    assert_eq!(changes[0]["kind"], "create");
    assert_eq!(decoded(&changes[0]["uri"]), uri(&config));
    let filled = changes[1]["edits"][0]["newText"].as_str().unwrap();
    assert!(filled.contains("\"locals/unused\" = \"allow\""), "{filled}");

    // The command writes it, once.
    let folder = uri(&dir);
    let command = json!({ "command": "papyrus.server.createConfig", "arguments": [folder] });
    let result = client
        .request("workspace/executeCommand", command.clone())
        .await;
    assert_eq!(result["created"], true);
    assert_eq!(decoded(&result["uri"]), uri(&config));
    let written = std::fs::read_to_string(&config).unwrap();
    assert!(written.contains("[lints]"), "{written}");
    let result = client.request("workspace/executeCommand", command).await;
    assert_eq!(result["created"], false);

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_declaration_against_the_convention_takes_the_case_of_its_use() {
    let dir = std::env::temp_dir().join(format!("papyrus-lsp-case-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("papyrus.toml"), "game = \"skyrim\"\n").unwrap();
    let text = "ScriptName MyQuest extends Quest\nEvent OnInit()\n    Initialize()\nEndEvent\nFunction initialize()\nEndFunction\n";
    let script = dir.join("MyQuest.psc");
    std::fs::write(&script, text).unwrap();
    let mut client = editing_client(&dir).await;

    let actions = actions_on(&mut client, &script, text, "PAP020").await;
    let rename = titled(&actions, "Rename the declaration to `Initialize`");
    let changes = rename["edit"]["changes"].as_object().unwrap();
    let edits = changes.values().next().unwrap().as_array().unwrap();
    let lines: Vec<u64> = edits
        .iter()
        .map(|e| {
            assert_eq!(e["newText"], "Initialize");
            e["range"]["start"]["line"].as_u64().unwrap()
        })
        .collect();
    assert!(lines.contains(&2) && lines.contains(&4), "{edits:?}");
    // The use keeps its case: no fix rewrites it.
    assert!(
        actions
            .iter()
            .all(|a| a["title"] != "Rename to `initialize`"),
        "{actions:?}"
    );

    client.request("shutdown", Value::Null).await;
    client.notify("exit", Value::Null).await;
    let _ = std::fs::remove_dir_all(&dir);
}
