//! Le serveur, de bout en bout : un client JSON-RPC minimal sur un projet
//! temporaire.

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

    /// Le prochain message qui satisfait `wanted` ; les autres sont ignorés.
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
            .expect("réponse attendue")
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

#[tokio::test]
async fn une_session_complete() {
    let dir = project();
    // Deux tubes, un par sens.
    let (to_server, server_in) = tokio::io::duplex(1 << 16);
    let (server_out, from_server) = tokio::io::duplex(1 << 16);
    tokio::spawn(papyrus_lsp::serve(server_in, server_out));
    let mut client = Client {
        reader: BufReader::new(from_server),
        writer: to_server,
        next_id: 0,
    };

    let result = client
        .request(
            "initialize",
            json!({
                "capabilities": {
                    "general": { "positionEncodings": ["utf-8", "utf-16"] },
                    "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } },
                },
                "workspaceFolders": [{ "uri": uri(&dir), "name": "projet" }],
            }),
        )
        .await;
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-8");
    client.notify("initialized", json!({})).await;
    // Le serveur demande à surveiller la configuration et les scripts.
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
    assert!(log.contains("1 scripts indexés"), "{log}");

    // Un événement mal orthographié, hérité d'un script du projet, et une
    // locale mal nommée.
    let child = dir.join("Child.psc");
    let child_uri = uri(&child);
    let text = "ScriptName Child extends MyBase\nEvent OnRitualDome()\nEndEvent\nFunction F()\n    int HP = 1\nEndFunction\n";
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
    assert!(titles.contains(&"Renommer en `OnRitualDone`"), "{titles:?}");
    assert!(
        titles.contains(&"Autoriser `events/unknown` sur cette ligne"),
        "{titles:?}"
    );

    let edits = client
        .request(
            "textDocument/formatting",
            json!({ "textDocument": document, "options": { "tabSize": 4, "insertSpaces": true } }),
        )
        .await;
    let formatted = edits[0]["newText"].as_str().unwrap();
    // L'indentation de papyrus.toml, ou deux espaces : pas celle de l'éditeur.
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
    assert_eq!(folds, json!([{ "startLine": 3, "endLine": 4 }]));

    // Corrigé dans l'éditeur : les diagnostics suivent.
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

    // La configuration change sur le disque : les niveaux suivent.
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
