// @ts-check
// Le client LSP de Papyrus : lance `papyrus lsp` et lui confie les `.psc`.

const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

/** @type {LanguageClient | undefined} */
let client;

/**
 * Le binaire à lancer : le réglage, sinon celui que l'extension embarque,
 * sinon `papyrus` dans le `PATH`.
 * @param {vscode.ExtensionContext} context
 */
function serverPath(context) {
  const configured = vscode.workspace.getConfiguration("papyrus").get("server.path");
  if (typeof configured === "string" && configured.trim() !== "") {
    return configured;
  }
  const exe = process.platform === "win32" ? "papyrus.exe" : "papyrus";
  const bundled = path.join(context.extensionPath, "server", exe);
  return fs.existsSync(bundled) ? bundled : "papyrus";
}

/** @param {vscode.ExtensionContext} context */
async function start(context) {
  const command = serverPath(context);
  client = new LanguageClient(
    "papyrus",
    "Papyrus",
    { command, args: ["lsp"] },
    {
      documentSelector: [{ scheme: "file", language: "papyrus" }],
    },
  );
  try {
    await client.start();
  } catch (error) {
    vscode.window.showErrorMessage(
      `Papyrus : impossible de lancer « ${command} lsp ». ` +
        "Installer papyrus, ou régler papyrus.server.path.",
    );
    throw error;
  }
}

/** @param {vscode.ExtensionContext} context */
async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("papyrus.restart", async () => {
      await client?.stop();
      await start(context);
    }),
  );
  await start(context);
}

async function deactivate() {
  await client?.stop();
}

module.exports = { activate, deactivate };
